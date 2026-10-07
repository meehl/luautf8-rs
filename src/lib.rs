//! A UTF-8 support module for Lua.
//!
//! This crate provides the same API as the [`luautf8`](https://github.com/starwing/luautf8) Lua
//! module.
//!
//! Enable the `module` feature to create a compiled Lua module that acts as a drop-in replacement
//! for `luautf8` and can be loaded from Lua code using `require`.
//!
//! For applications that embed Lua via `mlua`, you can use [`create_module`] to create the module
//! table and register it with a [`mlua::Lua`] instance directly.

mod lua_pattern;
mod utf8;

use std::{iter::Peekable, str::Chars};

use icu_casemap::CaseMapper;
use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{DefaultIgnorableCodePoint, EastAsianWidth, GraphemeExtend},
};
use mlua::{
    FromLua, Function, Integer as LuaInteger, IntoLua, IntoLuaMulti, Lua, LuaString, MultiValue,
    Result as LuaResult, Table, Value, Variadic,
};
use unicode_normalization::{UnicodeNormalization, is_nfc};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    lua_pattern::{
        CaptureValue, Match, Pattern, ReplacementError, ReplacementString, find, gmatch,
        replace_with,
    },
    utf8::{char_start, decode_utf8, invalid_codepoint, move_by_chars},
};

const CHAR_PATTERN: &[u8] = if cfg!(any(feature = "lua51", feature = "luajit")) {
    b"[%z\x01-\x7F\xC2-\xF4][\x80-\xBF]*"
} else {
    b"[\x00-\x7F\xC2-\xF4][\x80-\xBF]*"
};

const VERSION: &str = "0.3.0";

/// Creates the module table.
///
/// The returned table can be registered with [`Lua::register_module`]:
///
/// ```no_run
/// # use mlua::{Lua, Result};
/// # use luautf8::create_module;
/// # fn main() -> Result<()> {
/// let lua = Lua::new();
/// let module = create_module(&lua)?;
/// lua.register_module("luautf8", module)?;
/// # Ok(())
/// # }
/// ```
pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let exports = lua.create_table()?;

    // Constants.
    exports.set("charpattern", lua.create_string(CHAR_PATTERN)?)?;
    exports.set("version", VERSION)?;

    // Lua 5.3 `utf8` compatibility.
    exports.set("offset", lua.create_function(l_offset)?)?;
    exports.set("codepoint", lua.create_function(l_codepoint)?)?;
    exports.set("codes", lua.create_function(l_codes)?)?;

    // string module compatibility.
    exports.set("byte", lua.create_function(l_byte)?)?;
    exports.set("char", lua.create_function(l_char)?)?;
    exports.set("find", lua.create_function(l_find)?)?;
    exports.set("gmatch", lua.create_function(l_gmatch)?)?;
    exports.set("gsub", lua.create_function(l_gsub)?)?;
    exports.set("len", lua.create_function(l_len)?)?;
    exports.set("lower", lua.create_function(l_lower)?)?;
    exports.set("match", lua.create_function(l_match)?)?;
    exports.set("reverse", lua.create_function(l_reverse)?)?;
    exports.set("sub", lua.create_function(l_sub)?)?;
    exports.set("upper", lua.create_function(l_upper)?)?;

    // Unicode-specific.
    exports.set("escape", lua.create_function(l_escape)?)?;
    exports.set("charpos", lua.create_function(l_charpos)?)?;
    exports.set("next", lua.create_function(l_next)?)?;
    exports.set("insert", lua.create_function(l_insert)?)?;
    exports.set("remove", lua.create_function(l_remove)?)?;
    exports.set("width", lua.create_function(l_width)?)?;
    exports.set("widthindex", lua.create_function(l_widthindex)?)?;
    exports.set("widthlimit", lua.create_function(l_widthlimit)?)?;
    exports.set("title", lua.create_function(l_title)?)?;
    exports.set("fold", lua.create_function(l_fold)?)?;
    exports.set("ncasecmp", lua.create_function(l_ncasecmp)?)?;
    exports.set("isvalid", lua.create_function(l_isvalid)?)?;
    exports.set("clean", lua.create_function(l_clean)?)?;
    exports.set("invalidoffset", lua.create_function(l_invalidoffset)?)?;
    exports.set("isnfc", lua.create_function(l_isnfc)?)?;
    exports.set("normalize_nfc", lua.create_function(l_normalize_nfc)?)?;
    exports.set("grapheme_indices", lua.create_function(l_grapheme_indices)?)?;

    Ok(exports)
}

/// Helper for constructing a `mlua::Error::BadArgument`
fn bad_argument(to: impl Into<String>, pos: usize, cause: mlua::Error) -> mlua::Error {
    mlua::Error::BadArgument {
        to: Some(to.into()),
        pos: pos + 1,
        name: None,
        cause: std::sync::Arc::new(cause),
    }
}

fn capture_value_to_lua(lua: &Lua, v: CaptureValue<'_>) -> Value {
    match v {
        CaptureValue::Text(t) => Value::String(lua.create_string(t).unwrap()),
        CaptureValue::Position(p) => Value::Integer((p + 1) as _),
    }
}

