#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

/// Decodes one RFC 2279 sequence from the start of `bytes`.
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

pub fn invalid_codepoint(cp: u32) -> bool {
    cp > 0x10FFFF || (0xD800 <= cp && cp <= 0xDFFF)
}
