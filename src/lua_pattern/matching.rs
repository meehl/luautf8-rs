use crate::lua_pattern::pattern::{
    CharacterClass, CharacterSet, Item, Modifier, Pattern, PredefinedClass, PredefinedClassKind,
    SetElement,
};

/// Searches the input for the first match starting at the given byte position.
pub fn find<'s>(input: &'s str, pattern: &Pattern, start: Option<usize>) -> Option<Match<'s>> {
    let start = match start {
        Some(byte_index) => position_at_byte(input, byte_index)?,
        None => Position::default(),
    };

    find_from_position(input, pattern, start, AnchorMode::Relative)
}

/// Returns an iterator over all matches.
pub fn find_all<'s, 'p>(input: &'s str, pattern: &'p Pattern) -> Matches<'s, 'p> {
    Matches::new(input, pattern)
}

/// Returns an iterator over all matches that owns the input string and pattern.
pub fn gmatch(input: String, pattern: Pattern) -> GMatches {
    GMatches::new(input, pattern)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Position {
    byte_index: usize,
    char_index: usize,
}

#[derive(Clone, Copy, Debug)]
enum Capture {
    Text { start: Position, end: Position },
    Position(Position),
}

impl Capture {
    pub fn value<'s>(&self, input: &'s str) -> CaptureValue<'s> {
        match *self {
            Self::Text { start, end } => {
                CaptureValue::Text(&input[start.byte_index..end.byte_index])
            }
            Self::Position(p) => CaptureValue::Position(p.char_index),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureValue<'s> {
    Text(&'s str),
    /// 0-based character index
    Position(usize),
}

/// A successful match.
#[derive(Debug)]
pub struct Match<'s> {
    input: &'s str,
    start: Position,
    end: Position,
    captures: Vec<Capture>,
}

impl<'s> Match<'s> {
    /// Returns the matched substring.
    pub fn as_str(&self) -> &'s str {
        &self.input[self.start.byte_index..self.end.byte_index]
    }

    /// Returns the character position at which the match starts.
    pub fn start(&self) -> usize {
        self.start.char_index
    }

    /// Returns the character position immediately after the match.
    pub fn end(&self) -> usize {
        self.end.char_index
    }

    /// Returns the byte position at which the match starts.
    pub fn start_byte(&self) -> usize {
        self.start.byte_index
    }

    /// Returns the byte position immediately after the match.
    pub fn end_byte(&self) -> usize {
        self.end.byte_index
    }

    /// Returns capture at given index.
    pub fn capture(&self, index: usize) -> Option<CaptureValue<'s>> {
        self.captures.get(index).map(|c| c.value(self.input))
    }

    /// Returns iterator over captures.
    pub fn captures(&self) -> impl Iterator<Item = CaptureValue<'s>> {
        self.captures.iter().map(|c| c.value(self.input))
    }

    /// Returns iterator over all captures, or the whole match if there are none.
    pub fn values(&self) -> impl Iterator<Item = CaptureValue<'s>> {
        let whole = self
            .captures
            .is_empty()
            .then(|| CaptureValue::Text(self.as_str()));
        whole.into_iter().chain(self.captures())
    }
}

/// Iterator over matches.
#[derive(Debug)]
pub struct Matches<'s, 'p> {
    input: &'s str,
    pattern: &'p Pattern,
    next_pos: Position,
    last_match_end: Option<Position>,
}

impl<'s, 'p> Matches<'s, 'p> {
    pub fn new(input: &'s str, pattern: &'p Pattern) -> Self {
        Self {
            input,
            pattern,
            next_pos: Position::default(),
            last_match_end: None,
        }
    }
}

impl<'s, 'p> Iterator for Matches<'s, 'p> {
    type Item = Match<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let matched = find_from_position(
                self.input,
                self.pattern,
                self.next_pos,
                AnchorMode::Absolute,
            )?;

            // this candidate was already represented by the previous accepted match
            if self.last_match_end == Some(matched.end) {
                // At end of input there is no later position to search
                if matched.start.byte_index == self.input.len() {
                    return None;
                }

                // otherwise continue searching one character later
                self.next_pos = advance(self.input, matched.start);
                continue;
            }

            // accept the match
            self.last_match_end = Some(matched.end);
            self.next_pos = matched.end;

            return Some(matched);
        }
    }
}

/// Iterator over matches that owns the input string and pattern.
pub struct GMatches {
    input: String,
    pattern: Pattern,
    next_pos: Position,
    last_match_end: Option<Position>,
}

impl GMatches {
    pub fn new(input: String, pattern: Pattern) -> Self {
        Self {
            input,
            pattern,
            next_pos: Position::default(),
            last_match_end: None,
        }
    }

    pub fn next(&mut self) -> Option<Match<'_>> {
        loop {
            let matched = find_from_position(
                &self.input,
                &self.pattern,
                self.next_pos,
                AnchorMode::Absolute,
            )?;

            // this candidate was already represented by the previous accepted match
            if self.last_match_end == Some(matched.end) {
                // At end of input there is no later position to search
                if matched.start.byte_index == self.input.len() {
                    return None;
                }

                // otherwise continue searching one character later.
                self.next_pos = advance(&self.input, matched.start);
                continue;
            }

            // accept the match
            self.last_match_end = Some(matched.end);
            self.next_pos = matched.end;

            return Some(matched);
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CaptureSlot {
    Text {
        start: Position,
        end: Option<Position>, // None while still open
    },
    Position(Position),
}

#[derive(Debug)]
struct MatchContext<'s, 'p> {
    input: &'s str,
    pattern: &'p Pattern,
    captures: Vec<Option<CaptureSlot>>,
}

#[derive(Debug, Clone, Copy)]
enum AnchorMode {
    /// `^` is relative to the requested search position.
    Relative,
    /// `^` is relative to the beginning of the actual input.
    Absolute,
}

/// Searches for match starting at given position.
fn find_from_position<'s>(
    input: &'s str,
    pattern: &Pattern,
    start: Position,
    anchor_mode: AnchorMode,
) -> Option<Match<'s>> {
    let mut ctx = MatchContext {
        input,
        pattern,
        captures: vec![None; pattern.capture_count()],
    };