/// Resolves an index relative to a sequence of length `len`.
///
/// Positive indices are returned unchanged. Negative indices count backward from the end, with
/// `-1` referring to the last position. A negative index that refers before the first position
/// results in `0`.
fn normalize_lua_index(pos: LuaInteger, len: usize) -> usize {
    if pos >= 0 {
        pos as usize
    } else if pos.unsigned_abs() as usize > len {
        0
    } else {
        (len as LuaInteger + pos + 1) as usize
    }
}

fn l_offset(
    lua: &Lua,
    (s, n, i): (String, LuaInteger, Option<LuaInteger>),
) -> LuaResult<MultiValue> {
    let len = s.len();
    let i = match i {
        Some(i) if i < 0 => len as LuaInteger + i + 1,
        Some(i) => i,
        None if n >= 0 => 1,
        None => len as LuaInteger + 1,
    };

    if !(1 <= i && i - 1 <= len as LuaInteger) {
        return Err(bad_argument(
            "offset",
            3,
            mlua::Error::runtime("position out of bounds"),
        ));
    }

    let pos = (i - 1) as usize; // translate to 0-based index

    let start = match n {
        0 => Some(s.floor_char_boundary(pos)),
        _ if !s.is_char_boundary(pos) => {
            return Err(mlua::Error::runtime(
                "initial position is a continuation byte",
            ));
        }
        n if n > 0 => s[pos..]
            .char_indices()
            .map(|(offset, _)| pos + offset)
            // char right after the end returns the string length in bytes
            .chain(std::iter::once(s.len()))
            .nth((n - 1) as usize),
        n if n < 0 => s[..pos]
            .char_indices()
            .rev()
            .nth((-n - 1) as usize)
            .map(|(offset, _)| offset),
        _ => unreachable!("LuaInteger must be zero, positive, or negative"),
    };

    let Some(start) = start else {
        return mlua::Value::Nil.into_lua_multi(lua);
    };

    let end = s
        .get(start..)
        .and_then(|rest| rest.chars().next())
        .map_or(start + 1, |c| start + c.len_utf8());

    // translate back to 1-based indices and return
    (start + 1, end).into_lua_multi(lua)
}

fn l_codepoint(
    _lua: &Lua,
    (s, i, j, lax): (
        LuaString,
        Option<LuaInteger>,
        Option<LuaInteger>,
        Option<bool>,
    ),
) -> LuaResult<Variadic<LuaInteger>> {
    let bytes = s.as_bytes().to_vec();
    let len = bytes.len();
    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    // NOTE: `luautf8` docs claim `j` defaults to `len` but it's actually `i`.
    let j = j.map_or(i, |j| normalize_lua_index(j, len));
    let lax = lax.unwrap_or(false);

    if !(i >= 1) {
        return Err(bad_argument(
            "codepoint",
            2,
            mlua::Error::runtime("out of bounds"),
        ));
    }
    if !(j <= len) {
        return Err(bad_argument(
            "codepoint",
            3,
            mlua::Error::runtime("out of bounds"),
        ));
    }

    if i > j {
        return Ok(Variadic::new());
    }

    let start = i - 1;
    let end = j;
    let mut result = Variadic::new();
    let mut pos = start;
    while pos < end {
        let at = pos;
        let (cp, consumed) = decode_utf8(&bytes[at..])
            .map_err(|_| mlua::Error::runtime("invalid UTF-8 codepoint"))?;
        pos += consumed;
        if !lax && invalid_codepoint(cp) {
            return Err(mlua::Error::runtime("invalid UTF-8 codepoint"));
        }
        result.push(cp as LuaInteger);
    }

    Ok(result)
}

fn l_codes(lua: &Lua, (s, lax): (LuaString, Option<bool>)) -> LuaResult<Function> {
    let bytes = s.as_bytes().to_vec();
    let len = bytes.len();
    let lax = lax.unwrap_or(false);

    let mut pos = 0;
    lua.create_function_mut(move |lua, ()| {
        if pos < len {
            let at = pos;
            let (cp, consumed) = decode_utf8(&bytes[at..])
                .map_err(|_| mlua::Error::runtime("invalid UTF-8 codepoint"))?;
            pos += consumed;
            if !lax && invalid_codepoint(cp) {
                return Err(mlua::Error::runtime("invalid UTF-8 codepoint"));
            }
            (at + 1, cp as LuaInteger).into_lua_multi(lua)
        } else {
            (Value::Nil,).into_lua_multi(lua)
        }
    })
}

fn l_byte(
    _lua: &Lua,
    (s, start, end): (LuaString, Option<LuaInteger>, Option<LuaInteger>),
) -> LuaResult<Variadic<LuaInteger>> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
    let len = s.chars().count() as LuaInteger;

    let normalize = |idx: LuaInteger| if idx >= 0 { idx } else { len + idx + 1 };
    let i = normalize(start.unwrap_or(1)).max(1);
    let j = normalize(end.unwrap_or(start.unwrap_or(1))).min(len);

    if i > j {
        Ok(Variadic::new())
    } else {
        Ok(s.chars()
            .skip((i - 1) as usize)
            .take((j - i + 1) as usize)
            .map(|ch| ch as LuaInteger)
            .collect())
    }
}

