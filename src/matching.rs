use crate::pattern::{
    CharacterClass, CharacterSet, Item, Modifier, Pattern, PredefinedClass, PredefinedClassKind,
    SetElement,
};

/// A successful match.
#[derive(Debug)]
pub struct Match<'s> {
    input: &'s str,
    start: usize,
    end: usize,
    captures: Vec<CaptureSpan>,
}

/// Byte offsets into the input string at which a capture starts and ends.
#[derive(Clone, Copy, Debug)]
struct CaptureSpan {
    start: usize,
    end: usize,
}

impl<'s> Match<'s> {
    /// Returns the matched substring.
    pub fn as_str(&self) -> &'s str {
        &self.input[self.start..self.end]
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    /// Returns capture at given index.
    pub fn capture(&self, index: usize) -> Option<&'s str> {
        self.captures
            .get(index)
            .map(|span| &self.input[span.start..span.end])
    }

    /// Returns iterator over captures.
    pub fn captures(&self) -> impl Iterator<Item = &'s str> {
        self.captures
            .iter()
            .map(|span| &self.input[span.start..span.end])
    }

    /// Returns number of captures.
    pub fn capture_count(&self) -> usize {
        self.captures.len()
    }
}

/// Iterator over matches.
#[derive(Debug)]
pub struct Matches<'p, 's> {
    matcher: Matcher<'p>,
    input: &'s str,
    next_pos: usize,
}

impl<'p, 's> Matches<'p, 's> {
    pub fn new(pattern: &'p Pattern, input: &'s str) -> Self {
        Self {
            matcher: Matcher::new(pattern),
            input,
            next_pos: 0,
        }
    }
}

impl<'p, 's> Iterator for Matches<'p, 's> {
    type Item = Match<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        let m = self.matcher.find_from(self.input, self.next_pos)?;
        // always advance least one char past the previous match end to avoid inifite loop on
        // zero-length match.
        self.next_pos = if m.end > self.next_pos {
            m.end
        } else {
            advance(self.input, m.end)
        };
        Some(m)
    }
}

#[derive(Clone, Copy, Debug)]
struct CaptureSlot {
    start: usize,
    end: Option<usize>, // None while still open
}

#[derive(Debug)]
struct MatchContext<'s> {
    input: &'s str,
    captures: Vec<CaptureSlot>,
}

#[derive(Debug)]
pub struct Matcher<'p> {
    pattern: &'p Pattern,
}

impl<'p> Matcher<'p> {
    pub fn new(pattern: &'p Pattern) -> Self {
        Self { pattern }
    }

