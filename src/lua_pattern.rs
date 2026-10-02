mod matching;
mod pattern;
mod replacement;

pub(crate) use matching::{CaptureValue, Match, find, gmatch};
pub(crate) use pattern::Pattern;
pub(crate) use replacement::{ReplacementError, ReplacementString, replace_with};