fn l_char(lua: &Lua, args: Variadic<LuaInteger>) -> LuaResult<LuaString> {
    let mut bytes = Vec::with_capacity(args.len());

    for (n, cp) in args.iter().enumerate() {
        if !(0..0x110000).contains(cp) {
            return Err(bad_argument(
                "char",
                n,
                mlua::Error::runtime("value out of range"),
            ));
        }

        let cp = *cp as u32;

        match cp {
            0x0000..0x0080 => {
                bytes.push(cp as u8);
            }
            0x0080..0x0800 => {
                bytes.push(0xC0 | (cp >> 6) as u8);
                bytes.push(0x80 | (cp & 0x3F) as u8);
            }
            0x0800..0x010000 => {
                bytes.push(0xE0 | (cp >> 12) as u8);
                bytes.push(0x80 | (cp >> 6 & 0x3F) as u8);
                bytes.push(0x80 | (cp & 0x3F) as u8);
            }
            0x010000..0x110000 => {
                bytes.push(0xF0 | (cp >> 18) as u8);
                bytes.push(0x80 | (cp >> 12 & 0x3F) as u8);
                bytes.push(0x80 | (cp >> 6 & 0x3F) as u8);
                bytes.push(0x80 | (cp & 0x3F) as u8);
            }
            _ => unreachable!(),
        }
    }

    lua.create_string(bytes)
}

fn l_find(
    lua: &Lua,
    (s, pattern, init, plain): (String, String, Option<LuaInteger>, Option<bool>),
) -> LuaResult<MultiValue> {
    let init = init.map_or(1, |x| if x == 0 { 1 } else { x });
    let start_byte = match char_pos(s.as_bytes(), init as isize) {
        Some(start) => start,
        None if init <= 0 => 1,
        None => return (mlua::Value::Nil,).into_lua_multi(lua),
    };

    if plain.unwrap_or(false) {
        match s[start_byte..].find(&pattern) {
            Some(found) => {
                let found_start_byte = start_byte + found;
                let start_char = s[..found_start_byte].chars().count();
                let end_char = start_char + pattern.chars().count();
                Ok(MultiValue::from(vec![
                    Value::Integer((start_char + 1) as _),
                    Value::Integer(end_char as _),
                ]))
            }
            None => (mlua::Value::Nil,).into_lua_multi(lua),
        }
    } else {
        let p = Pattern::parse(&pattern).map_err(|_| mlua::Error::runtime("malformed pattern"))?;
        match find(&s, &p, Some(start_byte)) {
            Some(m) => {
                let pos = vec![
                    Value::Integer((m.start() + 1) as _),
                    Value::Integer(m.end() as _),
                ];
                Ok(MultiValue::from_iter(
                    pos.into_iter()
                        .chain(m.captures().map(|c| capture_value_to_lua(lua, c))),
                ))
            }
            None => (mlua::Value::Nil,).into_lua_multi(lua),
        }
    }
}

fn l_gmatch(lua: &Lua, (s, pattern): (LuaString, LuaString)) -> LuaResult<Function> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?
        .to_owned();
    let pattern_str = pattern
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
    let p = Pattern::parse_gmatch(&pattern_str)
        .map_err(|_| mlua::Error::runtime("malformed pattern"))?;

    let mut matches = gmatch(s, p);

    lua.create_function_mut(move |lua, ()| match matches.next() {
        Some(m) => Ok(MultiValue::from_iter(
            m.values().map(|c| capture_value_to_lua(lua, c)),
        )),
        None => (mlua::Value::Nil,).into_lua_multi(lua),
    })
}

enum LuaReplacement {
    String(ReplacementString),
    Table(mlua::Table),
    Function(mlua::Function),
}

impl LuaReplacement {
    fn apply(&self, lua: &Lua, m: &Match) -> LuaResult<Option<String>> {
        match self {
            Self::String(replacement_string) => Ok(Some(replacement_string.apply(m))),
            Self::Table(table) => {
                // use first capture as key, or whole match if no captures
                let key = m.capture(0).map_or_else(
                    || Value::String(lua.create_string(m.as_str()).unwrap()),
                    |c| capture_value_to_lua(lua, c),
                );

                let value = table.get(key)?;
                match value {
                    Value::String(string) => Ok(Some(string.to_str()?.to_owned())),
                    Value::Integer(integer) => Ok(Some(integer.to_string())),
                    Value::Number(number) => Ok(Some(number.to_string())),
                    Value::Nil | Value::Boolean(false) => Ok(None),
                    other => Err(mlua::Error::runtime(format!(
                        "invalid replacement value (a {})",
                        other.type_name()
                    ))),
                }
            }
            Self::Function(function) => {
                let args = Variadic::from_iter(m.values().map(|c| capture_value_to_lua(lua, c)));

                let value = function.call(args)?;
                match value {
                    Value::String(string) => Ok(Some(string.to_str()?.to_owned())),
                    Value::Integer(integer) => Ok(Some(integer.to_string())),
                    Value::Number(number) => Ok(Some(number.to_string())),
                    Value::Nil | Value::Boolean(false) => Ok(None),
                    other => Err(mlua::Error::runtime(format!(
                        "invalid replacement value (a {})",
                        other.type_name()
                    ))),
                }
            }
        }
    }
}

