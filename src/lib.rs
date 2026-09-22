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

mod matching;
mod pattern;
mod replacement;

use std::{iter::Peekable, str::Chars};

use mlua::{
    Function, Integer as LuaInteger, IntoLua, IntoLuaMulti, Lua, LuaString, MultiValue,
    Result as LuaResult, Table, Value, Variadic,
};
use unicode_normalization::{UnicodeNormalization, is_nfc};
use unicode_segmentation::UnicodeSegmentation;

use crate::{matching::Match, pattern::Pattern, replacement::ReplacementString};

// TODO: pattern depends on lua version
const CHAR_PATTERN: &[u8] = b"[\0-\x7F\xC2-\xF4][\x80-\xBF]*";
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

/// Resolves an index relative to a sequence of length `len`.
///
/// Positive indices are returned unchanged. Negative indices count backward from the end, with
/// `-1` referring to the last position. A negative index that refers before the first position
/// results in `0`.
fn normalize_lua_index(pos: i32, len: usize) -> usize {
    if pos >= 0 {
        pos as usize
    } else if pos.unsigned_abs() as usize > len {
        0
    } else {
        (len as i32 + pos + 1) as usize
    }
}

/// Returns the position (in bytes) where the encoding of the n-th character of s (counting from
/// position i) starts. A negative n gets characters before position i. The default for i is 1 when
/// n is non-negative and #s + 1 otherwise, so that utf8.offset(s, -n) gets the offset of the n-th
/// character from the end of the string. If the specified character is neither in the subject nor
/// right after its end, the function returns nil.
///
/// As a special case, when n is 0 the function returns the start of the encoding of the character
/// that contains the i-th byte of s. This function assumes that s is a valid UTF-8 string.
fn l_offset(lua: &Lua, (s, n, i): (String, i32, Option<i32>)) -> LuaResult<MultiValue> {
    let len = s.len();
    let i = match i {
        Some(i) if i < 0 => len as i32 + i + 1,
        Some(i) => i,
        None if n >= 0 => 1,
        None => len as i32 + 1,
    };

    if !(1..=len as i32 + 1).contains(&i) {
        // TODO: use mlua::Error:BadArgument?
        return Err(mlua::Error::runtime("bad argument: position out of range"));
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
        _ => unreachable!("i32 must be zero, positive, or negative"),
    };

    let Some(start) = start else {
        return mlua::Value::Nil.into_lua_multi(lua);
    };

    let end = s
        .get(start..)
        .and_then(|rest| rest.chars().next())
        .map_or(start, |c| start + c.len_utf8());

    // translate back to 1-based indices and return
    (start + 1, end + 1).into_lua_multi(lua)
}

/// Returns the code points of the substring starting at position `i` and ending at `j` (both
/// inclusive, default 1 and i). When `lax` is true, invalid code points such as surrogates are
/// returned instead of raising an error.
/// NOTE: orignal luautf8 docs claim `j` defaults to `#s` but it actually defaults to `i`.
fn l_codepoint(
    _lua: &Lua,
    (s, i, j, lax): (LuaString, Option<i32>, Option<i32>, Option<bool>),
) -> LuaResult<Variadic<u32>> {
    let bytes = s.as_bytes().to_vec();
    let len = bytes.len();
    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    let j = j.map_or(i, |j| normalize_lua_index(j, len));
    let lax = lax.unwrap_or(false);

    // TODO: use mlua::Error::BadArgument
    if !(i >= 1) {
        return Err(mlua::Error::runtime("bad argument: out of bounds"));
    }
    if !(j <= len) {
        return Err(mlua::Error::runtime("bad argument: out of bounds"));
    }

    // TODO: deal with ranges that are too long

    if i > j {
        return Ok(Variadic::new());
    }

    let start = (i - 1) as usize;
    let end = j as usize;
    let mut result = Variadic::new();
    let mut pos = start;
    while pos < end {
        let Some((_, ch)) = next_char(&bytes, &mut pos, lax)
            .map_err(|_| mlua::Error::runtime("invalid UTF-8 codepoint"))?
        else {
            break;
        };
        result.push(ch as u32);
    }

    Ok(result)
}

