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

use std::{iter::Peekable, str::Chars};

use mlua::{
    Integer as LuaInteger, IntoLua, Lua, LuaString, MultiValue, Result as LuaResult, Table, Value,
    Variadic,
};

// TODO: pattern depends on lua version
const CHAR_PATTERN: &[u8] = b"[\0-\x7F\xC2-\xF4][\x80-\xBF]*";
const VERSION: &str = "0.3.0";

/// Creates the module table.
///
/// The returned table can be registered with [`Lua::register_module`]:
///
/// ```no_run
/// # use mlua::{Lua, Result};
/// # use luautf8_rs::create_module;
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

fn l_offset(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_codepoint(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_codes(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_byte(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
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

fn l_find(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_gmatch(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_gsub(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
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

fn l_match(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

/// Returns the reverse of `s`. Reverses by character, not by byte. When `lax` is true, invalid
/// sequences are reversed leniently without raising an error.
fn l_reverse(lua: &Lua, (s, lax): (LuaString, Option<bool>)) -> LuaResult<LuaString> {
    if lax.unwrap_or(false) {
        todo!()
    } else {
        let reversed = s.to_str()?.chars().rev().collect::<String>();
        lua.create_string(reversed)
    }
}

fn l_sub(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
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

fn l_charpos(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
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

fn l_isvalid(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_clean(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_invalidoffset(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_isnfc(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_normalize_nfc(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

fn l_grapheme_indices(_lua: &Lua, _args: MultiValue) -> LuaResult<MultiValue> {
    todo!()
}

#[cfg(feature = "module")]
#[mlua::lua_module]
fn luautf8(lua: &Lua) -> LuaResult<Table> {
    create_module(lua)
}
