//! Helper functions for navigating a byte slice by character without validation.
//!
//! A character consists of one non-continuation byte followed by zero or more continuation bytes.
//! A leading run of continuation bytes is treated as a single character starting at byte 0.

use std::ops::Range;

/// Returns true if the byte is a continuation byte.
#[inline]
fn is_continuation(byte: u8) -> bool {
    byte & 0xC0 == 0x80
}

/// Returns true if `position` is at a continuation byte.
pub fn is_at_continuation(s: &[u8], position: usize) -> bool {
    s.get(position).is_some_and(|&byte| is_continuation(byte))
}

/// Finds the first non-continuation byte strictly after `position`.
///
/// Returns `len()` if no such byte exists. Positions beyond the slice are clamped to `len()`.
pub fn next_boundary(s: &[u8], position: usize) -> usize {
    let position = position.min(s.len());

    if position == s.len() {
        return position;
    }

    s[position + 1..]
        .iter()
        .position(|&b| !is_continuation(b))
        .map_or(s.len(), |offset| position + 1 + offset)
}

/// Finds the first non-continuation byte strictly before `position`.
///
/// Returns 0 if no such byte exists. Positions beyond the slice are clamped to `len()`.
pub fn prev_boundary(s: &[u8], position: usize) -> usize {
    let position = position.min(s.len());

    s[..position]
        .iter()
        .rposition(|&b| !is_continuation(b))
        .unwrap_or(0)
}

/// Finds the start of the character containing `position`.
///
/// Returns `None` if `position >= len()`.
pub fn char_start(s: &[u8], position: usize) -> Option<usize> {
    if position >= s.len() {
        return None;
    }

    if !is_continuation(s[position]) {
        return Some(position);
    }

    Some(
        s[..position]
            .iter()
            .rposition(|&b| !is_continuation(b))
            .unwrap_or(0),
    )
}

/// Returns the byte range of the character containing `position`.
///
/// Returns `None` if `position >= len()`.
pub fn char_range(s: &[u8], position: usize) -> Option<Range<usize>> {
    let start = char_start(s, position)?;
    let end = next_boundary(s, start);
    Some(start..end)
}

/// Moves `steps` character boundaries from a byte position.
///
/// Positive values move forward, negative values move backward.
/// A zero step preserves the position (after clamping to the slice).
pub fn move_by(s: &[u8], start: usize, steps: isize) -> Option<usize> {
    let mut position = start.min(s.len());
    let mut remaining = steps;

    while remaining > 0 {
        let next = next_boundary(s, position);

        if next == position {
            break;
        }

        position = next;
        remaining -= 1;
    }

    while remaining < 0 {
        let previous = prev_boundary(s, position);

        if previous == position {
            break;
        }

        position = previous;
        remaining += 1;
    }

    (remaining == 0).then_some(position)
}

/// Counts the characters that start between `start` and `end`.
pub fn char_count(s: &[u8], start: usize, end: usize) -> usize {
    let mut position = start;
    let end = end.min(s.len());
    let mut count = 0;

    while position < end {
        position = next_boundary(s, position);
        count += 1;
    }

    count
}
