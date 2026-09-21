//! Data structures to represent a Lua pattern and the associated parser.
//! Used for reference:
//! - https://www.lua.org/pil/20.2.html
//! - https://www.lua.org/manual/5.3/manual.html#6.4.1

use std::fmt;

use crate::{
    matching::{Match, Matcher, Matches},
    replacement::Replacer,
};

/// A parsed Lua pattern.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Pattern {
    sequence: Vec<Item>,
    /// `^` at start of pattern.
    anchored_start: bool,
    /// `$` at end of pattern.
    anchored_end: bool,
    /// Total number of captures in this pattern.
    capture_count: usize,
}

impl Pattern {
    /// Parses pattern string into a reusable `Pattern`.
    pub fn parse(source: &str) -> Result<Self, PatternError> {
        Parser::new(source).parse()
    }

    /// Finds the first match anywhere in the `input`.
    pub fn find<'s>(&self, input: &'s str) -> Option<Match<'s>> {
        Matcher::new(self).find(input)
    }

    /// Creates an iterator over all matches.
    pub fn find_all<'p, 's>(&'p self, input: &'s str) -> Matches<'p, 's> {
        Matches::new(self, input)
    }

    /// Replaces all matches using a `Replacement`. `limit` limits the number of replacements to
    /// perform.
    pub fn replace<F, E>(
        &self,
        input: &str,
        replacement: F,
        limit: Option<usize>,
    ) -> Result<(String, usize), E>
    where
        F: Fn(&Match) -> Result<Option<String>, E>,
    {
        Replacer::new(self).replace_with(input, replacement, limit)
    }

    pub fn sequence(&self) -> &[Item] {
        &self.sequence
    }
    pub fn anchored_start(&self) -> bool {
        self.anchored_start
    }
    pub fn anchored_end(&self) -> bool {
        self.anchored_end
    }
    pub fn capture_count(&self) -> usize {
        self.capture_count
    }
}

/// Parses a Lua pattern into a `Pattern`.
#[derive(Debug)]
struct Parser<'s> {
    input: &'s str,
    /// Byte offset into `input`.
    pos: usize,
    /// Number assinged to the next capture.
    next_capture: usize,
    /// Stack for keeping track of opened captures.
    open_capture_stack: Vec<usize>,
}

impl<'s> Parser<'s> {
    fn new(input: &'s str) -> Self {
        Self {
            input,
            pos: 0,
            next_capture: 0,
            open_capture_stack: Vec::new(),
        }
    }

    /// Consumes `self` and parses `self.input` into a `Pattern`.
    fn parse(mut self) -> Result<Pattern, PatternError> {
        let anchored_start = self.consume_if('^');
        let sequence = self.parse_sequence(None)?;
        let anchored_end = self.consume_if('$');

        if !self.open_capture_stack.is_empty() {
            return Err(self.error("unmatched capture open"));
        }

        if !self.at_end() {
            return Err(self.error("unexpected character after pattern"));
        }

        Ok(Pattern {
            sequence,
            anchored_start,
            anchored_end,
            capture_count: self.next_capture,
        })
    }

    fn parse_sequence(&mut self, terminator: Option<char>) -> Result<Vec<Item>, PatternError> {
        let mut items = Vec::new();

        while !self.at_end() {
            let current = self.peek().expect("not at end");

            if Some(current) == terminator {
                break;
            }

            // let `parse` handle trailing '$'
            if terminator.is_none() && current == '$' && self.remaining() == "$" {
                break;
            }

            items.push(self.parse_item()?);
        }

        if terminator.is_some() && self.at_end() {
            return Err(self.error("unterminated sequence"));
        }

        Ok(items)
    }