/// Returns an iterator over of `s` that returns the position (in bytes) and codepoint of each
/// UTF-8 character. When lax is true, invalid sequences are skipped instead of raising an error.
fn l_codes(lua: &Lua, (s, lax): (LuaString, Option<bool>)) -> LuaResult<Function> {
    let bytes = s.as_bytes().to_vec();
    let lax = lax.unwrap_or(false);

    let mut pos = 0;
    lua.create_function_mut(move |lua, ()| match next_char(&bytes, &mut pos, lax) {
        Ok(Some((pos, ch))) => (pos + 1, ch as u32).into_lua_multi(lua),
        Ok(None) => (Value::Nil,).into_lua_multi(lua),
        Err(_) => Err(mlua::Error::runtime("invalid UTF-8 codepoint")),
    })
}

fn next_char(
    bytes: &[u8],
    pos: &mut usize,
    lax: bool,
) -> Result<Option<(usize, char)>, std::str::Utf8Error> {
    while *pos < bytes.len() {
        let start = *pos;
        let end = (start + 4).min(bytes.len());

        match std::str::from_utf8(&bytes[start..end]) {
            Ok(s) => {
                let ch = s.chars().next().unwrap();
                *pos += ch.len_utf8();
                return Ok(Some((start, ch)));
            }
            Err(e) => {
                let valid = e.valid_up_to();

                if valid > 0 {
                    let ch = std::str::from_utf8(&bytes[start..start + valid])
                        .unwrap()
                        .chars()
                        .next()
                        .unwrap();

                    *pos += ch.len_utf8();
                    return Ok(Some((start, ch)));
                }

                if !lax {
                    return Err(e);
                }

                *pos += e.error_len().unwrap_or(1);
            }
        }
    }

    Ok(None)
}

/// Returns the internal numeric codes of the characters of `s`.
fn l_byte(
    _lua: &Lua,
    (s, start, end): (LuaString, Option<i32>, Option<i32>),
) -> LuaResult<Variadic<i32>> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
    let len = s.chars().count() as i32;

    let normalize = |idx: i32| if idx >= 0 { idx } else { len + idx + 1 };
    let i = normalize(start.unwrap_or(1)).max(1);
    let j = normalize(end.unwrap_or(start.unwrap_or(1))).min(len);

    if i > j {
        Ok(Variadic::new())
    } else {
        Ok(s.chars()
            .skip((i - 1) as usize)
            .take((j - i + 1) as usize)
            .map(|ch| ch as i32)
            .collect())
    }
}

/// Returns a string with each argument converted to a UTF-8 byte sequence.
fn l_char(_lua: &Lua, args: Variadic<LuaInteger>) -> LuaResult<String> {
    let mut result = String::with_capacity(args.len());
    for arg in args {
        // TODO: use `BadArgument` error
        let ch = char::from_u32(arg as u32).ok_or(mlua::Error::runtime("value out of range"))?;
        result.push(ch);
    }
    Ok(result)
}

/// Finds the first occurrence of `pattern` in `s`. Returns nil if not found.
fn l_find(
    lua: &Lua,
    (s, pattern, _init, _plain): (String, String, Option<i32>, Option<bool>),
) -> LuaResult<MultiValue> {
    // TODO: check if error messages are the same
    // TODO: init
    // TODO: plain
    let p = Pattern::parse(&pattern).map_err(|_| mlua::Error::runtime("malformed pattern"))?;
    match p.find(&s) {
        Some(m) if m.capture_count() > 0 => {
            let captures = m
                .captures()
                .map(|s| s.into_lua(lua))
                .collect::<LuaResult<Vec<_>>>()?;
            (m.start() + 1, m.end(), MultiValue::from_vec(captures)).into_lua_multi(lua)
        }
        Some(m) => {
            // no captures so just return the entire match
            (m.start() + 1, m.end(), m.as_str().to_string()).into_lua_multi(lua)
        }
        None => (mlua::Value::Nil,).into_lua_multi(lua),
    }
}