    let (start, end) = find_pattern(&mut ctx, start, anchor_mode)?;

    let captures = ctx
        .captures
        .into_iter()
        .map(|slot| {
            let slot = slot.expect("successful match doesn't leave capture slot unset");
            match slot {
                CaptureSlot::Text { start, end } => Capture::Text {
                    start,
                    end: end.unwrap_or(start),
                },
                CaptureSlot::Position(position) => Capture::Position(position),
            }
        })
        .collect();

    Some(Match {
        input,
        start,
        end,
        captures,
    })
}

/// Slides the pattern over the input and tries to find a match.
/// Returns the start and end positions of the match if successful.
fn find_pattern(
    ctx: &mut MatchContext,
    start: Position,
    anchor_mode: AnchorMode,
) -> Option<(Position, Position)> {
    let anchored = ctx.pattern.has_start_anchor();
    let pattern_index = if anchored { 1 } else { 0 };

    let mut pos = start;

    loop {
        // each attempted starting position needs a clean capture state
        ctx.captures.fill(None);

        // an absolutely anchored pattern may only match at the actual beginning of the input
        if anchored && matches!(anchor_mode, AnchorMode::Absolute) && pos.byte_index != 0 {
            return None;
        }

        if let Some(end) = do_match(ctx, pos, pattern_index) {
            return Some((pos, end));
        }

        // an anchored pattern gets exactly one attempt at the given start position.
        if anchored {
            return None;
        }

        if pos.byte_index == ctx.input.len() {
            return None;
        }

        pos = advance(ctx.input, pos);
    }
}

/// Tries to match pattern at given position and pattern index.
/// Returns end position on successful match or `None` on no match.
fn do_match(
    ctx: &mut MatchContext,
    mut pos: Position,
    mut pattern_index: usize,
) -> Option<Position> {
    let pattern_sequence = ctx.pattern.sequence();

    while pattern_index < pattern_sequence.len() {
        match &pattern_sequence[pattern_index] {
            Item::AnchorStart => {
                unreachable!("caller skips this by setting pattern_index to 1")
            }
            Item::AnchorEnd => {
                if pos.byte_index != ctx.input.len() {
                    return None;
                }

                pattern_index += 1;
            }
            Item::Class { class, modifier } => {
                let does_match = match_class(ctx, pos, class);

                if !does_match {
                    match modifier {
                        Some(
                            Modifier::ZeroOrMoreGreedy
                            | Modifier::ZeroOrMoreLazy
                            | Modifier::ZeroOrOne,
                        ) => {
                            // modifier makes class optional, so just continue
                            pattern_index += 1;
                            continue;
                        }
                        None | Some(Modifier::OneOrMore) => {
                            // class not optional, didn't match so return
                            return None;
                        }
                    }
                }

                // we have a match! proceed according to modifier
                match modifier {
                    None => {
                        pos = advance(ctx.input, pos);
                        pattern_index += 1;
                    }
                    Some(Modifier::ZeroOrMoreGreedy) => {
                        return max_expand(ctx, pos, pattern_index, class);
                    }
                    Some(Modifier::ZeroOrMoreLazy) => {
                        return min_expand(ctx, pos, pattern_index, class);
                    }
                    Some(Modifier::OneOrMore) => {
                        let next_pos = advance(ctx.input, pos);
                        return max_expand(ctx, next_pos, pattern_index, class);
                    }
                    Some(Modifier::ZeroOrOne) => {
                        let next_pos = advance(ctx.input, pos);
                        // check if rest of pattern matches if this is counted as "one"
                        if let Some(end) = do_match(ctx, next_pos, pattern_index + 1) {
                            return Some(end);
                        }
                        // otherwise fallback to zero occurrences
                        pattern_index += 1;
                    }
                }
            }
            Item::CaptureRef(n) => {
                pos = match_capture_ref(ctx, pos, *n)?;
                pattern_index += 1;
            }
            Item::Balanced { open, close } => {
                pos = match_balanced(ctx, pos, *open, *close)?;
                pattern_index += 1;
            }
            Item::Frontier(character_set) => {
                if !match_frontier(ctx, pos, character_set) {
                    return None;
                }
                pattern_index += 1;
            }
            Item::CaptureOpen(capture_index) => {
                let saved = ctx.captures[*capture_index];
                ctx.captures[*capture_index] = Some(CaptureSlot::Text {
                    start: pos,
                    end: None,
                });
                let result = do_match(ctx, pos, pattern_index + 1);
                if result.is_none() {
                    ctx.captures[*capture_index] = saved;
                }
                return result;
            }
            Item::CaptureClose(capture_index) => {
                let saved = ctx.captures[*capture_index];

                match &mut ctx.captures[*capture_index] {
                    Some(CaptureSlot::Text { end, .. }) => {
                        *end = Some(pos);
                    }
                    _ => unreachable!("capture close without an open text capture"),
                }

                let result = do_match(ctx, pos, pattern_index + 1);
                if result.is_none() {
                    ctx.captures[*capture_index] = saved;
                }
                return result;
            }
            Item::PositionCapture(capture_index) => {
                let saved = ctx.captures[*capture_index];

                ctx.captures[*capture_index] = Some(CaptureSlot::Position(pos));

                let result = do_match(ctx, pos, pattern_index + 1);
                if result.is_none() {
                    ctx.captures[*capture_index] = saved;
                }
                return result;
            }
        }
    }

    Some(pos)
}

fn match_class(ctx: &MatchContext, pos: Position, class: &CharacterClass) -> bool {
    ctx.input[pos.byte_index..]
        .chars()
        .next()
        .is_some_and(|ch| class.matches(ch))
}

