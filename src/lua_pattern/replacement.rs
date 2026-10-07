use crate::lua_pattern::{
    matching::{CaptureValue, Match, find_all},
    pattern::Pattern,
};

/// Iterates over matches and replaces them according to replace function.
pub fn replace_with<F, E>(
    input: &str,
    pattern: &Pattern,
    replacement: F,
    limit: Option<usize>,
) -> Result<(String, usize), E>
where
    F: Fn(&Match) -> Result<Option<String>, E>,
{
    let limit = limit.unwrap_or(usize::MAX);

    let mut output = String::new();
    let mut last = 0;
    let mut num_of_replacements = 0;

    for m in find_all(input, pattern).take(limit) {
        // leave original between matches intact
        output.push_str(&input[last..m.start_byte()]);

        match replacement(&m)? {
            Some(replacement) => output.push_str(&replacement),
            None => output.push_str(m.as_str()),
        }

        last = m.end_byte();
        num_of_replacements += 1;
    }

    // leave original after final match intact
    output.push_str(&input[last..]);
    Ok((output, num_of_replacements))
}

/// Parsed replacement string.
pub struct ReplacementString {
    parts: Vec<ReplacementPart>,
}

enum ReplacementPart {
    Text(String),
    /// `%0`: replace with whole match.
    WholeMatch,
    /// `%1` to `%9`: replace with the corresponding capture.
    CaptureRef(u8),
}

#[derive(Debug)]
pub enum ReplacementError {
    InvalidReference,
    InvalidEscape,
}

impl ReplacementString {
    pub fn parse(source: &str, max_capture_count: u8) -> Result<Self, ReplacementError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut chars = source.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch != '%' {
                text.push(ch);
                continue;
            }