fn l_gsub(
    lua: &Lua,
    (s, pattern, repl, n): (String, String, Value, Option<LuaInteger>),
) -> LuaResult<(String, usize)> {
    let pattern =
        Pattern::parse(&pattern).map_err(|_| mlua::Error::runtime("malformed pattern"))?;

    let replacement = match repl {
        Value::String(repl_str) => LuaReplacement::String(
            ReplacementString::parse(&repl_str.to_str()?, pattern.capture_count() as u8).map_err(
                |e| match e {
                    ReplacementError::InvalidReference => {
                        mlua::Error::runtime("invalid capture index")
                    }
                    ReplacementError::InvalidEscape => {
                        mlua::Error::runtime("invalid use of '%' in replacement string")
                    }
                },
            )?,
        ),
        Value::Table(table) => LuaReplacement::Table(table),
        Value::Function(function) => LuaReplacement::Function(function),
        _ => return Err(mlua::Error::runtime("string/function/table expected")),
    };

    let limit = n.map(|n| {
        if n <= 0 {
            0
        } else {
            usize::try_from(n).unwrap_or(usize::MAX)
        }
    });

    replace_with(&s, &pattern, |m: &Match| replacement.apply(lua, m), limit)
}

fn l_len(
    lua: &Lua,
    (s, i, j, lax): (
        LuaString,
        Option<LuaInteger>,
        Option<LuaInteger>,
        Option<bool>,
    ),
) -> LuaResult<MultiValue> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let strict = !lax.unwrap_or(false);
    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    let j = j.map_or(len, |j| normalize_lua_index(j, len));

    if !(1 <= i && i - 1 <= len) {
        return Err(bad_argument(
            "len",
            2,
            mlua::Error::runtime("initial position out of bounds"),
        ));
    }

    if !(j <= len) {
        return Err(bad_argument(
            "len",
            3,
            mlua::Error::runtime("final position out of bounds"),
        ));
    }

    let start = i - 1;
    let end = j;

    let mut pos = start;
    let mut result = 0;
    for chunk in bytes[start..end].utf8_chunks() {
        for ch in chunk.valid().chars() {
            result += 1;
            pos += ch.len_utf8();
        }
        let invalid = chunk.invalid();
        if !invalid.is_empty() {
            if strict {
                return (mlua::Nil, pos + 1).into_lua_multi(lua);
            }
            result += 1;
            pos += invalid.len();
        }
    }

    (result,).into_lua_multi(lua)
}

fn l_match(
    lua: &Lua,
    (s, pattern, init): (String, String, Option<LuaInteger>),
) -> LuaResult<MultiValue> {
    let init = init.map_or(1, |x| if x == 0 { 1 } else { x });
    let start_byte = match char_pos(s.as_bytes(), init as isize) {
        Some(start) => start,
        None if init <= 0 => 1,
        None => return (mlua::Value::Nil,).into_lua_multi(lua),
    };

    let p = Pattern::parse(&pattern).map_err(|_| mlua::Error::runtime("malformed pattern"))?;
    match find(&s, &p, Some(start_byte)) {
        Some(m) => Ok(MultiValue::from_iter(
            m.values().map(|c| capture_value_to_lua(lua, c)),
        )),
        None => (mlua::Value::Nil,).into_lua_multi(lua),
    }
}

fn l_reverse(lua: &Lua, (s, lax): (LuaString, Option<bool>)) -> LuaResult<LuaString> {
    let bytes = s.as_bytes();
    let strict = !lax.unwrap_or(false);
    let mut result = vec![0u8; bytes.len()];
    let mut pos = bytes.len();

    for chunk in bytes.utf8_chunks() {
        for ch in chunk.valid().chars() {
            let len = ch.len_utf8();
            pos -= len;
            ch.encode_utf8(&mut result[pos..pos + len]);
        }
        let invalid = chunk.invalid();
        if !invalid.is_empty() {
            if strict {
                return Err(mlua::Error::runtime("invalid UTF-8 code"));
            }
            pos -= invalid.len();
            result[pos..pos + invalid.len()].copy_from_slice(invalid);
        }
    }

    lua.create_string(result)
}

fn l_sub(
    _lua: &Lua,
    (s, start, end): (String, LuaInteger, Option<LuaInteger>),
) -> LuaResult<String> {
    let len = s.chars().count() as LuaInteger;

    let normalize = |idx: LuaInteger| if idx >= 0 { idx } else { len + idx + 1 };
    let i = normalize(start).max(1);
    let j = normalize(end.unwrap_or(-1)).min(len);

    if i > j {
        Ok(String::new())
    } else {
        Ok(s.chars()
            .skip((i - 1) as usize)
            .take((j - i + 1) as usize)
            .collect())
    }
}