    /// Searches the entire input for the first match.
    pub fn find<'s>(&self, input: &'s str) -> Option<Match<'s>> {
        self.find_from(input, 0)
    }

    /// Searches the input for the first match starting at given position.
    pub fn find_from<'s>(&self, input: &'s str, start: usize) -> Option<Match<'s>> {
        if start > input.len() {
            return None;
        }

        // only try the start if anchored with `^`
        if self.pattern.anchored_start() {
            return self.match_at(input, start);
        }

        // otherwise, "slide" the pattern over the input and try to find a match
        let mut pos = start;
        loop {
            if let Some(m) = self.match_at(input, pos) {
                return Some(m);
            }
            if pos >= input.len() {
                return None;
            }
            pos = advance(input, pos);
        }
    }

    /// Atempts to match the pattern to the input at given position.
    fn match_at<'s>(&self, input: &'s str, position: usize) -> Option<Match<'s>> {
        let mut ctx = MatchContext {
            input,
            captures: vec![
                CaptureSlot {
                    start: position,
                    end: None
                };
                self.pattern.capture_count()
            ],
        };

        let end = self.do_match(&mut ctx, position, 0)?;

        let captures = ctx
            .captures
            .into_iter()
            .map(|slot| CaptureSpan {
                start: slot.start,
                end: slot.end.unwrap_or(slot.start),
            })
            .collect();

        Some(Match {
            input,
            start: position,
            end,
            captures,
        })
    }

    // Recursive match, similar to Lua's `match` in `lstrlib.c`.
    fn do_match<'s>(
        &self,
        ctx: &mut MatchContext<'s>,
        mut pos: usize,
        mut item_index: usize,
    ) -> Option<usize> {
        loop {
            let pattern_sequence = self.pattern.sequence();
            if item_index >= pattern_sequence.len() {
                return if self.pattern.anchored_end() {
                    (pos == ctx.input.len()).then_some(pos)
                } else {
                    Some(pos)
                };
            }

            match &pattern_sequence[item_index] {
                Item::Class { class, modifier } => {
                    let does_match = self.match_class(ctx, pos, class);

                    if !does_match {
                        match modifier {
                            Some(Modifier::ZeroOrMoreGreedy)
                            | Some(Modifier::ZeroOrMoreLazy)
                            | Some(Modifier::ZeroOrOne) => {
                                // modifier makes class optional, so just continue
                                item_index += 1;
                                continue;
                            }
                            None | Some(Modifier::OneOrMore) => {
                                // class not optinal, didn't match so return
                                return None;
                            }
                        }
                    }

                    // we have a match! proceed according to modifier
                    match modifier {
                        None => {
                            pos = advance(ctx.input, pos);
                            item_index += 1;
                            continue;
                        }
                        Some(Modifier::ZeroOrMoreGreedy) => {
                            return self.max_expand(ctx, pos, item_index, class);
                        }
                        Some(Modifier::ZeroOrMoreLazy) => {
                            return self.min_expand(ctx, pos, item_index, class);
                        }
                        Some(Modifier::OneOrMore) => {
                            let next_pos = advance(ctx.input, pos);
                            return self.max_expand(ctx, next_pos, item_index, class);
                        }
                        Some(Modifier::ZeroOrOne) => {
                            let next_pos = advance(ctx.input, pos);
                            // check if rest of pattern matches if this is counted as "one"
                            if let Some(end) = self.do_match(ctx, next_pos, item_index + 1) {
                                return Some(end);
                            }
                            // otherwise fallback to zero occurrences
                            item_index += 1;
                            continue;
                        }
                    }
                }
                Item::CaptureRef(n) => match self.match_capture_ref(ctx, pos, *n) {
                    Some(end) => {
                        pos = end;
                        item_index += 1;
                        continue;
                    }
                    None => return None,
                },
                Item::Balanced { open, close } => {
                    match self.match_balanced(ctx, pos, *open, *close) {
                        Some(end) => {
                            pos = end;
                            item_index += 1;
                            continue;
                        }
                        None => return None,
                    }
                }
                Item::Frontier(character_set) => {
                    if self.match_frontier(ctx, pos, character_set) {
                        item_index += 1;
                        continue;
                    }
                    return None;
                }
                Item::CaptureOpen(capture_index) => {
                    let saved = ctx.captures[*capture_index];
                    ctx.captures[*capture_index] = CaptureSlot {
                        start: pos,
                        end: None,
                    };
                    let result = self.do_match(ctx, pos, item_index + 1);
                    if result.is_none() {
                        ctx.captures[*capture_index] = saved;
                    }
                    return result;
                }
                Item::CaptureClose(capture_index) => {
                    let saved = ctx.captures[*capture_index];
                    ctx.captures[*capture_index].end = Some(pos);
                    let result = self.do_match(ctx, pos, item_index + 1);
                    if result.is_none() {
                        ctx.captures[*capture_index] = saved;
                    }
                    return result;
                }
            }
        }
    }

    fn match_class<'s>(&self, ctx: &MatchContext<'s>, pos: usize, class: &CharacterClass) -> bool {
        match ctx.input[pos..].chars().next() {
            Some(ch) => class.matches(ch),
            None => false,
        }
    }

    /// Greedily matches as many repetitions of `class` as possible.
    fn max_expand<'s>(
        &self,
        ctx: &mut MatchContext<'s>,
        start: usize,
        item_index: usize,
        class: &CharacterClass,
    ) -> Option<usize> {
        // consume as many repititions as possible and keep track of their positions
        let mut current_pos = start;
        let mut candidate_positions = vec![current_pos];
        while self.match_class(ctx, current_pos, class) {
            current_pos = advance(ctx.input, current_pos);
            candidate_positions.push(current_pos);
        }

        // backtrack from longest match until we find a position where the rest of the pattern also
        // matches
        for &pos in candidate_positions.iter().rev() {
            if let Some(end) = self.do_match(ctx, pos, item_index + 1) {
                return Some(end);
            }
        }

        None
    }

    /// Matches as few repetitions of `class` as possible.
    fn min_expand<'s>(
        &self,
        ctx: &mut MatchContext<'s>,
        start: usize,
        item_index: usize,
        class: &CharacterClass,
    ) -> Option<usize> {
        // start at zero repetitions and increase until rest of the pattern also matches.
        let mut current_pos = start;
        loop {
            if let Some(end) = self.do_match(ctx, current_pos, item_index + 1) {
                return Some(end);
            }
            if self.match_class(ctx, current_pos, class) {
                current_pos = advance(ctx.input, current_pos);
            } else {
                return None;
            }
        }
    }

    fn match_capture_ref<'s>(
        &self,
        ctx: &mut MatchContext<'s>,
        pos: usize,
        n: u8,
    ) -> Option<usize> {
        let index = (n as usize).checked_sub(1)?;
        let capture = ctx.captures.get(index)?;
        let end = capture.end?;
        let captured = &ctx.input[capture.start..end];
        if ctx.input[pos..].starts_with(captured) {
            Some(pos + captured.len())
        } else {
            None
        }
    }

    fn match_balanced<'s>(
        &self,
        ctx: &mut MatchContext<'s>,
        start_pos: usize,
        open: char,
        close: char,
    ) -> Option<usize> {
        let mut chars = ctx.input[start_pos..].char_indices();
        let (_, first) = chars.next()?;

        if first != open {
            return None;
        }

        let mut balance = 1;
        for (i, ch) in chars {
            if ch == open {
                balance += 1;
            } else if ch == close {
                balance -= 1;
                if balance == 0 {
                    return Some(start_pos + i + ch.len_utf8());
                }
            }
        }

        None
    }

    fn match_frontier<'s>(
        &self,
        ctx: &mut MatchContext<'s>,
        pos: usize,
        set: &CharacterSet,
    ) -> bool {
        let previous = if pos == 0 {
            '\0'
        } else {
            ctx.input[..pos].chars().next_back().unwrap_or('\0')
        };
        let next = ctx.input[pos..].chars().next().unwrap_or('\0');
        !set.matches(previous) && set.matches(next)
    }
}