    fn parse_item(&mut self) -> Result<Item, PatternError> {
        match self.next() {
            Some('(') => {
                let index = self.next_capture;
                self.next_capture += 1;
                self.open_capture_stack.push(index);
                Ok(Item::CaptureOpen(index))
            }
            Some(')') => {
                let index = self
                    .open_capture_stack
                    .pop()
                    .ok_or(self.error("unmatched capture close"))?;
                Ok(Item::CaptureClose(index))
            }
            Some('[') => {
                let set = self.parse_set()?;
                let modifier = self.parse_modifier();
                Ok(Item::Class {
                    class: CharacterClass::Set(set),
                    modifier,
                })
            }
            Some('%') => self.parse_percent_item(),
            Some('.') => {
                let modifier = self.parse_modifier();
                Ok(Item::Class {
                    class: CharacterClass::Any,
                    modifier,
                })
            }
            Some(ch) => {
                let modifier = self.parse_modifier();
                Ok(Item::Class {
                    class: CharacterClass::Literal(ch),
                    modifier,
                })
            }
            None => Err(self.error("unexpected end of pattern")),
        }
    }

    fn parse_set(&mut self) -> Result<CharacterSet, PatternError> {
        let complement = self.consume_if('^');
        let mut elements = Vec::new();

        // check for ']' literal at start
        if self.consume_if(']') {
            elements.push(SetElement::Literal(']'));
        }

        while !self.at_end() {
            if self.peek() == Some(']') {
                break;
            }

            let element = self.parse_set_element()?;

            // check for range
            if let SetElement::Literal(start) = element
                && self.consume_if('-')
            {
                match self.peek() {
                    // '-' at end is a literal
                    Some(']') => {
                        elements.push(SetElement::Literal(start));
                        elements.push(SetElement::Literal('-'));
                    }
                    Some(ch) => {
                        // range
                        self.next();
                        elements.push(SetElement::Range(start, ch));
                    }
                    None => Err(self.error("unexpected end of pattern"))?,
                }
                continue;
            }

            elements.push(element);
        }

        self.expect(']')?;

        Ok(CharacterSet {
            complement,
            elements,
        })
    }

    fn parse_set_element(&mut self) -> Result<SetElement, PatternError> {
        match self.next() {
            Some('%') => {
                let next = self
                    .next()
                    .ok_or_else(|| self.error("unexpected end of pattern"))?;

                if let Some(class) = PredefinedClass::from_char(next) {
                    Ok(SetElement::Class(class))
                } else {
                    // escaped magic characters such as %- / %% / etc.
                    Ok(SetElement::Literal(next))
                }
            }
            Some(ch) => Ok(SetElement::Literal(ch)),
            None => Err(self.error("unexpected end of pattern")),
        }
    }

    /// Parses everything beginning with '%' such as predefined classes, capture references,
    /// balanced patterns, frontier patterns, and escaped literals.
    fn parse_percent_item(&mut self) -> Result<Item, PatternError> {
        let ch = self
            .next()
            .ok_or_else(|| self.error("pattern ends with '%'"))?;

        match ch {
            'b' => self.parse_balanced(),
            'f' => self.parse_frontier(),
            '1'..='9' => Ok(Item::CaptureRef(ch.to_digit(10).unwrap() as u8)),
            '0' => Err(self.error("invalid capture index &0")),
            c if c.is_ascii_alphabetic() => {
                if let Some(class) = PredefinedClass::from_char(c) {
                    let modifier = self.parse_modifier();
                    Ok(Item::Class {
                        class: CharacterClass::Predefined(class),
                        modifier,
                    })
                } else {
                    let modifier = self.parse_modifier();
                    Ok(Item::Class {
                        class: CharacterClass::Literal(c),
                        modifier,
                    })
                }
            }
            // escaped magic characters
            c => {
                let modifier = self.parse_modifier();
                Ok(Item::Class {
                    class: CharacterClass::Literal(c),
                    modifier,
                })
            }
        }
    }

    fn parse_balanced(&mut self) -> Result<Item, PatternError> {
        let open = self
            .next()
            .ok_or_else(|| self.error("missing opening character for '%b'"))?;
        let close = self
            .next()
            .ok_or_else(|| self.error("missing closing character for '%b'"))?;

        if open == close {
            return Err(self.error("'%b' needs two distinct characters"));
        }

        Ok(Item::Balanced { open, close })
    }