fn l_escape(_lua: &Lua, s: String) -> LuaResult<String> {
    let mut chars = s.chars().peekable();
    let mut result = String::with_capacity(s.len());

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        match chars.peek() {
            Some(d) if d.is_ascii_digit() || d == &'{' => {
                result.push(parse_escaped_codepoint(&mut chars, 10)?);
            }
            Some('u' | 'U') => {
                chars.next();
                result.push(parse_escaped_codepoint(&mut chars, 10)?);
            }
            Some('x' | 'X') => {
                chars.next();
                result.push(parse_escaped_codepoint(&mut chars, 16)?);
            }
            Some(other) => {
                result.push(*other);
                chars.next();
            }
            None => return Err(mlua::Error::runtime("unfinished escape")),
        }
    }

    Ok(result)
}

/// Parses either "d+" or "{d+}" into a `char`. `radix` selects the base of digit "d".
fn parse_escaped_codepoint(chars: &mut Peekable<Chars<'_>>, radix: u32) -> LuaResult<char> {
    let braced = chars.next_if_eq(&'{').is_some();
    let mut value: u32 = 0;
    let mut digits = 0;

    loop {
        match chars.peek() {
            Some(c) if c.is_digit(radix) => {
                value = value
                    .checked_mul(radix)
                    .and_then(|v| v.checked_add(c.to_digit(radix).expect("c is a digit")))
                    .ok_or_else(|| mlua::Error::runtime("invalid codepoint"))?;
                digits += 1;
                chars.next();
            }
            // braced form: only `}` ends the sequence
            Some('}') if braced => {
                chars.next();
                break;
            }
            Some(c) if braced => return Err(mlua::Error::runtime(format!("invalid escape '{c}'"))),
            None if braced => return Err(mlua::Error::runtime("unfinished escape")),
            // unbraced form: stop at the first non-digit or end
            _ => break,
        }
    }

    if digits == 0 {
        return Err(mlua::Error::runtime("invalid escape: expected digit"));
    }

    char::from_u32(value).ok_or_else(|| mlua::Error::runtime("invalid codepoint"))
}

fn l_charpos(
    lua: &Lua,
    (s, i_or_n, n): (LuaString, Option<LuaInteger>, Option<LuaInteger>),
) -> LuaResult<MultiValue> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;

    let (search, base, from_end, index) = match n {
        None => {
            let n = i_or_n.unwrap_or(0);
            match n {
                0 => (&s[..], 0, false, 0),
                n if n > 0 => (&s[..], 0, false, (n - 1) as usize),
                n => (&s[..], 0, true, (-n - 1) as usize),
            }
        }
        Some(n) => {
            let i = i_or_n.unwrap_or(1);
            let pos = normalize_lua_index(i, s.len()).max(1);
            let pos = pos - 1;

            match n {
                0 => {
                    let start = s.floor_char_boundary(pos);
                    (&s[start..], start, false, 0)
                }
                n if n > 0 => {
                    let start = s.ceil_char_boundary(pos);
                    let index = if start == pos {
                        n as usize
                    } else {
                        (n - 1) as usize
                    };

                    (&s[start..], start, false, index)
                }
                n => {
                    let end = s.ceil_char_boundary(pos);
                    (&s[..end], 0, true, (-n - 1) as usize)
                }
            }
        }
    };

    let char_index = if from_end {
        search.char_indices().nth_back(index)
    } else {
        search.char_indices().nth(index)
    };

    if let Some((offset, ch)) = char_index {
        (base + offset + 1, ch as LuaInteger).into_lua_multi(lua)
    } else {
        mlua::Nil.into_lua_multi(lua)
    }
}

fn l_next(
    lua: &Lua,
    (s, o, i): (LuaString, Option<LuaInteger>, Option<LuaInteger>),
) -> LuaResult<MultiValue> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let offset = o.map_or(1, |offset| normalize_lua_index(offset, len));
    let index = i.unwrap_or_else(|| if o.is_some() { 1 } else { 0 });

    let position = if index == 0 {
        char_start(&bytes, offset)
    } else {
        move_by_chars(&bytes, offset.max(1), index)
    };

    let Some(position) = position else {
        return mlua::Nil.into_lua_multi(lua);
    };

    let cp = match decode_utf8(&bytes[position..]) {
        Ok((cp, _)) => cp,
        Err(_) => 0,
    };

    (cp, position + 1).into_lua_multi(lua)
}

fn next_char_pos(bytes: &[u8]) -> Option<usize> {
    bytes
        .iter()
        .enumerate()
        .filter(|&(_, b)| b & 0b1100_0000 != 0b1000_0000)
        .map(|(i, _)| i)
        .nth(1)
}

fn char_pos(bytes: &[u8], n: isize) -> Option<usize> {
    if n >= 0 {
        bytes
            .iter()
            .enumerate()
            .filter(|&(_, b)| b & 0b1100_0000 != 0b1000_0000)
            .map(|(i, _)| i)
            // char right after the end returns the length
            .chain(std::iter::once(bytes.len()))
            .nth((n - 1) as usize)
    } else {
        bytes
            .iter()
            .enumerate()
            .rev()
            .filter(|&(_, b)| b & 0b1100_0000 != 0b1000_0000)
            .nth((-n - 1) as usize)
            .map(|(i, _)| i)
    }
}