fn advance(input: &str, pos: usize) -> usize {
    match input[pos..].chars().next() {
        Some(ch) => pos + ch.len_utf8(),
        None => pos,
    }
}

/// Whether a parsed character class matches a given `char`.
trait MatchesClass {
    fn matches(&self, ch: char) -> bool;
}

impl MatchesClass for CharacterClass {
    fn matches(&self, ch: char) -> bool {
        match self {
            CharacterClass::Literal(c) => *c == ch,
            CharacterClass::Any => true,
            CharacterClass::Predefined(pc) => pc.matches(ch),
            CharacterClass::Set(set) => set.matches(ch),
        }
    }
}

impl MatchesClass for CharacterSet {
    fn matches(&self, ch: char) -> bool {
        let found = self.elements.iter().any(|el| match el {
            SetElement::Literal(c) => *c == ch,
            SetElement::Class(class) => class.matches(ch),
            SetElement::Range(lo, hi) => *lo <= ch && ch <= *hi,
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
        Matcher::new(&pattern(p))
            .find(input)
            .map(|m| m.as_str().to_owned())
    }

    fn find_with_captures(p: &str, input: &str) -> Option<(String, Vec<String>)> {
        Matcher::new(&pattern(p)).find(input).map(|m| {
            (
                m.as_str().to_owned(),
                m.captures().map(str::to_owned).collect(),
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
        let result = Matcher::new(&pattern("(a(b(c)))")).find("abc").unwrap();

        assert_eq!(result.as_str(), "abc");
        assert_eq!(result.capture(0), Some("abc"));
        assert_eq!(result.capture(1), Some("bc"));
        assert_eq!(result.capture(2), Some("c"));
    }

    #[test]
    fn capture_can_be_empty() {
        let result = Matcher::new(&pattern("(a*)b")).find("b").unwrap();

        assert_eq!(result.as_str(), "b");
        assert_eq!(result.capture(0), Some(""));
    }

    #[test]
    fn capture_contains_greedy_match() {
        let result = Matcher::new(&pattern("(a*)b")).find("aaab").unwrap();

        assert_eq!(result.as_str(), "aaab");
        assert_eq!(result.capture(0), Some("aaa"));
    }

    #[test]
    fn capture_backtracks() {
        let result = Matcher::new(&pattern("(a*)a")).find("aaa").unwrap();

        assert_eq!(result.as_str(), "aaa");
        assert_eq!(result.capture(0), Some("aa"));
    }

    #[test]
    fn capture_backtracks_inside_nested_capture() {
        let result = Matcher::new(&pattern("((a*)a)")).find("aaa").unwrap();

        assert_eq!(result.as_str(), "aaa");
        assert_eq!(result.capture(0), Some("aaa"));
        assert_eq!(result.capture(1), Some("aa"));
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
        let result = Matcher::new(&pattern("(a)(b) %1%2")).find("ab ab").unwrap();
        assert_eq!(result.as_str(), "ab ab");
        assert_eq!(result.capture(0), Some("a"));
        assert_eq!(result.capture(1), Some("b"));
    }

    #[test]
    fn capture_reference_after_nested_capture() {
        let result = Matcher::new(&pattern("(a(b))%1")).find("abab").unwrap();

        assert_eq!(result.as_str(), "abab");
        assert_eq!(result.capture(0), Some("ab"));
        assert_eq!(result.capture(1), Some("b"));
    }

    #[test]
    fn failed_backtracking_does_not_corrupt_capture_state() {
        let result = Matcher::new(&pattern("(a*)ab")).find("aaab");

        assert!(result.is_some());

        let result = result.unwrap();

        assert_eq!(result.as_str(), "aaab");
        assert_eq!(result.capture(0), Some("aa"));
    }

    #[test]
    fn capture_state_is_restored_between_backtracking_branches() {
        let result = Matcher::new(&pattern("(a*)(b*)c")).find("aaabbc").unwrap();

        assert_eq!(result.as_str(), "aaabbc");
        assert_eq!(result.capture(0), Some("aaa"));
        assert_eq!(result.capture(1), Some("bb"));
    }

    #[test]
    fn nested_capture_backtracking() {
        let result = Matcher::new(&pattern("((a*)b*)c")).find("aaabbc").unwrap();

        assert_eq!(result.as_str(), "aaabbc");
        assert_eq!(result.capture(0), Some("aaabb"));
        assert_eq!(result.capture(1), Some("aaa"));
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
        let result = Matcher::new(&pattern("%f[%a]hello")).find("hello").unwrap();

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
        let input = "é";
        let result = Matcher::new(&pattern(".")).find(input).unwrap();

        assert_eq!(result.as_str(), "é");
        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 2);
    }

    #[test]
    fn matching_four_byte_character() {
        let input = "😀";
        let result = Matcher::new(&pattern(".")).find(input).unwrap();

        assert_eq!(result.as_str(), "😀");
        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 4);
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
        let result = Matcher::new(&pattern("(%a+)")).find("hello 안녕").unwrap();

        assert_eq!(result.capture(0), Some("hello"));
    }

    #[test]
    fn unicode_capture_span_is_byte_based() {
        let result = Matcher::new(&pattern("(é)")).find("é").unwrap();

        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 2);
        assert_eq!(result.capture(0), Some("é"));
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
        let result = Matcher::new(&pattern(".")).find("éx").unwrap();

        assert_eq!(result.start(), 0);
    }

    #[test]
    fn find_returns_match_at_end_for_empty_pattern() {
        let result = Matcher::new(&pattern("")).find("abc").unwrap();

        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 0);
    }

    #[test]
    fn match_offsets_are_byte_offsets() {
        let input = "hello 안녕";
        let result = Matcher::new(&pattern("안녕")).find(input).unwrap();

        assert_eq!(&input[result.start()..result.end()], "안녕");
        assert_eq!(result.start(), 6);
        assert_eq!(result.end(), 12);
    }

    #[test]
    fn capture_offsets_are_byte_offsets() {
        let input = "hello 안녕";
        let result = Matcher::new(&pattern("(안녕)")).find(input).unwrap();

        assert_eq!(result.capture(0), Some("안녕"));
    }

    #[test]
    fn star_can_produce_empty_match() {
        let result = Matcher::new(&pattern("a*")).find("bbb").unwrap();

        assert_eq!(result.as_str(), "");
        assert_eq!(result.start(), 0);
        assert_eq!(result.end(), 0);
    }

    #[test]
    fn optional_can_produce_empty_match() {
        let result = Matcher::new(&pattern("a?")).find("bbb").unwrap();

        assert_eq!(result.as_str(), "");
    }

    #[test]
    fn frontier_is_zero_width() {
        let result = Matcher::new(&pattern("%f[%a]")).find(" abc").unwrap();

        assert_eq!(result.as_str(), "");
        assert_eq!(result.start(), 1);
        assert_eq!(result.end(), 1);
    }

    #[test]
    fn empty_match_can_occur_at_eof() {
        let result = Matcher::new(&pattern("$")).find("abc").unwrap();

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
        let result = Matcher::new(&pattern("(.*)x")).find("abcx").unwrap();

        assert_eq!(result.as_str(), "abcx");
        assert_eq!(result.capture(0), Some("abc"));
    }

    #[test]
    fn greedy_capture_backtracks_to_last_literal() {
        let result = Matcher::new(&pattern("(.*)x")).find("abcxdefx").unwrap();

        assert_eq!(result.as_str(), "abcxdefx");
        assert_eq!(result.capture(0), Some("abcxdef"));
    }

    #[test]
    fn lazy_capture_stops_at_first_literal() {
        let result = Matcher::new(&pattern("(.-)x")).find("abcxdefx").unwrap();

        assert_eq!(result.as_str(), "abcx");
        assert_eq!(result.capture(0), Some("abc"));
    }

    #[test]
    fn greedy_capture_can_backtrack_to_empty() {
        let result = Matcher::new(&pattern("(.*)x")).find("x").unwrap();

        assert_eq!(result.capture(0), Some(""));
    }

    #[test]
    fn lazy_capture_can_match_empty() {
        let result = Matcher::new(&pattern("(.-)x")).find("x").unwrap();

        assert_eq!(result.capture(0), Some(""));
    }
}