    fn parse_frontier(&mut self) -> Result<Item, PatternError> {
        self.expect('[')?;
        let set = self.parse_set()?;
        Ok(Item::Frontier(set))
    }

    fn parse_modifier(&mut self) -> Option<Modifier> {
        let modifier = match self.peek()? {
            '*' => Modifier::ZeroOrMoreGreedy,
            '+' => Modifier::OneOrMore,
            '-' => Modifier::ZeroOrMoreLazy,
            '?' => Modifier::ZeroOrOne,
            _ => return None,
        };
        self.next();
        Some(modifier)
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn at_end(&self) -> bool {
        self.pos == self.input.len()
    }

    fn remaining(&self) -> &str {
        &self.input[self.pos..]
    }

    /// Returns `true` and consumes the next `char` if it matches `ch`.
    fn consume_if(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), PatternError> {
        if self.consume_if(expected) {
            Ok(())
        } else {
            Err(self.error(format!("expected '{}'", expected)))
        }
    }

    fn error(&self, message: impl Into<String>) -> PatternError {
        PatternError::at(self.pos, message)
    }
}

/// Pattern item.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Item {
    /// Single character class with optinal repetition modifier.
    Class {
        class: CharacterClass,
        modifier: Option<Modifier>,
    },
    /// Reference to a previous capture group, e.g. `%1`.
    CaptureRef(u8),
    /// Balanced , e.g. `%b()`
    Balanced { open: char, close: char },
    /// Frontier Pattern, e.g. `%f[a-z]`.
    Frontier(CharacterSet),
    /// Opening parenthesis of a capture.
    CaptureOpen(usize),
    /// Closing parenthesis of a capture.
    CaptureClose(usize),
}

/// A Lua character class.
/// Used to represent a set of characters.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CharacterClass {
    /// Literal Unicode scalar value.
    Literal(char),
    /// `.`: any Unicode scalar value.
    Any,
    /// `%x`: predefined character class.
    Predefined(PredefinedClass),
    /// `[set]`: character set.
    Set(CharacterSet),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PredefinedClass {
    pub(crate) kind: PredefinedClassKind,
    pub(crate) complement: bool,
}

impl PredefinedClass {
    fn from_char(ch: char) -> Option<Self> {
        let kind = match ch.to_ascii_lowercase() {
            'a' => PredefinedClassKind::Letter,
            'c' => PredefinedClassKind::Control,
            'd' => PredefinedClassKind::Digit,
            'g' => PredefinedClassKind::Printable,
            'l' => PredefinedClassKind::Lowercase,
            'p' => PredefinedClassKind::Punctuation,
            's' => PredefinedClassKind::Space,
            'u' => PredefinedClassKind::Uppercase,
            'w' => PredefinedClassKind::Alphanumeric,
            'x' => PredefinedClassKind::HexDigit,
            'z' => PredefinedClassKind::Nul,
            _ => return None,
        };

        Some(Self {
            kind,
            complement: ch.is_ascii_uppercase(),
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PredefinedClassKind {
    /// `%a`: letter (alphabetic).
    Letter,
    /// `%c`: control character.
    Control,
    /// `%d`: decimal digit.
    Digit,
    /// `%g`: printable character except space
    Printable,
    /// `%l`: lower case letter.
    Lowercase,
    /// `%p`: punctuation character.
    Punctuation,
    /// `%s`: space character.
    Space,
    /// `%u`: upper case character.
    Uppercase,
    /// `%w`: alphanumeric character.
    Alphanumeric,
    /// `%x`: hexadecimal digit.
    HexDigit,
    /// `%z`: null terminator.
    Nul,
}

/// A Lua character set.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct CharacterSet {
    pub(crate) complement: bool,
    pub(crate) elements: Vec<SetElement>,
}

/// A Lua character set element.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum SetElement {
    /// A literal character
    Literal(char),
    /// predefined character class
    Class(PredefinedClass),
    /// A character range
    Range(char, char),
}

/// A Lua pattern modifier.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum Modifier {
    /// `*`: zero or more. Matches longest possible sequence.
    ZeroOrMoreGreedy,
    /// `+`: one or more.
    OneOrMore,
    /// `-`: zero or more. Matches shortest possible sequence.
    ZeroOrMoreLazy,
    /// `?`: zero or one.
    ZeroOrOne,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PatternError {
    message: String,
    /// Offset into the pattern
    position: usize,
}

impl PatternError {
    fn at(position: usize, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.position)
    }
}

impl std::error::Error for PatternError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Pattern {
        Pattern::parse(source).unwrap_or_else(|e| panic!("failed to parse {source:?}: {e}"))
    }