fn l_insert(lua: &Lua, (s, args): (LuaString, MultiValue)) -> LuaResult<LuaString> {
    let mut args = args.into_iter();
    let arg2 = args.next();
    let (idx, subs_arg, subs_pos): (Option<LuaInteger>, Option<Value>, usize) = match &arg2 {
        Some(Value::Integer(i)) => (Some(*i), args.next(), 3),
        Some(Value::Number(n)) => (Some(*n as LuaInteger), args.next(), 3),
        _ => (None, arg2, 2),
    };
    let subs = match subs_arg {
        Some(Value::String(s)) => s.as_bytes(),
        Some(other) => {
            return Err(bad_argument(
                "insert",
                subs_pos,
                mlua::Error::runtime(format!("string expected, got {}", other.type_name())),
            ));
        }
        None => {
            return Err(bad_argument(
                "insert",
                subs_pos,
                mlua::Error::runtime("string expected, got no value"),
            ));
        }
    };

    let s = s.as_bytes();
    let mut buf = Vec::with_capacity(s.len() + subs.len());
    if let Some(idx) = idx {
        let at = if idx == 0 {
            s.len()
        } else {
            char_pos(&s, idx as isize)
                .ok_or_else(|| bad_argument("insert", 2, mlua::Error::runtime("invalid index")))?
        };
        buf.extend_from_slice(&s[..at]);
        buf.extend_from_slice(&subs);
        buf.extend_from_slice(&s[at..]);
    } else {
        buf.extend_from_slice(&s);
        buf.extend_from_slice(&subs);
    }

    lua.create_string(buf)
}

fn l_remove(
    lua: &Lua,
    (s, i, j): (LuaString, Option<LuaInteger>, Option<LuaInteger>),
) -> LuaResult<LuaString> {
    let bytes = s.as_bytes();
    let i = i.unwrap_or(-1);
    let j = j.unwrap_or(-1);

    let start = char_pos(&bytes, i as isize).unwrap_or_else(|| if i > 0 { bytes.len() } else { 0 });
    let end = char_pos(&bytes, j as isize).map_or_else(
        || if i > 0 { bytes.len() } else { 0 },
        |end| next_char_pos(&bytes[end..]).map_or(bytes.len(), |offset| end + offset),
    );

    if start > end {
        return Ok(s);
    }

    let mut buf = Vec::with_capacity(bytes.len());
    buf.extend_from_slice(&bytes[..start]);
    buf.extend_from_slice(&bytes[end..]);

    lua.create_string(buf)
}

fn ch_width(ch: char, ambi_width: usize, default_width: usize) -> usize {
    if CodePointSetData::new::<DefaultIgnorableCodePoint>().contains(ch) {
        return default_width;
    }

    if CodePointSetData::new::<GraphemeExtend>().contains(ch) {
        return default_width;
    }

    match CodePointMapData::<EastAsianWidth>::new().get(ch) {
        EastAsianWidth::Wide | EastAsianWidth::Fullwidth => 2,
        EastAsianWidth::Ambiguous => ambi_width,
        _ => 1,
    }
}

fn next_optional<T: FromLua>(
    lua: &Lua,
    args: &mut impl Iterator<Item = Value>,
) -> LuaResult<Option<T>> {
    match args.next() {
        None | Some(Value::Nil) => Ok(None),
        Some(value) => T::from_lua(value, lua).map(Some),
    }
}

fn l_width(lua: &Lua, args: MultiValue) -> LuaResult<LuaInteger> {
    let mut args = args.into_iter();

    let first_arg = args
        .next()
        .ok_or_else(|| mlua::Error::runtime("expected string or integer"))?;

    match first_arg {
        Value::String(lua_string) => {
            let len = lua_string.as_bytes().len();
            let s = lua_string
                .to_str()
                .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
            let i = next_optional::<LuaInteger>(lua, &mut args)?.unwrap_or(1);
            let j = next_optional::<LuaInteger>(lua, &mut args)?.unwrap_or(len as LuaInteger);
            let ambi_width = next_optional::<LuaInteger>(lua, &mut args)?.unwrap_or(1) as usize;
            let default_width = next_optional::<LuaInteger>(lua, &mut args)?.unwrap_or(0) as usize;

            let i = normalize_lua_index(i, len);
            let j = normalize_lua_index(j, len);

            if !(1 <= i && i - 1 <= len) {
                return Err(bad_argument(
                    "width",
                    2,
                    mlua::Error::runtime("initial position out of bounds"),
                ));
            }

            if !(j <= len) {
                return Err(bad_argument(
                    "width",
                    3,
                    mlua::Error::runtime("final position out of bounds"),
                ));
            }

            if i > j {
                return Ok(0);
            }

            let start = i - 1;
            let end = j;

            let total = s
                .get(start..end)
                .ok_or_else(|| mlua::Error::runtime("invalid UTF-8 code"))?
                .chars()
                .fold(0, |acc, ch| acc + ch_width(ch, ambi_width, default_width));

            Ok(total as LuaInteger)
        }
        Value::Integer(codepoint) => {
            let ambi_width = next_optional::<LuaInteger>(lua, &mut args)?.unwrap_or(1) as usize;
            let default_width = next_optional::<LuaInteger>(lua, &mut args)?.unwrap_or(0) as usize;

            let Some(ch) = char::from_u32(codepoint as u32) else {
                return Ok(1);
            };

            Ok(ch_width(ch, ambi_width, default_width) as LuaInteger)
        }
        other => Err(mlua::Error::runtime(format!(
            "number/string expected, got {}",
            other.type_name()
        ))),
    }
}