/// Greedily matches as many repetitions of `class` as possible.
fn max_expand(
    ctx: &mut MatchContext,
    start: Position,
    item_index: usize,
    class: &CharacterClass,
) -> Option<Position> {
    // consume as many repetitions as possible and keep track of their positions
    let mut current_pos = start;
    let mut candidate_positions = vec![current_pos];
    while match_class(ctx, current_pos, class) {
        current_pos = advance(ctx.input, current_pos);
        candidate_positions.push(current_pos);
    }

    // backtrack from longest match until we find a position where the rest of the pattern
    // also matches
    for &pos in candidate_positions.iter().rev() {
        if let Some(end) = do_match(ctx, pos, item_index + 1) {
            return Some(end);
        }
    }

    None
}

/// Matches as few repetitions of `class` as possible.
fn min_expand(
    ctx: &mut MatchContext,
    start: Position,
    item_index: usize,
    class: &CharacterClass,
) -> Option<Position> {
    // start at zero repetitions and increase until rest of the pattern also matches.
    let mut current_pos = start;
    loop {
        if let Some(end) = do_match(ctx, current_pos, item_index + 1) {
            return Some(end);
        }
        if match_class(ctx, current_pos, class) {
            current_pos = advance(ctx.input, current_pos);
        } else {
            return None;
        }
    }
}

fn match_capture_ref(ctx: &MatchContext, pos: Position, n: u8) -> Option<Position> {
    let index = (n as usize).checked_sub(1)?;
    let capture = ctx.captures.get(index)?.as_ref()?;

    let CaptureSlot::Text {
        start,
        end: Some(end),
    } = capture
    else {
        return None;
    };

    let captured = &ctx.input[start.byte_index..end.byte_index];

    ctx.input[pos.byte_index..]
        .starts_with(captured)
        .then(|| Position {
            byte_index: pos.byte_index + captured.len(),
            char_index: pos.char_index + (end.char_index - start.char_index),
        })
}

fn match_balanced(ctx: &MatchContext, pos: Position, open: char, close: char) -> Option<Position> {
    let mut chars = ctx.input[pos.byte_index..].char_indices();
    let (_, first) = chars.next()?;

    if first != open {
        return None;
    }

    let mut balance = 1;
    for (char_pos, (i, ch)) in chars.enumerate() {
        if ch == open {
            balance += 1;
        } else if ch == close {
            balance -= 1;
            if balance == 0 {
                return Some(Position {
                    byte_index: pos.byte_index + i + ch.len_utf8(),
                    char_index: pos.char_index + 1 + char_pos + 1,
                });
            }
        }
    }

    None
}

fn match_frontier(ctx: &MatchContext, pos: Position, set: &CharacterSet) -> bool {
    let previous = ctx.input[..pos.byte_index]
        .chars()
        .next_back()
        .unwrap_or('\0');
    let next = ctx.input[pos.byte_index..].chars().next().unwrap_or('\0');
    !set.matches(previous) && set.matches(next)
}

fn advance(input: &str, pos: Position) -> Position {
    input[pos.byte_index..]
        .chars()
        .next()
        .map_or(pos, |ch| Position {
            byte_index: pos.byte_index + ch.len_utf8(),
            char_index: pos.char_index + 1,
        })
}

fn position_at_byte(input: &str, byte_index: usize) -> Option<Position> {
    let mut pos = Position::default();

    while pos.byte_index < byte_index {
        if pos.byte_index == input.len() {
            return None;
        }
        pos = advance(input, pos);
    }

    Some(pos)
}

/// Whether a parsed character class matches a given `char`.
trait MatchesClass {
    fn matches(&self, ch: char) -> bool;
}

impl MatchesClass for CharacterClass {
    fn matches(&self, ch: char) -> bool {
        match self {
            Self::Literal(c) => *c == ch,
            Self::Any => true,
            Self::Predefined(pc) => pc.matches(ch),
            Self::Set(set) => set.matches(ch),
        }
    }
}

impl MatchesClass for CharacterSet {
    fn matches(&self, ch: char) -> bool {
        let found = self.elements.iter().any(|el| match el {
            SetElement::Literal(c) => *c == ch,
            SetElement::Class(class) => class.matches(ch),
            SetElement::Range(lo, hi) => (*lo..=*hi).contains(&ch),
        });
        found != self.complement
    }
}