            match chars.peek() {
                Some('%') => {
                    chars.next();
                    text.push('%');
                }
                Some('0') => {
                    chars.next();

                    if !text.is_empty() {
                        parts.push(ReplacementPart::Text(std::mem::take(&mut text)));
                    }

                    parts.push(ReplacementPart::WholeMatch);
                }
                Some('1'..='9') => {
                    let digit = chars.next().expect("peeked value exists");
                    let capture = digit.to_digit(10).expect("1..=9 is a digit") as u8;

                    if capture > max_capture_count {
                        return Err(ReplacementError::InvalidReference);
                    }

                    if !text.is_empty() {
                        parts.push(ReplacementPart::Text(std::mem::take(&mut text)));
                    }

                    parts.push(ReplacementPart::CaptureRef(capture));
                }
                Some(_) | None => {
                    return Err(ReplacementError::InvalidEscape);
                }
            }
        }

        if !text.is_empty() {
            parts.push(ReplacementPart::Text(text));
        }

        Ok(Self { parts })
    }

    pub fn apply(&self, m: &Match<'_>) -> String {
        let mut output = String::new();

        for part in &self.parts {
            match part {
                ReplacementPart::Text(s) => output.push_str(s),
                ReplacementPart::WholeMatch => output.push_str(m.as_str()),
                ReplacementPart::CaptureRef(i) => match m.capture((i - 1) as usize) {
                    Some(CaptureValue::Text(text)) => output.push_str(text),
                    Some(CaptureValue::Position(char_pos)) => {
                        output.push_str(&format!("{}", char_pos + 1));
                    }
                    None => unreachable!("rejected by parser"),
                },
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_with_replaces_all_matches() {
        let input = "foo bar foo";
        let pattern = Pattern::parse("foo").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |_| Ok::<_, ()>(Some("baz".to_owned())),
            None,
        )
        .unwrap();

        assert_eq!(output, "baz bar baz");
        assert_eq!(count, 2);
    }

    #[test]
    fn replace_with_leaves_non_matching_input_intact() {
        let input = "hello world";
        let pattern = Pattern::parse("xyz").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |_| Ok::<_, ()>(Some("replacement".to_owned())),
            None,
        )
        .unwrap();

        assert_eq!(output, input);
        assert_eq!(count, 0);
    }

    #[test]
    fn replace_with_replacement_can_be_empty() {
        let input = "hello world";
        let pattern = Pattern::parse(" ").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some(String::new())), None).unwrap();

        assert_eq!(output, "helloworld");
        assert_eq!(count, 1);
    }

    #[test]
    fn replace_with_none_keeps_original_match() {
        let input = "foo bar foo";
        let pattern = Pattern::parse("foo").unwrap();

        let (output, count) = replace_with(input, &pattern, |_| Ok::<_, ()>(None), None).unwrap();

        assert_eq!(output, input);
        assert_eq!(count, 2);
    }

    #[test]
    fn replace_with_limit_zero_does_not_replace_anything() {
        let input = "foo foo";
        let pattern = Pattern::parse("foo").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |_| Ok::<_, ()>(Some("bar".to_owned())),
            Some(0),
        )
        .unwrap();

        assert_eq!(output, input);
        assert_eq!(count, 0);
    }

    #[test]
    fn replace_with_limit_one_replaces_only_first_match() {
        let input = "foo foo foo";
        let pattern = Pattern::parse("foo").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |_| Ok::<_, ()>(Some("bar".to_owned())),
            Some(1),
        )
        .unwrap();

        assert_eq!(output, "bar foo foo");
        assert_eq!(count, 1);
    }

    #[test]
    fn replace_with_limit_replaces_exact_number_of_matches() {
        let input = "foo foo foo";
        let pattern = Pattern::parse("foo").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |_| Ok::<_, ()>(Some("bar".to_owned())),
            Some(3),
        )
        .unwrap();

        assert_eq!(output, "bar bar bar");
        assert_eq!(count, 3);
    }

    #[test]
    fn replace_with_limit_larger_than_number_of_matches_replaces_all() {
        let input = "foo foo";
        let pattern = Pattern::parse("foo").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |_| Ok::<_, ()>(Some("bar".to_owned())),
            Some(10),
        )
        .unwrap();

        assert_eq!(output, "bar bar");
        assert_eq!(count, 2);
    }

    #[test]
    fn replace_with_adjacent_matches() {
        let input = "aaaa";
        let pattern = Pattern::parse("aa").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some("X".to_owned())), None).unwrap();

        assert_eq!(output, "XX");
        assert_eq!(count, 2);
    }

    #[test]
    fn replace_with_empty_matches() {
        let input = "abc";
        let pattern = Pattern::parse("()").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some("X".to_owned())), None).unwrap();

        assert_eq!(output, "XaXbXcX");
        assert_eq!(count, 4);
    }

    #[test]
    fn replace_with_empty_match_on_empty_input() {
        let input = "";
        let pattern = Pattern::parse("()").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some("X".to_owned())), None).unwrap();

        assert_eq!(output, "X");
        assert_eq!(count, 1);
    }

    #[test]
    fn replace_with_can_use_captures() {
        let input = "hello world";
        let pattern = Pattern::parse("(%a+) (%a+)").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |m| {
                let first = match m.capture(0).unwrap() {
                    CaptureValue::Text(value) => value,
                    CaptureValue::Position(_) => unreachable!(),
                };

                let second = match m.capture(1).unwrap() {
                    CaptureValue::Text(value) => value,
                    CaptureValue::Position(_) => unreachable!(),
                };

                Ok::<_, ()>(Some(format!("{second} {first}")))
            },
            None,
        )
        .unwrap();

        assert_eq!(output, "world hello");
        assert_eq!(count, 1);
    }

    #[test]
    fn replace_with_can_use_positional_capture() {
        let input = "aaac";
        let pattern = Pattern::parse("a*()ac").unwrap();

        let (output, count) = replace_with(
            input,
            &pattern,
            |m| {
                let position = match m.capture(0).unwrap() {
                    CaptureValue::Position(position) => position,
                    CaptureValue::Text(_) => unreachable!(),
                };

                Ok::<_, ()>(Some(format!("[{position}]")))
            },
            None,
        )
        .unwrap();

        assert_eq!(output, "[2]");
        assert_eq!(count, 1);
    }

    #[test]
    fn replace_with_lastmatch_handling() {
        let input = "abc";
        let pattern = Pattern::parse("b?").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some("X".to_owned())), None).unwrap();

        assert_eq!(output, "XaXcX");
        assert_eq!(count, 3);
    }

    #[test]
    fn replace_with_start_anchor_applies_only_once() {
        let input = "abc";
        let pattern = Pattern::parse("^").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some("X".to_owned())), None).unwrap();

        assert_eq!(output, "Xabc");
        assert_eq!(count, 1);
    }

    #[test]
    fn replace_with_anchored_pattern_only_replaces_at_start() {
        let input = "abac";
        let pattern = Pattern::parse("^a").unwrap();

        let (output, count) =
            replace_with(input, &pattern, |_| Ok::<_, ()>(Some("X".to_owned())), None).unwrap();

        assert_eq!(output, "Xbac");
        assert_eq!(count, 1);
    }
}