    fn parse_err(source: &str) -> PatternError {
        Pattern::parse(source).expect_err(&format!("expected {source:?} to fail"))
    }

    fn literal(c: char) -> Item {
        Item::Class {
            class: CharacterClass::Literal(c),
            modifier: None,
        }
    }

    fn any() -> Item {
        Item::Class {
            class: CharacterClass::Any,
            modifier: None,
        }
    }

    fn class(class: PredefinedClass) -> Item {
        Item::Class {
            class: CharacterClass::Predefined(class),
            modifier: None,
        }
    }

    fn modified(class: CharacterClass, modifier: Modifier) -> Item {
        Item::Class {
            class,
            modifier: Some(modifier),
        }
    }

    fn predefined(kind: PredefinedClassKind) -> PredefinedClass {
        PredefinedClass {
            kind,
            complement: false,
        }
    }

    fn complemented(kind: PredefinedClassKind) -> PredefinedClass {
        PredefinedClass {
            kind,
            complement: true,
        }
    }

    #[test]
    fn parses_empty_pattern() {
        let pattern = parse("");

        assert_eq!(pattern.sequence, vec![]);
        assert!(!pattern.anchored_start);
        assert!(!pattern.anchored_end);
        assert_eq!(pattern.capture_count, 0);
    }

    #[test]
    fn parses_literal() {
        let pattern = parse("a");

        assert_eq!(pattern.sequence, vec![literal('a')]);
    }

    #[test]
    fn parses_multiple_literals() {
        let pattern = parse("abcd");

        assert_eq!(
            pattern.sequence,
            vec![literal('a'), literal('b'), literal('c'), literal('d'),]
        );
    }

    #[test]
    fn parses_any_character() {
        let pattern = parse(".");

        assert_eq!(pattern.sequence, vec![any()]);
    }

    #[test]
    fn parses_any_characters() {
        let pattern = parse("...");

        assert_eq!(pattern.sequence, vec![any(), any(), any()]);
    }

    #[test]
    fn parses_start_anchor() {
        let pattern = parse("^abc");

        assert!(pattern.anchored_start);
        assert!(!pattern.anchored_end);
        assert_eq!(
            pattern.sequence,
            vec![literal('a'), literal('b'), literal('c')]
        );
    }

    #[test]
    fn parses_end_anchor() {
        let pattern = parse("abc$");

        assert!(!pattern.anchored_start);
        assert!(pattern.anchored_end);
        assert_eq!(
            pattern.sequence,
            vec![literal('a'), literal('b'), literal('c')]
        );
    }

    #[test]
    fn parses_both_anchors() {
        let pattern = parse("^abc$");

        assert!(pattern.anchored_start);
        assert!(pattern.anchored_end);
        assert_eq!(
            pattern.sequence,
            vec![literal('a'), literal('b'), literal('c')]
        );
    }

    #[test]
    fn parses_start_anchor_in_middle_as_literal() {
        let pattern = parse("a^b");

        assert!(!pattern.anchored_start);
        assert_eq!(
            pattern.sequence,
            vec![literal('a'), literal('^'), literal('b')]
        );
    }

    #[test]
    fn parses_end_anchor_in_middle_as_literal() {
        let pattern = parse("a$b");

        assert!(!pattern.anchored_end);
        assert_eq!(
            pattern.sequence,
            vec![literal('a'), literal('$'), literal('b')]
        );
    }