impl MatchesClass for PredefinedClass {
    fn matches(&self, ch: char) -> bool {
        let base = match self.kind {
            PredefinedClassKind::Letter => ch.is_alphabetic(),
            PredefinedClassKind::Control => ch.is_control(),
            // TODO: this is too broad! check `digit_table` in luautf8's `unidata.h`
            PredefinedClassKind::Digit => ch.is_numeric(),
            PredefinedClassKind::Printable => !ch.is_whitespace() && !ch.is_control(),
            PredefinedClassKind::Lowercase => ch.is_lowercase(),
            PredefinedClassKind::Punctuation => {
                ch.is_ascii_punctuation()
                    || (!ch.is_ascii()
                        && !ch.is_alphanumeric()
                        && !ch.is_whitespace()
                        && !ch.is_control())
            }
            PredefinedClassKind::Space => ch.is_whitespace(),
            PredefinedClassKind::Uppercase => ch.is_uppercase(),
            PredefinedClassKind::Alphanumeric => ch.is_alphanumeric(),
            PredefinedClassKind::HexDigit => ch.is_ascii_hexdigit(),
            PredefinedClassKind::Nul => ch == '\0',
        };
        if self.complement { !base } else { base }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(s: &str) -> Pattern {
        Pattern::parse(s).expect("valid Lua pattern")
    }

    fn find(p: &str, input: &str) -> Option<String> {
        super::find(input, &pattern(p), None).map(|m| m.as_str().to_owned())
    }

    fn find_with_captures(p: &str, input: &str) -> Option<(String, Vec<String>)> {
        super::find(input, &pattern(p), None).map(|m| {
            (
                m.as_str().to_owned(),
                m.captures()
                    .map(|capture| match capture {
                        CaptureValue::Text(text) => text.to_string(),
                        CaptureValue::Position(char_pos) => char_pos.to_string(),
                    })
                    .collect(),
            )
        })
    }

    #[test]
    fn literal_exact_match() {
        assert_eq!(find("hello", "hello"), Some("hello".into()));
    }

    #[test]
    fn literal_match_at_beginning() {
        assert_eq!(find("hello", "hello world"), Some("hello".into()));
    }

    #[test]
    fn literal_match_in_middle() {
        assert_eq!(find("world", "hello world!"), Some("world".into()));
    }

    #[test]
    fn literal_match_at_end() {
        assert_eq!(find("world", "hello world"), Some("world".into()));
    }

    #[test]
    fn literal_not_found() {
        assert_eq!(find("foo", "bar"), None);
    }

    #[test]
    fn literal_is_case_sensitive() {
        assert_eq!(find("foo", "FOO"), None);
        assert_eq!(find("Foo", "Foo"), Some("Foo".into()));
    }

    #[test]
    fn empty_pattern_matches_empty_string() {
        assert_eq!(find("", "abc"), Some("".into()));
    }

    #[test]
    fn empty_pattern_matches_empty_input() {
        assert_eq!(find("", ""), Some("".into()));
    }

    #[test]
    fn literal_empty_input_does_not_match() {
        assert_eq!(find("a", ""), None);
    }

    #[test]
    fn dot_matches_one_character() {
        assert_eq!(find(".", "abc"), Some("a".into()));
    }

    #[test]
    fn dot_matches_unicode_scalar() {
        assert_eq!(find(".", "éabc"), Some("é".into()));
    }

    #[test]
    fn dot_matches_four_byte_unicode_scalar() {
        assert_eq!(find(".", "😀abc"), Some("😀".into()));
    }

    #[test]
    fn dot_does_not_match_empty_input() {
        assert_eq!(find(".", ""), None);
    }

    #[test]
    fn dot_matches_whitespace() {
        assert_eq!(find(".", " abc"), Some(" ".into()));
    }

    #[test]
    fn simple_character_set() {
        assert_eq!(find("[abc]", "xxxbyyy"), Some("b".into()));
    }

    #[test]
    fn character_set_matches_first_possible_character() {
        assert_eq!(find("[abc]", "xxcay"), Some("c".into()));
    }

    #[test]
    fn character_set_does_not_match_other_character() {
        assert_eq!(find("[abc]", "xyz"), None);
    }

    #[test]
    fn character_set_range() {
        assert_eq!(find("[a-z]", "123m456"), Some("m".into()));
    }

    #[test]
    fn character_set_range_rejects_outside_range() {
        assert_eq!(find("[a-z]", "123A456"), None);
    }

    #[test]
    fn multiple_ranges() {
        assert_eq!(find("[a-fx-z]", "hello"), Some("e".into()));
        assert_eq!(find("[a-fx-z]", "uvwxy"), Some("x".into()));
    }

    #[test]
    fn set_with_literal_and_range() {
        assert_eq!(find("[0-9x]", "abcx"), Some("x".into()));
        assert_eq!(find("[0-9x]", "abc7"), Some("7".into()));
    }

    #[test]
    fn negated_character_set() {
        assert_eq!(find("[^a-z]", "abc1def"), Some("1".into()));
    }

    #[test]
    fn negated_character_set_matches_unicode() {
        assert_eq!(find("[^a-z]", "abcé"), Some("é".into()));
    }

    #[test]
    fn negated_character_set_does_not_match_member() {
        assert_eq!(find("[^a-z]", "abcdef"), None);
    }

    #[test]
    fn digit_class() {
        assert_eq!(find("%d", "abc123"), Some("1".into()));
    }

    #[test]
    fn digit_class_matches_unicode_digits_if_supported() {
        assert_eq!(find("%d", "abc１２３"), Some("１".into()));
    }

    #[test]
    fn non_digit_class() {
        assert_eq!(find("%D", "123abc"), Some("a".into()));
    }

    #[test]
    fn letter_class() {
        assert_eq!(find("%a", "123abc"), Some("a".into()));
    }

    #[test]
    fn non_letter_class() {
        assert_eq!(find("%A", "abc123"), Some("1".into()));
    }

    #[test]
    fn lowercase_class() {
        assert_eq!(find("%l", "ABCdef"), Some("d".into()));
    }

    #[test]
    fn uppercase_class() {
        assert_eq!(find("%u", "abcDEF"), Some("D".into()));
    }

    #[test]
    fn alphanumeric_class() {
        assert_eq!(find("%w", "---abc"), Some("a".into()));
    }

    #[test]
    fn non_alphanumeric_class() {
        assert_eq!(find("%W", "abc-"), Some("-".into()));
    }

    #[test]
    fn whitespace_class() {
        assert_eq!(find("%s", "abc def"), Some(" ".into()));
    }

    #[test]
    fn non_whitespace_class() {
        assert_eq!(find("%S", "   abc"), Some("a".into()));
    }

    #[test]
    fn punctuation_class() {
        assert_eq!(find("%p", "abc!def"), Some("!".into()));
    }

    #[test]
    fn hexadecimal_class() {
        assert_eq!(find("%x", "xyzaf"), Some("a".into()));
    }

    #[test]
    fn hexadecimal_class_uppercase() {
        assert_eq!(find("%x", "xyzAF"), Some("A".into()));
    }

    #[test]
    fn non_hexadecimal_class() {
        assert_eq!(find("%X", "abcZ"), Some("Z".into()));
    }

    #[test]
    fn control_class() {
        let input = "abc\nxyz";
        assert_eq!(find("%c", input), Some("\n".into()));
    }

    #[test]
    fn nul_class() {
        let input = "abc\0def";
        assert_eq!(find("%z", input), Some("\0".into()));
    }

    #[test]
    fn set_with_predefined_class() {
        assert_eq!(find("[%d]", "abc7"), Some("7".into()));
    }

    #[test]
    fn set_with_multiple_predefined_classes() {
        assert_eq!(find("[%d%l]", "123ABCdef"), Some("1".into()));
    }

    #[test]
    fn set_with_predefined_class_and_literal() {
        assert_eq!(find("[%d-]", "abc-"), Some("-".into()));
        assert_eq!(find("[%d-]", "abc7"), Some("7".into()));
    }

    #[test]
    fn optional_character_present() {
        assert_eq!(find("ab?c", "abc"), Some("abc".into()));
    }

    #[test]
    fn optional_character_absent() {
        assert_eq!(find("ab?c", "ac"), Some("ac".into()));
    }

    #[test]
    fn optional_is_greedy() {
        assert_eq!(find("ab?b", "abb"), Some("abb".into()));
    }

    #[test]
    fn optional_backtracks() {
        assert_eq!(find("ab?bc", "abbc"), Some("abbc".into()));
    }

    #[test]
    fn star_matches_zero_characters() {
        assert_eq!(find("ab*c", "ac"), Some("ac".into()));
    }

    #[test]
    fn star_matches_one_character() {
        assert_eq!(find("ab*c", "abc"), Some("abc".into()));
    }

    #[test]
    fn star_matches_many_characters() {
        assert_eq!(find("ab*c", "abbbc"), Some("abbbc".into()));
    }

    #[test]
    fn star_is_greedy() {
        assert_eq!(find("a*", "aaab"), Some("aaa".into()));
    }

    #[test]
    fn star_backtracks() {
        assert_eq!(find("a*a", "aaa"), Some("aaa".into()));
    }

    #[test]
    fn star_backtracks_multiple_times() {
        assert_eq!(find("a*aab", "aaaab"), Some("aaaab".into()));
    }

    #[test]
    fn star_can_backtrack_to_zero() {
        assert_eq!(find("a*a", "ba"), Some("a".into()));
    }

    #[test]
    fn star_does_not_consume_wrong_character() {
        assert_eq!(find("a*b", "aaac"), None);
    }

    #[test]
    fn star_can_match_empty_at_first_position() {
        assert_eq!(find("a*", "bbb"), Some("".into()));
    }

    #[test]
    fn plus_requires_one_character() {
        assert_eq!(find("a+b", "ab"), Some("ab".into()));
    }

    #[test]
    fn plus_requires_at_least_one() {
        assert_eq!(find("a+b", "b"), None);
    }

    #[test]
    fn plus_matches_many() {
        assert_eq!(find("a+b", "aaab"), Some("aaab".into()));
    }

    #[test]
    fn plus_is_greedy() {
        assert_eq!(find("a+", "aaab"), Some("aaa".into()));
    }

    #[test]
    fn plus_backtracks() {
        assert_eq!(find("a+a", "aaa"), Some("aaa".into()));
    }

    #[test]
    fn lazy_matches_zero_when_possible() {
        assert_eq!(find("a-b", "b"), Some("b".into()));
    }

    #[test]
    fn lazy_consumes_until_suffix_matches() {
        assert_eq!(find("a-b", "aaab"), Some("aaab".into()));
    }

    #[test]
    fn lazy_is_shorter_than_greedy() {
        assert_eq!(find("a-b", "aab"), Some("aab".into()));
    }

    #[test]
    fn lazy_backtracks_through_multiple_candidates() {
        assert_eq!(find("a-b", "aaac"), None);
    }

    #[test]
    fn lazy_can_match_empty() {
        assert_eq!(find("a-", "bbb"), Some("".into()));
    }

    #[test]
    fn greedy_takes_last_possible_suffix() {
        assert_eq!(find("a.*b", "a123b456b"), Some("a123b456b".into()));
    }

    #[test]
    fn lazy_takes_first_possible_suffix() {
        assert_eq!(find("a.-b", "a123b456b"), Some("a123b".into()));
    }

    #[test]
    fn greedy_and_lazy_differ() {
        assert_eq!(find("<.*>", "<one><two>"), Some("<one><two>".into()));
        assert_eq!(find("<.->", "<one><two>"), Some("<one>".into()));
    }

    #[test]
    fn single_capture() {
        assert_eq!(
            find_with_captures("(%a+)", "123hello456"),
            Some(("hello".into(), vec!["hello".into()]))
        );
    }

    #[test]
    fn multiple_captures() {
        assert_eq!(
            find_with_captures("(%a+)%s(%a+)", "hello world"),
            Some(("hello world".into(), vec!["hello".into(), "world".into(),]))
        );
    }

    #[test]
    fn capture_indices_follow_opening_parenthesis() {
        assert_eq!(
            find_with_captures("(a(b)c)", "abc"),
            Some(("abc".into(), vec!["abc".into(), "b".into(),]))
        );
    }

    #[test]
    fn nested_captures() {
        assert_eq!(
            find_with_captures("(a(b(c)))", "abc"),
            Some(("abc".into(), vec!["abc".into(), "bc".into(), "c".into()]))
        );
    }

    #[test]
    fn capture_can_be_empty() {
        assert_eq!(
            find_with_captures("(a*)b", "b"),
            Some(("b".into(), vec!["".into()]))
        );
    }

    #[test]
    fn capture_contains_greedy_match() {
        assert_eq!(
            find_with_captures("(a*)b", "aaab"),
            Some(("aaab".into(), vec!["aaa".into()]))
        );
    }

    #[test]
    fn capture_backtracks() {
        assert_eq!(
            find_with_captures("(a*)a", "aaa"),
            Some(("aaa".into(), vec!["aa".into()]))
        );
    }

    #[test]
    fn capture_backtracks_inside_nested_capture() {
        assert_eq!(
            find_with_captures("((a*)a)", "aaa"),
            Some(("aaa".into(), vec!["aaa".into(), "aa".into()]))
        );
    }

    #[test]
    fn capture_reference_matches_same_text() {
        assert_eq!(
            find_with_captures("(%a+)%1", "hellohello"),
            Some(("hellohello".into(), vec!["hello".into()]))
        );
    }

    #[test]
    fn capture_reference_rejects_different_text() {
        assert_eq!(find("(%a+)%1", "hellohelloX"), Some("hellohello".into()));
        assert_eq!(find("(%a+)%1", "hellohelloX"), Some("hellohello".into()));
    }

    #[test]
    fn capture_reference_can_be_empty() {
        assert_eq!(find("(a*)%1", "bbb"), Some("".into()));
    }

    #[test]
    fn capture_reference_uses_latest_capture_value() {
        assert_eq!(
            find_with_captures("(a)(b) %1%2", "ab ab"),
            Some(("ab ab".into(), vec!["a".into(), "b".into()]))
        );
    }

    #[test]
    fn capture_reference_after_nested_capture() {
        assert_eq!(
            find_with_captures("(a(b))%1", "abab"),
            Some(("abab".into(), vec!["ab".into(), "b".into()]))
        );
    }

    #[test]
    fn failed_backtracking_does_not_corrupt_capture_state() {
        let result = find_with_captures("(a*)ab", "aaab");
        assert_eq!(result, Some(("aaab".into(), vec!["aa".into()])));
    }

    #[test]
    fn capture_state_is_restored_between_backtracking_branches() {
        assert_eq!(
            find_with_captures("(a*)(b*)c", "aaabbc"),
            Some(("aaabbc".into(), vec!["aaa".into(), "bb".into()]))
        );
    }

    #[test]
    fn nested_capture_backtracking() {
        assert_eq!(
            find_with_captures("((a*)b*)c", "aaabbc"),
            Some(("aaabbc".into(), vec!["aaabb".into(), "aaa".into()]))
        );
    }

    #[test]
    fn start_anchor_matches_only_at_beginning() {
        assert_eq!(find("^abc", "abc"), Some("abc".into()));

        assert_eq!(find("^abc", "xabc"), None);
    }

    #[test]
    fn start_anchor_does_not_match_after_prefix() {
        assert_eq!(find("^abc", "xxxabc"), None);
    }

    #[test]
    fn end_anchor_matches_only_at_end() {
        assert_eq!(find("abc$", "abc"), Some("abc".into()));
        assert_eq!(find("abc$", "abcx"), None);
    }

    #[test]
    fn end_anchor_allows_prefix() {
        assert_eq!(find("abc$", "xxxabc"), Some("abc".into()));
    }

    #[test]
    fn both_anchors_require_entire_string() {
        assert_eq!(find("^abc$", "abc"), Some("abc".into()));
        assert_eq!(find("^abc$", "xabc"), None);
        assert_eq!(find("^abc$", "abcx"), None);
    }

    #[test]
    fn anchored_empty_pattern() {
        assert_eq!(find("^$", ""), Some("".into()));
        assert_eq!(find("^$", "abc"), None);
    }

    #[test]
    fn end_anchor_after_repetition() {
        assert_eq!(find("a*$", "aaa"), Some("aaa".into()));
    }

    #[test]
    fn greedy_repetition_must_backtrack_for_end_anchor() {
        assert_eq!(find("a*$", "aaa"), Some("aaa".into()));
    }

    #[test]
    fn frontier_matches_beginning_of_word() {
        assert_eq!(find("%f[%a]hello", " hello"), Some("hello".into()));
    }

    #[test]
    fn frontier_does_not_match_inside_word() {
        assert_eq!(find("%f[%a]ello", "hello"), None);
    }

    #[test]
    fn frontier_matches_after_non_member() {
        assert_eq!(find("%f[%a]world", "hello world"), Some("world".into()));
    }

    #[test]
    fn frontier_does_not_consume_character() {
        let result = super::find("hello", &pattern("%f[%a]hello"), None).unwrap();

        assert_eq!(result.as_str(), "hello");
        assert_eq!(result.start(), 0);
    }

    #[test]
    fn frontier_can_match_at_start_of_input() {
        assert_eq!(find("%f[%a]abc", "abc"), Some("abc".into()));
    }

    #[test]
    fn frontier_handles_unicode() {
        assert_eq!(find("%f[%a]é", " é"), Some("é".into()));
    }

    #[test]
    fn balanced_parentheses() {
        assert_eq!(find("%b()", "foo (bar) baz"), Some("(bar)".into()));
    }

    #[test]
    fn balanced_nested_parentheses() {
        assert_eq!(find("%b()", "foo (a(b)c) baz"), Some("(a(b)c)".into()));
    }

    #[test]
    fn balanced_deeply_nested() {
        assert_eq!(find("%b()", "(((())))"), Some("(((())))".into()));
    }

    #[test]
    fn balanced_stops_at_matching_close() {
        assert_eq!(find("%b()", "(one)(two)"), Some("(one)".into()));
    }

    #[test]
    fn balanced_fails_without_opening_character() {
        assert_eq!(find("%b()", "abc"), None);
    }

    #[test]
    fn balanced_fails_with_unclosed_group() {
        assert_eq!(find("%b()", "(abc"), None);
    }

    #[test]
    fn balanced_fails_with_wrong_closing_character() {
        assert_eq!(find("%b()", "(abc]"), None);
    }

    #[test]
    fn balanced_square_brackets() {
        assert_eq!(find("%b[]", "[abc]"), Some("[abc]".into()));
    }

    #[test]
    fn balanced_curly_braces() {
        assert_eq!(find("%b{}", "{a{b}c}"), Some("{a{b}c}".into()));
    }

    #[test]
    fn matching_does_not_split_utf8() {
        let result = super::find("é", &pattern("."), None).unwrap();

        assert_eq!(result.as_str(), "é");
        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 1);
    }

