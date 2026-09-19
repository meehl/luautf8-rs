use crate::{matching::Match, pattern::Pattern};

pub struct Replacer<'p> {
    pattern: &'p Pattern,
}

impl<'p> Replacer<'p> {
    pub fn new(pattern: &'p Pattern) -> Self {
        Self { pattern }
    }

    /// Iterates over matches and replaces them according to replace function.
    pub fn replace_with<F, E>(
        &self,
        input: &str,
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

        for m in self.pattern.find_all(input).take(limit) {
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
                ReplacementPart::CaptureRef(i) => {
                    if let Some(capture) = m.capture((i - 1) as usize) {
                        output.push_str(capture)
                    }
                }
            }
        }

        output
    }
}