    #[test]
    fn parses_start_anchor_alone() {
        let pattern = parse("^");

        assert!(pattern.anchored_start);
        assert!(pattern.sequence.is_empty());
    }

    #[test]
    fn parses_end_anchor_alone() {
        let pattern = parse("$");

        assert!(pattern.anchored_end);
        assert!(pattern.sequence.is_empty());
    }

    #[test]
    fn parses_modifiers() {
        let pattern = parse("a*b+c-d?");

        assert_eq!(
            pattern.sequence,
            vec![
                modified(CharacterClass::Literal('a'), Modifier::ZeroOrMoreGreedy,),
                modified(CharacterClass::Literal('b'), Modifier::OneOrMore,),
                modified(CharacterClass::Literal('c'), Modifier::ZeroOrMoreLazy,),
                modified(CharacterClass::Literal('d'), Modifier::ZeroOrOne,)
            ]
        );
    }

    #[test]
    fn parses_consecutive_modifiers() {
        let pattern = parse("a**");

        assert_eq!(
            pattern.sequence,
            vec![
                modified(CharacterClass::Literal('a'), Modifier::ZeroOrMoreGreedy,),
                literal('*'),
            ]
        );
    }

    #[test]
    fn parses_predefined_classes() {
        let pattern = parse("%a%c%d%g%l%p%s%u%w%x%z");

        assert_eq!(
            pattern.sequence,
            vec![
                class(predefined(PredefinedClassKind::Letter)),
                class(predefined(PredefinedClassKind::Control)),
                class(predefined(PredefinedClassKind::Digit)),
                class(predefined(PredefinedClassKind::Printable)),
                class(predefined(PredefinedClassKind::Lowercase)),
                class(predefined(PredefinedClassKind::Punctuation)),
                class(predefined(PredefinedClassKind::Space)),
                class(predefined(PredefinedClassKind::Uppercase)),
                class(predefined(PredefinedClassKind::Alphanumeric)),
                class(predefined(PredefinedClassKind::HexDigit)),
                class(predefined(PredefinedClassKind::Nul)),
            ]
        );
    }

    #[test]
    fn parses_complement_predefined_classes() {
        let pattern = parse("%A%C%D%G%L%P%S%U%W%X%Z");

        assert_eq!(
            pattern.sequence,
            vec![
                class(complemented(PredefinedClassKind::Letter)),
                class(complemented(PredefinedClassKind::Control)),
                class(complemented(PredefinedClassKind::Digit)),
                class(complemented(PredefinedClassKind::Printable)),
                class(complemented(PredefinedClassKind::Lowercase)),
                class(complemented(PredefinedClassKind::Punctuation)),
                class(complemented(PredefinedClassKind::Space)),
                class(complemented(PredefinedClassKind::Uppercase)),
                class(complemented(PredefinedClassKind::Alphanumeric)),
                class(complemented(PredefinedClassKind::HexDigit)),
                class(complemented(PredefinedClassKind::Nul)),
            ]
        );
    }

    #[test]
    fn predefined_class_can_have_modifier() {
        let pattern = parse("%a+");

        assert_eq!(
            pattern.sequence,
            vec![modified(
                CharacterClass::Predefined(predefined(PredefinedClassKind::Letter)),
                Modifier::OneOrMore,
            )]
        );
    }

    #[test]
    fn parses_escaped_percent() {
        let pattern = parse("%%");

        assert_eq!(pattern.sequence, vec![literal('%')]);
    }

    #[test]
    fn parses_escaped_magic_characters() {
        let pattern = parse("%.%[%]%(%)%*%+%-%?%^%$");

        assert_eq!(
            pattern.sequence,
            vec![
                literal('.'),
                literal('['),
                literal(']'),
                literal('('),
                literal(')'),
                literal('*'),
                literal('+'),
                literal('-'),
                literal('?'),
                literal('^'),
                literal('$'),
            ]
        );
    }

    #[test]
    fn unknown_percent_escape_is_literal() {
        let pattern = parse("%q");

        assert_eq!(pattern.sequence, vec![literal('q')]);
    }