fn l_widthindex(
    lua: &Lua,
    (s, width, i, j, ambi_width, default_width): (
        LuaString,
        LuaInteger,
        Option<LuaInteger>,
        Option<LuaInteger>,
        Option<LuaInteger>,
        Option<LuaInteger>,
    ),
) -> LuaResult<MultiValue> {
    let len = s.as_bytes().len();
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;

    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    let j = j.map_or(len, |j| normalize_lua_index(j, len));
    let ambi_width = ambi_width.unwrap_or(1) as usize;
    let default_width = default_width.unwrap_or(0) as usize;

    if !(1 <= i && i - 1 <= len) {
        return Err(bad_argument(
            "widthindex",
            3,
            mlua::Error::runtime("initial position out of bounds"),
        ));
    }

    if !(j <= len) {
        return Err(bad_argument(
            "widthindex",
            4,
            mlua::Error::runtime("final position out of bounds"),
        ));
    }

    if i > j {
        return 0.into_lua_multi(lua);
    }

    let start = i - 1;
    let end = j;

    let chars = s
        .get(start..end)
        .ok_or_else(|| mlua::Error::runtime("invalid UTF-8 code"))?
        .chars();

    let mut index: LuaInteger = 0;
    let mut width = width;
    for ch in chars {
        let ch_width = ch_width(ch, ambi_width, default_width);

        if width <= ch_width as LuaInteger {
            return (index + 1, width, ch_width).into_lua_multi(lua);
        }

        index += 1;
        width -= ch_width as LuaInteger;
    }

    index.into_lua_multi(lua)
}

fn l_widthlimit(
    lua: &Lua,
    (s, limit, i, j, ambi_width, default_width): (
        LuaString,
        LuaInteger,
        Option<LuaInteger>,
        Option<LuaInteger>,
        Option<LuaInteger>,
        Option<LuaInteger>,
    ),
) -> LuaResult<MultiValue> {
    let len = s.as_bytes().len();
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;

    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    let j = j.map_or(len, |j| normalize_lua_index(j, len));
    let ambi_width = ambi_width.unwrap_or(1) as usize;
    let default_width = default_width.unwrap_or(0) as usize;

    if !(1 <= i && i - 1 <= len) {
        return Err(bad_argument(
            "widthlimit",
            3,
            mlua::Error::runtime("initial position out of bounds"),
        ));
    }

    if !(j <= len) {
        return Err(bad_argument(
            "widthlimit",
            4,
            mlua::Error::runtime("final position out of bounds"),
        ));
    }

    if i > j {
        if limit >= 0 {
            return (i - 1, limit).into_lua_multi(lua);
        } else {
            return (j + 1, limit).into_lua_multi(lua);
        }
    }

    let start = i - 1;
    let end = j;
    let mut width = limit;
    let chars = s
        .get(start..end)
        .ok_or_else(|| mlua::Error::runtime("invalid UTF-8 code"))?
        .char_indices();

    let mut pos = 0;

    let index = if width >= 0 {
        for (byte_pos, ch) in chars {
            let ch_width = ch_width(ch, ambi_width, default_width);
            if width < ch_width as LuaInteger {
                break;
            }

            width -= ch_width as LuaInteger;
            pos = byte_pos + ch.len_utf8();

            if width == 0 {
                break;
            }
        }

        start + pos
    } else {
        let mut pos = end - start;

        for (byte_pos, ch) in chars.rev() {
            let ch_width = ch_width(ch, ambi_width, default_width);
            if -width < ch_width as LuaInteger {
                break;
            }

            width += ch_width as LuaInteger;
            pos = byte_pos;

            if width == 0 {
                break;
            }
        }

        start + pos + 1
    };

    (index, width).into_lua_multi(lua)
}

fn l_ncasecmp(_lua: &Lua, (a, b): (LuaString, LuaString)) -> LuaResult<LuaInteger> {
    let a = a
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
    let b = b
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;

    let mapper = CaseMapper::new();
    let ordering = a
        .chars()
        .map(|ch| mapper.simple_fold(ch))
        .cmp(b.chars().map(|ch| mapper.simple_fold(ch)));

    Ok(match ordering {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    })
}

fn l_isvalid(_lua: &Lua, s: LuaString) -> LuaResult<bool> {
    Ok(s.to_str().is_ok())
}