    #[test]
    fn matching_four_byte_character() {
        let result = super::find("😀", &pattern("."), None).unwrap();

        assert_eq!(result.as_str(), "😀");
        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 1);
    }

    #[test]
    fn unicode_literal() {
        assert_eq!(find("한국", "한국어를"), Some("한국".into()));
    }

    #[test]
    fn unicode_repetition() {
        assert_eq!(find(".+", "😀😀abc"), Some("😀😀abc".into()));
    }

    #[test]
    fn unicode_set_range() {
        assert_eq!(find("[α-ω]", "123β456"), Some("β".into()));
    }

    #[test]
    fn unicode_capture() {
        assert_eq!(
            find_with_captures("(%a+)", "안녕 hello"),
            Some(("안녕".into(), vec!["안녕".into()]))
        );
    }

    #[test]
    fn unicode_capture_span_is_char_based() {
        let result = super::find("é", &pattern("(é)"), None).unwrap();

        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 1);
        assert_eq!(result.capture(0), Some(CaptureValue::Text("é")));
    }

    #[test]
    fn find_returns_first_match() {
        assert_eq!(find("a", "xxaXXa"), Some("a".into()));
    }

    #[test]
    fn find_starts_at_each_character_boundary() {
        assert_eq!(find("bc", "aabc"), Some("bc".into()));
    }

    #[test]
    fn find_does_not_start_in_middle_of_utf8() {
        let result = super::find("éx", &pattern("."), None).unwrap();

        assert_eq!(result.start(), 0);
    }

    #[test]
    fn find_returns_match_at_start_for_empty_pattern() {
        let result = super::find("abc", &pattern(""), None).unwrap();

        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 0);
    }

    #[test]
    fn match_boundaries_are_char_indices() {
        let result = super::find("hello 안녕 abc", &pattern("안녕"), None).unwrap();

        assert_eq!(result.as_str(), "안녕");
        assert_eq!(result.start(), 6);
        assert_eq!(result.end(), 8);
    }

    #[test]
    fn star_can_produce_empty_match() {
        let result = super::find("bbb", &pattern("a*"), None).unwrap();

        assert_eq!(result.as_str(), "");
        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 0);
    }

    #[test]
    fn optional_can_produce_empty_match() {
        let result = super::find("bbb", &pattern("a?"), None).unwrap();

        assert_eq!(result.as_str(), "");
    }

    #[test]
    fn frontier_is_zero_width() {
        let result = super::find(" abc", &pattern("%f[%a]"), None).unwrap();

        assert_eq!(result.as_str(), "");
        assert_eq!(result.start(), 1);
        assert_eq!(result.end(), 1);
    }

    #[test]
    fn empty_match_can_occur_at_eof() {
        let result = super::find("abc", &pattern("$"), None).unwrap();

        assert_eq!(result.as_str(), "");
        assert_eq!(result.start(), 3);
        assert_eq!(result.end(), 3);
    }

    #[test]
    fn adjacent_greedy_repetitions() {
        assert_eq!(find("a*a*", "aaaa"), Some("aaaa".into()));
    }

    #[test]
    fn adjacent_repetitions_backtrack() {
        assert_eq!(find("a*a*b", "aaaab"), Some("aaaab".into()));
    }

    #[test]
    fn three_repetitions_backtrack() {
        assert_eq!(find("a*a*a*b", "aaaaab"), Some("aaaaab".into()));
    }

    #[test]
    fn greedy_capture_followed_by_literal() {
        assert_eq!(
            find_with_captures("(.*)x", "abcx"),
            Some(("abcx".into(), vec!["abc".into()]))
        );
    }

    #[test]
    fn greedy_capture_backtracks_to_last_literal() {
        assert_eq!(
            find_with_captures("(.*)x", "abcxdefx"),
            Some(("abcxdefx".into(), vec!["abcxdef".into()]))
        );
    }

    #[test]
    fn lazy_capture_stops_at_first_literal() {
        assert_eq!(
            find_with_captures("(.-)x", "abcxdefx"),
            Some(("abcx".into(), vec!["abc".into()]))
        );
    }

    #[test]
    fn greedy_capture_can_backtrack_to_empty() {
        assert_eq!(
            find_with_captures("(.*)x", "x"),
            Some(("x".into(), vec!["".into()]))
        );
    }

    #[test]
    fn lazy_capture_can_match_empty() {
        assert_eq!(
            find_with_captures("(.-)x", "x"),
            Some(("x".into(), vec!["".into()]))
        );
    }

    #[test]
    fn positional_capture_matches_every_position() {
        let pattern = Pattern::parse("()").unwrap();
        let matches = find_all("abc", &pattern).collect::<Vec<_>>();
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 0);
        assert_eq!(matches[1].start(), 1);
        assert_eq!(matches[1].end(), 1);
        assert_eq!(matches[2].start(), 2);
        assert_eq!(matches[2].end(), 2);
        assert_eq!(matches[3].start(), 3);
        assert_eq!(matches[3].end(), 3);
    }

    #[test]
    fn positional_capture_at_start() {
        let pattern = Pattern::parse("()").unwrap();
        let m = super::find("hello", &pattern, None).unwrap();

        assert_eq!(m.as_str(), "");
        assert_eq!(m.start(), 0);
        assert_eq!(m.end(), 0);

        assert_eq!(m.capture(0), Some(CaptureValue::Position(0)));
        assert_eq!(
            m.captures().collect::<Vec<_>>(),
            vec![CaptureValue::Position(0)]
        );
    }

    #[test]
    fn positional_capture_between_ascii_characters() {
        let pattern = Pattern::parse("a()bc").unwrap();
        let m = super::find("abc", &pattern, None).unwrap();

        assert_eq!(m.as_str(), "abc");
        assert_eq!(m.start(), 0);
        assert_eq!(m.end(), 3);

        assert_eq!(m.capture(0), Some(CaptureValue::Position(1)));
    }

    #[test]
    fn positional_capture_between_utf8() {
        let pattern = Pattern::parse("안()녕하세요").unwrap();
        let m = super::find("안녕하세요", &pattern, None).unwrap();

        assert_eq!(m.as_str(), "안녕하세요");
        assert_eq!(m.start(), 0);
        assert_eq!(m.end(), 5);

        assert_eq!(m.capture(0), Some(CaptureValue::Position(1)));
        assert_ne!(m.capture(0), Some(CaptureValue::Position(3)));
    }

    #[test]
    fn positional_capture_at_end() {
        let input = "안녕하세요";
        let pattern = Pattern::parse("안녕하세요()").unwrap();
        let m = super::find(input, &pattern, None).unwrap();

        assert_eq!(m.as_str(), input);
        assert_eq!(m.start(), 0);
        assert_eq!(m.end(), 5);
        assert_eq!(m.start_byte(), 0);
        assert_eq!(m.end_byte(), input.len());

        assert_eq!(m.capture(0), Some(CaptureValue::Position(5)));
    }

    #[test]
    fn multiple_positional_captures() {
        let pattern = Pattern::parse("안()녕()하").unwrap();
        let m = super::find("안녕하세요", &pattern, None).unwrap();

        assert_eq!(
            m.captures().collect::<Vec<_>>(),
            vec![CaptureValue::Position(1), CaptureValue::Position(2),]
        );
    }

    #[test]
    fn mixed_capture_types() {
        let pattern = Pattern::parse("(안)()녕").unwrap();
        let m = super::find("안녕하세요", &pattern, None).unwrap();

        assert_eq!(
            m.captures().collect::<Vec<_>>(),
            vec![CaptureValue::Text("안"), CaptureValue::Position(1),]
        );
    }

    #[test]
    fn find_all_returns_all_matches() {
        let input = "one two one three";
        let pattern = Pattern::parse("one").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].as_str(), "one");
        assert_eq!(matches[1].as_str(), "one");

        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 3);

        assert_eq!(matches[1].start(), 8);
        assert_eq!(matches[1].end(), 11);
    }

    #[test]
    fn find_all_returns_adjacent_matches() {
        let input = "aaaa";
        let pattern = Pattern::parse("aa").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 2);

        assert_eq!(matches[0].as_str(), "aa");
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 2);

        assert_eq!(matches[1].as_str(), "aa");
        assert_eq!(matches[1].start(), 2);
        assert_eq!(matches[1].end(), 4);
    }

    #[test]
    fn find_all_returns_no_matches() {
        let input = "abcdef";
        let pattern = Pattern::parse("xyz").unwrap();

        assert_eq!(find_all(input, &pattern).count(), 0);
    }

    #[test]
    fn find_all_handles_empty_match_at_start() {
        let input = "abc";
        let pattern = Pattern::parse("()").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 4);

        for (index, m) in matches.iter().enumerate() {
            assert_eq!(m.as_str(), "");
            assert_eq!(m.start(), index);
            assert_eq!(m.end(), index);
            assert_eq!(m.capture(0), Some(CaptureValue::Position(index)));
        }
    }

    #[test]
    fn find_all_handles_empty_matches_with_utf8() {
        let input = "안녕";
        let pattern = Pattern::parse("()").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 3);

        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 0);
        assert_eq!(matches[0].capture(0), Some(CaptureValue::Position(0)));

        assert_eq!(matches[1].start(), 1);
        assert_eq!(matches[1].end(), 1);
        assert_eq!(matches[1].capture(0), Some(CaptureValue::Position(1)));

        assert_eq!(matches[2].start(), 2);
        assert_eq!(matches[2].end(), 2);
        assert_eq!(matches[2].capture(0), Some(CaptureValue::Position(2)));
    }

    #[test]
    fn find_all_empty_match_advances_by_one_character_not_one_byte() {
        let input = "a안b";
        let pattern = Pattern::parse("()").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 4);

        let positions: Vec<_> = matches
            .iter()
            .map(|m| (m.start(), m.start_byte()))
            .collect();

        assert_eq!(positions, vec![(0, 0), (1, 1), (2, 4), (3, 5),]);

        let captures: Vec<_> = matches.iter().map(|m| m.capture(0)).collect();

        assert_eq!(
            captures,
            vec![
                Some(CaptureValue::Position(0)),
                Some(CaptureValue::Position(1)),
                Some(CaptureValue::Position(2)),
                Some(CaptureValue::Position(3)),
            ]
        );
    }

    #[test]
    fn find_all_empty_match_on_empty_input() {
        let input = "";
        let pattern = Pattern::parse("()").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "");
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 0);
        assert_eq!(matches[0].capture(0), Some(CaptureValue::Position(0)));
    }

    #[test]
    fn find_all_multiple_positional_captures() {
        let input = "안녕 안녕";
        let pattern = Pattern::parse("()안").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches.len(), 2);

        assert_eq!(matches[0].as_str(), "안");
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 1);
        assert_eq!(matches[0].capture(0), Some(CaptureValue::Position(0)));

        assert_eq!(matches[1].as_str(), "안");
        assert_eq!(matches[1].start(), 3);
        assert_eq!(matches[1].end(), 4);
        assert_eq!(matches[1].capture(0), Some(CaptureValue::Position(3)));
    }

    #[test]
    fn find_all_greedy_modifier_with_positional_capture() {
        let input = "aaac";
        let pattern = Pattern::parse("a*()ac").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();

        assert_eq!(matches[0].as_str(), "aaac");
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 4);

        // a* initially consumes all three 'a' characters but has to backtrack to match
        // the remainder.
        assert_eq!(matches[0].capture(0), Some(CaptureValue::Position(2)));
    }

    #[test]
    fn find_all_lastmatch_handling() {
        let input = "abc";
        let pattern = Pattern::parse("b?").unwrap();

        let matches: Vec<_> = find_all(input, &pattern).collect();
        dbg!(&matches);

        assert_eq!(matches.len(), 3);

        assert_eq!(matches[0].as_str(), "");
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[0].end(), 0);

        assert_eq!(matches[1].as_str(), "b");
        assert_eq!(matches[1].start(), 1);
        assert_eq!(matches[1].end(), 2);

        assert_eq!(matches[2].as_str(), "");
        assert_eq!(matches[2].start(), 3);
        assert_eq!(matches[2].end(), 3);
    }
}