    #[test]
    fn parses_simple_character_set() {
        let pattern = parse("[abc]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Literal('a'),
                        SetElement::Literal('b'),
                        SetElement::Literal('c'),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_character_range() {
        let pattern = parse("[a-z]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Range('a', 'z'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_multiple_ranges() {
        let pattern = parse("[a-zA-Z0-9]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Range('a', 'z'),
                        SetElement::Range('A', 'Z'),
                        SetElement::Range('0', '9'),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_complement_set() {
        let pattern = parse("[^abc]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: true,
                    elements: vec![
                        SetElement::Literal('a'),
                        SetElement::Literal('b'),
                        SetElement::Literal('c'),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_set_with_predefined_class() {
        let pattern = parse("[%a%d]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Class(predefined(PredefinedClassKind::Letter)),
                        SetElement::Class(predefined(PredefinedClassKind::Digit)),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_set_with_complemented_class() {
        let pattern = parse("[%a%D]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Class(predefined(PredefinedClassKind::Letter)),
                        SetElement::Class(complemented(PredefinedClassKind::Digit)),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_set_with_literal_and_range() {
        let pattern = parse("[a-z_]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Range('a', 'z'), SetElement::Literal('_'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_set_modifier() {
        let pattern = parse("[a-z]+");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Range('a', 'z'),],
                }),
                modifier: Some(Modifier::OneOrMore),
            }]
        );
    }

    #[test]
    fn parses_literal_closing_bracket_at_start_of_set() {
        let pattern = parse("[]]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Literal(']'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_literal_dash_at_end_of_set() {
        let pattern = parse("[a-]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Literal('a'), SetElement::Literal('-'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_literal_dash_at_start_of_set() {
        let pattern = parse("[-a]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Literal('-'), SetElement::Literal('a'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_escaped_dash_in_set() {
        let pattern = parse("[a%-z]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Literal('a'),
                        SetElement::Literal('-'),
                        SetElement::Literal('z'),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    // NOTE: the reference manual is a bit unclear on this but this is what luautf8 does.
    #[test]
    fn parses_literal_dash_after_class_in_set() {
        let pattern = parse("[%a-z]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Class(predefined(PredefinedClassKind::Letter)),
                        SetElement::Literal('-'),
                        SetElement::Literal('z'),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    // NOTE: following luautf8 behavior
    #[test]
    fn parses_literal_percent_as_range_end_in_set() {
        let pattern = parse("[a-%z]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Range('a', '%'), SetElement::Literal('z'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_balanced_pattern() {
        let pattern = parse("%b()");

        assert_eq!(
            pattern.sequence,
            vec![Item::Balanced {
                open: '(',
                close: ')',
            }]
        );
    }

    #[test]
    fn parses_balanced_pattern_with_arbitrary_delimiters() {
        let pattern = parse("%b[]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Balanced {
                open: '[',
                close: ']',
            }]
        );
    }

    #[test]
    fn parses_balanced_pattern_with_unicode_delimiters() {
        let pattern = parse("%b《》");

        assert_eq!(
            pattern.sequence,
            vec![Item::Balanced {
                open: '《',
                close: '》',
            }]
        );
    }

    #[test]
    fn rejects_incomplete_balanced_pattern() {
        parse_err("%b");
    }

    #[test]
    fn rejects_balanced_pattern_with_one_delimiter() {
        parse_err("%b(");
    }

    #[test]
    fn rejects_balanced_pattern_with_identical_delimiters() {
        parse_err("%b((");
    }

    #[test]
    fn parses_frontier() {
        let pattern = parse("%f[%a]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Frontier(CharacterSet {
                complement: false,
                elements: vec![SetElement::Class(predefined(PredefinedClassKind::Letter)),],
            })]
        );
    }

    #[test]
    fn parses_complemented_frontier_set() {
        let pattern = parse("%f[^%d]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Frontier(CharacterSet {
                complement: true,
                elements: vec![SetElement::Class(predefined(PredefinedClassKind::Digit)),],
            })]
        );
    }

    #[test]
    fn rejects_frontier_without_set() {
        parse_err("%f");
    }

    #[test]
    fn rejects_frontier_without_opening_bracket() {
        parse_err("%fa");
    }

    #[test]
    fn rejects_unterminated_frontier_set() {
        parse_err("%f[abc");
    }

    #[test]
    fn parses_capture() {
        let pattern = parse("(abc)");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureOpen(0),
                literal('a'),
                literal('b'),
                literal('c'),
                Item::CaptureClose(0),
            ]
        );

        assert_eq!(pattern.capture_count, 1);
    }

    #[test]
    fn parses_multiple_captures() {
        let pattern = parse("(a)(b)(c)");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureOpen(0),
                literal('a'),
                Item::CaptureClose(0),
                Item::CaptureOpen(1),
                literal('b'),
                Item::CaptureClose(1),
                Item::CaptureOpen(2),
                literal('c'),
                Item::CaptureClose(2),
            ]
        );

        assert_eq!(pattern.capture_count, 3);
    }

    #[test]
    fn numbers_nested_captures_globally() {
        let pattern = parse("(a(b(c)))");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureOpen(0),
                literal('a'),
                Item::CaptureOpen(1),
                literal('b'),
                Item::CaptureOpen(2),
                literal('c'),
                Item::CaptureClose(2),
                Item::CaptureClose(1),
                Item::CaptureClose(0),
            ]
        );

        assert_eq!(pattern.capture_count, 3);
    }

    #[test]
    fn capture_numbers_continue_after_nested_capture() {
        let pattern = parse("(a(b))(c)");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureOpen(0),
                literal('a'),
                Item::CaptureOpen(1),
                literal('b'),
                Item::CaptureClose(1),
                Item::CaptureClose(0),
                Item::CaptureOpen(2),
                literal('c'),
                Item::CaptureClose(2),
            ]
        );