fn l_clean(
    lua: &Lua,
    (s, replacement): (LuaString, Option<LuaString>),
) -> LuaResult<(LuaString, bool)> {
    let replacement = replacement
        .map(|repl| repl.to_str())
        .transpose()
        .map_err(|_| mlua::Error::runtime("replacement string must be valid UTF-8"))?;
    let repl = replacement.as_deref().unwrap_or("\u{FFFD}");

    let bytes = s.as_bytes();
    let mut iter = bytes.utf8_chunks();

    let Some(first_chunk) = iter.next() else {
        // string is empty
        return Ok((s, true));
    };

    if first_chunk.invalid().is_empty() {
        // entire string is valid
        return Ok((s, true));
    }

    let mut res = String::with_capacity(bytes.len());
    res.push_str(first_chunk.valid());
    let mut in_invalid_run = true;

    for chunk in iter {
        if !chunk.valid().is_empty() {
            if in_invalid_run {
                res.push_str(repl);
                in_invalid_run = false;
            }

            res.push_str(chunk.valid());
        }

        if !chunk.invalid().is_empty() {
            in_invalid_run = true;
        }
    }

    if in_invalid_run {
        res.push_str(repl);
    }

    lua.create_string(res).map(|res| (res, false))
}

fn l_invalidoffset(lua: &Lua, (s, i): (LuaString, Option<LuaInteger>)) -> LuaResult<Value> {
    let len = s.as_bytes().len();
    let start = i.map_or(1, |i| {
        (if i >= 0 { i } else { len as LuaInteger + i + 1 }).max(1)
    });
    let start = (start - 1) as usize; // translate to 0-based index
    if start > len {
        return Ok(mlua::Nil);
    }
    match std::str::from_utf8(&s.as_bytes()[start..]) {
        Ok(_) => Ok(mlua::Nil),
        // translate to 1-based index and return
        Err(e) => (start + e.valid_up_to() + 1).into_lua(lua),
    }
}

fn l_isnfc(_lua: &Lua, s: LuaString) -> LuaResult<bool> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("string is not valid UTF-8"))?;
    Ok(is_nfc(&s))
}

fn l_normalize_nfc(lua: &Lua, s: LuaString) -> LuaResult<(LuaString, bool)> {
    let borrowed_str = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("string is not valid UTF-8"))?;
    if is_nfc(&borrowed_str) {
        Ok((s, true))
    } else {
        let normalized = borrowed_str.chars().nfc().collect::<String>();
        lua.create_string(&normalized).map(|norm| (norm, false))
    }
}

fn l_grapheme_indices(
    lua: &Lua,
    (s, i, j): (LuaString, Option<LuaInteger>, Option<LuaInteger>),
) -> LuaResult<Function> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
    let len = s.len();
    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    let j = j.map_or(len, |j| normalize_lua_index(j, len));

    if !(i >= 1) {
        return Err(bad_argument(
            "grapheme_indices",
            2,
            mlua::Error::runtime("out of range"),
        ));
    }
    if !(j <= len) {
        return Err(bad_argument(
            "grapheme_indices",
            3,
            mlua::Error::runtime("out of range"),
        ));
    }

    let start = i - 1;
    let end = j;

    if !s.is_char_boundary(start) {
        return Err(mlua::Error::runtime("invalid UTF-8 code"));
    }

    let mut pos = start;
    lua.create_function_mut(move |lua, ()| {
        if pos >= end {
            return Value::Nil.into_lua_multi(lua);
        }

        match s[pos..].grapheme_indices(true).next() {
            Some((offset, grapheme)) => {
                let grapheme_start = pos + offset;
                let grapheme_stop = grapheme_start + grapheme.len();
                pos = grapheme_stop;

                (grapheme_start + 1, grapheme_stop).into_lua_multi(lua)
            }
            None => Value::Nil.into_lua_multi(lua),
        }
    })
}

macro_rules! make_case_mapper {
    ($fn_name:ident, $variant:ident) => {
        fn $fn_name(lua: &Lua, s: Value) -> LuaResult<Value> {
            match s {
                Value::Integer(n) => {
                    let Some(ch) = char::from_u32(n as u32) else {
                        return (n as LuaInteger).into_lua(lua);
                    };

                    let mapper = CaseMapper::new();
                    (mapper.$variant(ch) as LuaInteger).into_lua(lua)
                }
                Value::String(lua_string) => {
                    let s = lua_string
                        .to_str()
                        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;

                    let mapper = CaseMapper::new();
                    let mut result = String::with_capacity(s.len());

                    for ch in s.chars() {
                        result.push(mapper.$variant(ch));
                    }

                    result.into_lua(lua)
                }
                other => Err(mlua::Error::runtime(format!(
                    "number/string expected, got {}",
                    other.type_name()
                ))),
            }
        }
    };
}

make_case_mapper!(l_lower, simple_lowercase);
make_case_mapper!(l_upper, simple_uppercase);
make_case_mapper!(l_title, simple_titlecase);
make_case_mapper!(l_fold, simple_fold);

#[cfg(feature = "module")]
#[mlua::lua_module]
fn luautf8(lua: &Lua) -> LuaResult<Table> {
    create_module(lua)
}

#[cfg(test)]
mod tests {
    use super::create_module;

    #[test]
    fn api_is_compatibile() -> Result<(), mlua::Error> {
        let lua = mlua::Lua::new();
        let module = create_module(&lua).unwrap();
        lua.register_module("lua-utf8", module).unwrap();
        lua.load(std::path::Path::new("tests/compat.lua")).exec()
    }
}