fn l_gmatch(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

enum LuaReplacement {
    String(ReplacementString),
    Table(mlua::Table),
    Function(mlua::Function),
}

/// Replaces every occurrence of `pattern` in `s` with `repl`, returning the new string and the
/// number of substitutions. A maximum of `n` (default: unlimited) replacements will be performed.
fn l_gsub(
    lua: &Lua,
    (s, pattern, repl, n): (String, String, Value, Option<i32>),
) -> LuaResult<(String, usize)> {
    let pattern =
        Pattern::parse(&pattern).map_err(|_| mlua::Error::runtime("malformed pattern"))?;
    let replacement = match repl {
        Value::String(s) => LuaReplacement::String(
            ReplacementString::parse(&s.to_str()?, pattern.capture_count() as u8).map_err(|e| {
                match e {
                    replacement::ReplacementError::InvalidReference => {
                        mlua::Error::runtime("invalid capture index")
                    }
                    replacement::ReplacementError::InvalidEscape => {
                        mlua::Error::runtime("invalid use of '%' in replacement string")
                    }
                }
            })?,
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

    let replace = |m: &Match| match &replacement {
        LuaReplacement::String(replacement_string) => Ok(Some(replacement_string.apply(m))),
        LuaReplacement::Table(table) => {
            // use first capture as key, or whole match if no captures
            let key = m.capture(0).unwrap_or_else(|| m.as_str());
            let value = table.get(key)?;
            match value {
                Value::String(string) => Ok(Some(string.to_str()?.to_owned())),
                Value::Nil | Value::Boolean(false) => Ok(None),
                other => Err(mlua::Error::runtime(format!(
                    "invalid replacement value (a {})",
                    other.type_name()
                ))),
            }
        }
        LuaReplacement::Function(function) => {
            // use all captures as function arguments, or whole match if no captures
            let args: Variadic<LuaString> = if m.capture_count() == 0 {
                Variadic::from(vec![lua.create_string(m.as_str())?])
            } else {
                Variadic::from(
                    m.captures()
                        .map(|capture| lua.create_string(capture))
                        .collect::<LuaResult<Vec<_>>>()?,
                )
            };

            let value = function.call(args)?;

            match value {
                Value::String(string) => Ok(Some(string.to_str()?.to_owned())),
                Value::Nil | Value::Boolean(false) => Ok(None),
                other => Err(mlua::Error::runtime(format!(
                    "invalid replacement value (a {})",
                    other.type_name()
                ))),
            }
        }
    };

    pattern.replace(&s, replace, limit)
}

fn l_len(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

/// Converts `s` to lowercase. With an integer argument, converts a code point.
fn l_lower(lua: &Lua, s: Value) -> LuaResult<Value> {
    match s {
        Value::Integer(n) => {
            let ch = char::from_u32(n as u32)
                .ok_or_else(|| mlua::Error::runtime("invalid Unicode codepoint"))?;
            // NOTE: the lowercase mapping can expand to multiple `char`s.
            // `luautf8` only returns one codepoint so do the same here.
            (ch.to_lowercase().next().unwrap() as i64).into_lua(lua)
        }
        Value::String(lua_string) => lua_string.to_str()?.to_lowercase().into_lua(lua),
        other => Err(mlua::Error::runtime(format!(
            "number/string expected, got {}",
            other.type_name()
        ))),
    }
}

/// Matches pattern in `s`, returning the captures (or the whole match).
fn l_match(lua: &Lua, (s, pattern, _init): (String, String, Option<i32>)) -> LuaResult<MultiValue> {
    // TODO: use same error msgs?
    // TODO: init (start position)
    let p = Pattern::parse(&pattern).map_err(|_| mlua::Error::runtime("malformed pattern"))?;
    match p.find(&s) {
        Some(m) if m.capture_count() > 0 => {
            let captures = m
                .captures()
                .map(|s| s.into_lua(lua))
                .collect::<LuaResult<Vec<_>>>()?;
            Ok(MultiValue::from_vec(captures))
        }
        Some(m) => {
            // no captures so just return the entire match
            (m.as_str().to_string(),).into_lua_multi(lua)
        }
        None => (mlua::Value::Nil,).into_lua_multi(lua),
    }
}

/// Returns the reverse of `s`. Reverses by character, not by byte. When `lax` is true, invalid
/// sequences are reversed leniently without raising an error.
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

/// Returns the substring of `s` starting at `start` and ending at `end`.
fn l_sub(_lua: &Lua, (s, start, end): (String, i32, Option<i32>)) -> LuaResult<String> {
    let len = s.chars().count() as i32;

    let normalize = |idx: i32| if idx >= 0 { idx } else { len + idx + 1 };
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

/// Converts `s` to uppercase. With an integer argument, converts a code point.
fn l_upper(lua: &Lua, s: Value) -> LuaResult<Value> {
    match s {
        Value::Integer(n) => {
            let ch = char::from_u32(n as u32)
                .ok_or_else(|| mlua::Error::runtime("invalid Unicode codepoint"))?;
            // NOTE: the uppercase mapping can expand to multiple `char`s.
            // `luautf8` only returns one codepoint so do the same here.
            (ch.to_uppercase().next().unwrap() as i64).into_lua(lua)
        }
        Value::String(lua_string) => lua_string.to_str()?.to_uppercase().into_lua(lua),
        other => Err(mlua::Error::runtime(format!(
            "number/string expected, got {}",
            other.type_name()
        ))),
    }
}

/// Escapes `s` to UTF-8 format (supports %ddd, %{ddd}, %uddd, %u{ddd}, %xhhh, %x{hhh}, and %? for
/// any other character).
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
                result.push(parse_escaped_codepoint(&mut chars, 10)?)
            }
            Some('u' | 'U') => {
                chars.next();
                result.push(parse_escaped_codepoint(&mut chars, 10)?)
            }
            Some('x' | 'X') => {
                chars.next();
                result.push(parse_escaped_codepoint(&mut chars, 16)?)
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
pub fn parse_escaped_codepoint(chars: &mut Peekable<Chars<'_>>, radix: u32) -> LuaResult<char> {
    let braced = chars.next_if_eq(&'{').is_some();
    let mut value: u32 = 0;
    let mut digits = 0;

    loop {
        match chars.peek() {
            Some(c) if c.is_digit(radix) => {
                value = value
                    .checked_mul(radix)
                    .and_then(|v| v.checked_add(c.to_digit(radix).expect("c is a digit")))
                    .ok_or(mlua::Error::runtime("invalid codepoint"))?;
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

    char::from_u32(value).ok_or(mlua::Error::runtime("invalid codepoint"))
}

/// Converts UTF-8 character position `n` to byte position, also returning the code point at the
/// resulting position. Assumes `s` is valid UTF-8. With one optional argument it is `n`; with two
/// optional arguments they are `i` (byte position) and `n` (character count after `i`).
fn l_charpos(
    lua: &Lua,
    (s, i_or_n, n): (LuaString, Option<i32>, Option<i32>),
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
        (base + offset + 1, ch as u32).into_lua_multi(lua)
    } else {
        mlua::Nil.into_lua_multi(lua)
    }
}

fn l_next(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_insert(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_remove(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_width(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_widthindex(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_widthlimit(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_title(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_fold(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_ncasecmp(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

/// Returns `true` if `s` is a valid UTF-8 string.
fn l_isvalid(_lua: &Lua, s: LuaString) -> LuaResult<bool> {
    Ok(s.to_str().is_ok())
}

/// Replaces invalid UTF-8 byte sequences in `s` with replacement (default: U+FFFD).
/// Returns the clean string and whether the original string was valid.
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

/// Returns the byte position position within s of the first invalid UTF-8 byte sequence (1 is the
/// first byte of the string). If s is a valid UTF-8 string, returns nil. The default for i is 1.
/// The optional numeric argument i specifies where to start the search and can be negative.
fn l_invalidoffset(lua: &Lua, (s, i): (LuaString, Option<i32>)) -> LuaResult<Value> {
    let len = s.as_bytes().len();
    let start = i.map_or(1, |i| (if i >= 0 { i } else { len as i32 + i + 1 }).max(1));
    let start = (start - 1) as usize; // translate to 0-based index
    match std::str::from_utf8(&s.as_bytes()[start..]) {
        Ok(_) => Ok(mlua::Nil),
        // translate to 1-based index and return
        Err(e) => (start + e.valid_up_to() + 1).into_lua(lua),
    }
}

/// Returns whether `s` is in Normalization Form C (NFC).
/// Raises an error for invalid UTF-8.
fn l_isnfc(_lua: &Lua, s: LuaString) -> LuaResult<bool> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("string is not valid UTF-8"))?;
    Ok(is_nfc(&s))
}

/// Normalizes `s` to NFC and returns the normalized string and whether `s` was already normalized.
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

/// Returns an iterator over grapheme clusters of `s`, yielding the inclusive byte range [from, to]
/// of each cluster.
fn l_grapheme_indices(
    lua: &Lua,
    (s, i, j): (LuaString, Option<i32>, Option<i32>),
) -> LuaResult<Function> {
    let s = s
        .to_str()
        .map_err(|_| mlua::Error::runtime("invalid UTF-8 code"))?;
    let len = s.len();
    let i = i.map_or(1, |i| normalize_lua_index(i, len));
    let j = j.map_or(len, |j| normalize_lua_index(j, len));

    if !(i >= 1) {
        // TODO: use BadArgument
        return Err(mlua::Error::runtime("bad argument: position out of range"));
    }

    if !(j <= len) {
        // TODO: use BadArgument
        return Err(mlua::Error::runtime("bad argument: position out of range"));
    }

    let start = (i - 1) as usize;
    let end = j as usize;

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

#[cfg(feature = "module")]
#[mlua::lua_module]
fn luautf8(lua: &Lua) -> LuaResult<Table> {
    create_module(lua)
}