        assert_eq!(pattern.capture_count, 3);
    }

    #[test]
    fn parses_empty_capture() {
        let pattern = parse("()");

        assert_eq!(
            pattern.sequence,
            vec![Item::CaptureOpen(0), Item::CaptureClose(0),]
        );

        assert_eq!(pattern.capture_count, 1);
    }

    #[test]
    fn parses_multiple_empty_captures() {
        let pattern = parse("()()");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureOpen(0),
                Item::CaptureClose(0),
                Item::CaptureOpen(1),
                Item::CaptureClose(1),
            ]
        );

        assert_eq!(pattern.capture_count, 2);
    }

    #[test]
    fn parses_capture_references() {
        let pattern = parse("%1%2%3%4%5%6%7%8%9");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureRef(1),
                Item::CaptureRef(2),
                Item::CaptureRef(3),
                Item::CaptureRef(4),
                Item::CaptureRef(5),
                Item::CaptureRef(6),
                Item::CaptureRef(7),
                Item::CaptureRef(8),
                Item::CaptureRef(9),
            ]
        );
    }

    #[test]
    fn capture_reference_can_appear_between_items() {
        let pattern = parse("(%a)%1");

        assert_eq!(
            pattern.sequence,
            vec![
                Item::CaptureOpen(0),
                Item::Class {
                    class: CharacterClass::Predefined(predefined(PredefinedClassKind::Letter)),
                    modifier: None,
                },
                Item::CaptureClose(0),
                Item::CaptureRef(1),
            ]
        );
    }

    #[test]
    fn parses_unicode_literals() {
        let pattern = parse("héllo안녕");

        assert_eq!(
            pattern.sequence,
            vec![
                literal('h'),
                literal('é'),
                literal('l'),
                literal('l'),
                literal('o'),
                literal('안'),
                literal('녕'),
            ]
        );
    }

    #[test]
    fn parses_unicode_set_literals() {
        let pattern = parse("[äöü]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![
                        SetElement::Literal('ä'),
                        SetElement::Literal('ö'),
                        SetElement::Literal('ü'),
                    ],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_unicode_set_range() {
        let pattern = parse("[α-ω]");

        assert_eq!(
            pattern.sequence,
            vec![Item::Class {
                class: CharacterClass::Set(CharacterSet {
                    complement: false,
                    elements: vec![SetElement::Range('α', 'ω'),],
                }),
                modifier: None,
            }]
        );
    }

    #[test]
    fn parses_unicode_balanced_delimiters() {
        let pattern = parse("%b《》");

        assert_eq!(
            pattern.sequence,
            vec![Item::Balanced {
                open: '《',
                close: '》',
            }]
        );
    }

    #[test]
    fn unicode_before_error_does_not_corrupt_parser_position() {
        let error = parse_err("αβ[abc");

        assert!(
            error.position >= "αβ".len(),
            "unexpected error position: {}",
            error.position
        );
    }

    #[test]
    fn rejects_unterminated_capture() {
        parse_err("(abc");
    }

    #[test]
    fn rejects_unexpected_closing_parenthesis() {
        parse_err("abc)");
    }

    #[test]
    fn rejects_unterminated_set() {
        parse_err("[abc");
    }

    #[test]
    fn rejects_trailing_percent() {
        parse_err("abc%");
    }

    #[test]
    fn rejects_unterminated_balanced_pattern() {
        parse_err("%b(");
    }

    #[test]
    fn rejects_unterminated_frontier() {
        parse_err("%f[abc");
    }

    #[test]
    fn parses_realistic_identifier_pattern() {
        let pattern = parse("^%a[%w_]*$");

        assert!(pattern.anchored_start);
        assert!(pattern.anchored_end);
        assert_eq!(pattern.capture_count, 0);

        assert_eq!(
            pattern.sequence,
            vec![
                Item::Class {
                    class: CharacterClass::Predefined(predefined(PredefinedClassKind::Letter)),
                    modifier: None,
                },
                Item::Class {
                    class: CharacterClass::Set(CharacterSet {
                        complement: false,
                        elements: vec![
                            SetElement::Class(predefined(PredefinedClassKind::Alphanumeric)),
                            SetElement::Literal('_'),
                        ],
                    }),
                    modifier: Some(Modifier::ZeroOrMoreGreedy),
                },
            ]
        );
    }

    #[test]
    fn parses_complex_pattern() {
        let pattern = parse("^(%a+)%s*(%d+)%s*=%s*(%b())$");

        assert!(pattern.anchored_start);
        assert!(pattern.anchored_end);
        assert_eq!(pattern.capture_count, 3);

        assert_eq!(pattern.sequence.len(), 13);
    }

    #[test]
    fn parses_frontier_between_other_items() {
        let pattern = parse("%f[%a]%a+%f[^%a]");

        assert_eq!(pattern.sequence.len(), 3);

        assert!(matches!(pattern.sequence[0], Item::Frontier(_)));

        assert!(matches!(
            pattern.sequence[1],
            Item::Class {
                modifier: Some(Modifier::OneOrMore),
                ..
            }
        ));

        assert!(matches!(pattern.sequence[2], Item::Frontier(_)));
    }

    #[test]
    fn valid_patterns() {
        let patterns = [
            "",
            "a",
            ".",
            "^abc$",
            "%a+",
            "[a-z]*",
            "[^%d]+",
            "(%a+)",
            "(a(b(c)))",
            "%b()",
            "%f[%a]",
            "%%",
            "%.",
            "[%-]",
            "[-a]",
            "[a-]",
            "αβγ",
            "[α-ω]",
        ];

        for source in patterns {
            Pattern::parse(source).unwrap_or_else(|e| panic!("failed to parse {source:?}: {e}"));
        }
    }

    #[test]
    fn invalid_patterns() {
        let patterns = [
            "(", "(abc", ")", "[", "[abc", "%", "%b", "%b(", "%f", "%fa", "%f[", "%0",
        ];

        for source in patterns {
            assert!(
                Pattern::parse(source).is_err(),
                "{source:?} unexpectedly parsed successfully"
            );
        }
    }
}
