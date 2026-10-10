use std::{
    iter::FusedIterator,
    ops::{Bound, Range, RangeBounds},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeErrorKind {
    InvalidLead,
    Truncated,
    Overlong,
    InvalidUtf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError {
    // Byte offset where decoding failed
    pub offset: usize,
    pub kind: DecodeErrorKind,
}

/// Returns whether `byte` is a UTF-8 continuation-byte.
#[inline]
fn is_continuation_byte(byte: u8) -> bool {
    byte & 0xC0 == 0x80
}

/// Decodes one code point at the start of `bytes` according to RFC 2279.
///
/// `bytes` must not be empty.
/// Returns the code point and length of the encoding.
/// NOTE: The resulting code point is not validated!
pub fn decode(bytes: &[u8], strict: bool) -> Result<(u32, usize), DecodeErrorKind> {
    debug_assert!(!bytes.is_empty());
    let first = bytes[0];

    let (len, initial_bits) = match first {
        0x00..=0x7F => return Ok((first as u32, 1)),
        0xC0..=0xDF => (2, first & 0b0001_1111),
        0xE0..=0xEF => (3, first & 0b0000_1111),
        0xF0..=0xF7 => (4, first & 0b0000_0111),
        0xF8..=0xFB => (5, first & 0b0000_0011),
        0xFC..=0xFD => (6, first & 0b0000_0001),
        _ => return Err(DecodeErrorKind::InvalidLead),
    };

    let rest = bytes.get(1..len).ok_or(DecodeErrorKind::Truncated)?;

    let mut cp = initial_bits as u32;
    for &b in rest.iter() {
        if !is_continuation_byte(b) {
            return Err(DecodeErrorKind::Truncated);
        }
        cp = (cp << 6) | (b & 0b0011_1111) as u32;
    }

    // reject "overlong" sequences
    const MIN_FOR_LEN: [u32; 7] = [0, 0, 0x80, 0x800, 0x1_0000, 0x20_0000, 0x400_0000];
    if cp < MIN_FOR_LEN[len] {
        return Err(DecodeErrorKind::Overlong);
    }

    if strict && (cp > 0x10_FFFF || (0xD800..=0xDFFF).contains(&cp)) {
        return Err(DecodeErrorKind::InvalidUtf8);
    }

    Ok((cp, len))
}

/// Decodes the last code point that *starts* in `front..back`.
/// Requires `front < back <= bytes.len()`.
fn decode_before(
    bytes: &[u8],
    front: usize,
    back: usize,
    strict: bool,
) -> Result<(usize, u32), DecodeError> {
    debug_assert!(front < back && back <= bytes.len());
    let lowest = back.saturating_sub(6).max(front);

    // find the first non-continuation byte before `back`
    let mut i = back - 1;
    while i > lowest && is_continuation_byte(bytes[i]) {
        i -= 1;
    }

    let (cp, n) = decode(&bytes[i..], strict).map_err(|kind| DecodeError { offset: i, kind })?;
    if i + n < back {
        // number of continuation bytes doesn't match length of decoded code point
        return Err(DecodeError {
            offset: i + n,
            kind: DecodeErrorKind::InvalidLead,
        });
    }

    Ok((i, cp))
}

#[derive(Debug, Clone)]
pub struct CodePoints<B> {
    bytes: B,
    // next position to decode from the front
    front: usize,
    // only decode code points that *start* before `back`
    back: usize,
    /// Strict mode rejects surrogates and code points above `0x10FFFF`.
    /// Lax mode accepts everything RFC 2279 allows.
    strict: bool,
}

impl<B: AsRef<[u8]>> CodePoints<B> {
    pub fn all(bytes: B, strict: bool) -> Self {
        let len = bytes.as_ref().len();
        Self {
            bytes,
            front: 0,
            back: len,
            strict,
        }
    }

    pub fn range(bytes: B, range: impl RangeBounds<usize>, strict: bool) -> Self {
        let len = bytes.as_ref().len();
        let start = match range.start_bound() {
            Bound::Included(&s) => s,
            Bound::Excluded(&s) => s.saturating_add(1),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&e) => e.saturating_add(1),
            Bound::Excluded(&e) => e,
            Bound::Unbounded => len,
        };
        Self {
            bytes,
            front: start.min(len),
            back: end.min(len),
            strict,
        }
    }

    pub fn remaining(&self) -> Range<usize> {
        self.front..self.back.max(self.front)
    }

    fn next_indexed(&mut self) -> Option<(usize, Result<u32, DecodeError>)> {
        if self.front >= self.back {
            return None;
        }
        let at = self.front;
        Some(match decode(&self.bytes.as_ref()[at..], self.strict) {
            Ok((cp, n)) => {
                self.front += n;
                (at, Ok(cp))
            }
            Err(kind) => {
                self.back = at; // fuse
                (at, Err(DecodeError { offset: at, kind }))
            }
        })
    }

    fn next_back_indexed(&mut self) -> Option<(usize, Result<u32, DecodeError>)> {
        if self.front >= self.back {
            return None;
        }
        Some(
            match decode_before(&self.bytes.as_ref(), self.front, self.back, self.strict) {
                Ok((start, cp)) => {
                    self.back = start;
                    (start, Ok(cp))
                }
                Err(e) => {
                    self.back = self.front; // fuse
                    (e.offset, Err(e))
                }
            },
        )
    }
}

impl<B: AsRef<[u8]>> Iterator for CodePoints<B> {
    type Item = (usize, Result<u32, DecodeError>);

    fn next(&mut self) -> Option<Self::Item> {
        self.next_indexed()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let span = self.back.saturating_sub(self.front);
        (span.min(1), Some(span))
    }
}

impl<B: AsRef<[u8]>> DoubleEndedIterator for CodePoints<B> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.next_back_indexed()
    }
}

impl<B: AsRef<[u8]>> FusedIterator for CodePoints<B> {}

/// Encodes one code point according to RFC 2279.
///
/// NOTE: Allows for code points up to `0x7FFFFFFF`.
pub fn encode(cp: u32, out: &mut Vec<u8>) {
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
