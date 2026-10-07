#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

/// Decodes one code point sequence from the start of `bytes` according to RFC 2279.
///
/// Returns the code point and the number of bytes consumed.
/// NOTE: The resulting code point is not validated!
pub fn decode_utf8(bytes: &[u8]) -> Result<(u32, usize), DecodeError> {
    let first = *bytes.first().ok_or(DecodeError)?;

    let (len, initial_bits) = match first {
        0x00..=0x7F => return Ok((first as u32, 1)),
        0xC0..=0xDF => (2, first & 0b0001_1111),
        0xE0..=0xEF => (3, first & 0b0000_1111),
        0xF0..=0xF7 => (4, first & 0b0000_0111),
        0xF8..=0xFB => (5, first & 0b0000_0011),
        0xFC..=0xFD => (6, first & 0b0000_0001),
        _ => return Err(DecodeError),
    };

    let rest = bytes.get(1..len).ok_or(DecodeError)?;

    let mut cp = initial_bits as u32;
    for &b in rest.iter() {
        if b & 0b1100_0000 != 0b1000_0000 {
            return Err(DecodeError);
        }
        cp = (cp << 6) | (b & 0b0011_1111) as u32;
    }

    // reject "overlong" sequences
    const MIN_FOR_LEN: [u32; 7] = [0, 0, 0x80, 0x800, 0x1_0000, 0x20_0000, 0x400_0000];
    if cp < MIN_FOR_LEN[len] {
        return Err(DecodeError);
    }

    Ok((cp, len))
}

/// Encodes one code point according to RFC 2279.
///
/// NOTE: Allows for code points up to `0x7FFFFFFF`.
pub fn encode_utf8(cp: u32, out: &mut Vec<u8>) {
    assert!(cp <= 0x7FFFFFFF);

    match cp {
        0x0000_0000..=0x0000_007F => {
            out.push(cp as u8);
        }
        0x0000_0080..=0x0000_07FF => {
            out.push(0xC0 | (cp >> 6) as u8);
            out.push(0x80 | (cp & 0x3F) as u8);
        }
        0x0000_0800..=0x0000_FFFF => {
            out.push(0xE0 | (cp >> 12) as u8);
            out.push(0x80 | ((cp >> 6) & 0x3F) as u8);
            out.push(0x80 | (cp & 0x3F) as u8);
        }
        0x0001_0000..=0x001F_FFFF => {
            out.push(0xF0 | (cp >> 18) as u8);
            out.push(0x80 | ((cp >> 12) & 0x3F) as u8);
            out.push(0x80 | ((cp >> 6) & 0x3F) as u8);
            out.push(0x80 | (cp & 0x3F) as u8);
        }
        0x0020_0000..=0x03FF_FFFF => {
            out.push(0xF8 | (cp >> 24) as u8);
            out.push(0x80 | ((cp >> 18) & 0x3F) as u8);
            out.push(0x80 | ((cp >> 12) & 0x3F) as u8);
            out.push(0x80 | ((cp >> 6) & 0x3F) as u8);
            out.push(0x80 | (cp & 0x3F) as u8);
        }
        0x0400_0000..=0x7FFF_FFFF => {
            out.push(0xFC | (cp >> 30) as u8);
            out.push(0x80 | ((cp >> 24) & 0x3F) as u8);
            out.push(0x80 | ((cp >> 18) & 0x3F) as u8);
            out.push(0x80 | ((cp >> 12) & 0x3F) as u8);
            out.push(0x80 | ((cp >> 6) & 0x3F) as u8);
            out.push(0x80 | (cp & 0x3F) as u8);
        }
        _ => unreachable!(),
    }
}

pub fn invalid_codepoint(cp: u32) -> bool {
    cp > 0x10FFFF || (0xD800 <= cp && cp <= 0xDFFF)
}

/// Returns whether `byte` is a UTF-8 continuation-byte.
#[inline]
fn is_continuation_byte(byte: u8) -> bool {
    byte & 0xC0 == 0x80
}

/// Returns the byte position of the encoded code point immediately before `position`.
#[inline]
fn previous_char_position(bytes: &[u8], position: usize) -> Option<usize> {
    if position == 0 || position > bytes.len() {
        return None;
    }

    let mut position = position - 1;

    while position > 0 && is_continuation_byte(bytes[position]) {
        position -= 1;
    }

    Some(position)
}

/// Returns the byte position of the encoded codepoint immediately after `position`.
#[inline]
fn next_char_position(bytes: &[u8], position: usize) -> Option<usize> {
    if position >= bytes.len() {
        return None;
    }

    let mut position = position + 1;

    while position < bytes.len() && is_continuation_byte(bytes[position]) {
        position += 1;
    }

    (position < bytes.len()).then_some(position)
}

/// Returns the start of the encoded code point containing `position`.
pub fn char_start(bytes: &[u8], position: usize) -> Option<usize> {
    if position >= bytes.len() {
        return None;
    }

    if is_continuation_byte(bytes[position]) {
        previous_char_position(bytes, position)
    } else {
        Some(position)
    }
}

/// Moves `distance` encoded code points from the `start` position and returns the byte position.
///
/// A distance of zero returns `start`. Positive distances move forward and negative distances
/// move backward.
/// Returns `None` if the requested position lies outside the slice.
pub fn move_by_chars(bytes: &[u8], start: usize, distance: i64) -> Option<usize> {
    if start > bytes.len() {
        return None;
    }

    let mut position = start;

    if distance >= 0 {
        for _ in 0..distance {
            position = next_char_position(bytes, position)?;
        }
    } else {
        for _ in 0..-distance {
            position = previous_char_position(bytes, position)?;
        }
    }

    Some(position)
}
