pub mod character_class;
mod tokenizer;

use character_class::CharacterClass;
use unic_ucd_category::GeneralCategory;

use self::tokenizer::RegexToken;

#[derive(Debug, Eq, PartialEq)]
pub enum RegexEntry {
    AnyCharacter,
    UnicodeCharacterClass(Vec<GeneralCategory>),
    NegatedUnicodeCharacterClass(Vec<GeneralCategory>),
    NonUnicodeCharacterClass(CharacterClass),
    Concatenation(Vec<RegexEntry>),
    Alternation(Vec<RegexEntry>),
    Repetition {
        base: Box<RegexEntry>,
        min: Option<u64>,
        max: Option<u64>,
    },
}

//it would be cleaner (but perhaps overengineered) to have a separate enum with specific states (i.e., repetition, etc.) for each transformation stage
#[derive(PartialEq, Eq, Debug, Clone)]
enum PartiallyParsed {
    Lexed(RegexToken),
    Group(Vec<PartiallyParsed>),
    Repetition {
        base: Box<PartiallyParsed>,
        min: Option<u64>,
        max: Option<u64>,
    },
    Alternation(Vec<PartiallyParsed>),
}

impl RegexEntry {
    pub fn parse(regex: &str) -> Result<Self, String> {
        let lexed = Self::lex(regex)?;
        let grouped = Self::group(lexed);
        let repetitions = Self::parse_repetitions(grouped)?;

        unimplemented!()
    }

    fn lex(regex: &str) -> Result<Vec<PartiallyParsed>, String> {
        Ok(RegexToken::parse(regex)?
            .into_iter()
            .map(|token| PartiallyParsed::Lexed(token))
            .collect())
    }

    fn group(mut input: Vec<PartiallyParsed>) -> Vec<PartiallyParsed> {
        fn parse_group(input: &mut impl Iterator<Item = PartiallyParsed>) -> Vec<PartiallyParsed> {
            let mut output = Vec::new();

            while let Some(next) = input.next() {
                let part = match next {
                    PartiallyParsed::Lexed(RegexToken::OpenGroup) => {
                        PartiallyParsed::Group(parse_group(input))
                    }
                    PartiallyParsed::Lexed(RegexToken::CloseGroup) => break,
                    partial => partial,
                };
                output.push(part);
            }

            output
        }

        parse_group(&mut input.into_iter())
    }

    fn parse_repetitions(mut input: Vec<PartiallyParsed>) -> Result<Vec<PartiallyParsed>, String> {
        let mut iterator = input.into_iter().peekable();

        let mut output = Vec::new();

        loop {
            match iterator.next() {
                Some(PartiallyParsed::Lexed(RegexToken::Repetition { .. })) => {
                    return Err(
                        "Encountered repetition not succeeding repeatable token or group.".into(),
                    )
                }
                Some(part) => {
                    if let Some(PartiallyParsed::Lexed(RegexToken::Repetition { .. })) =
                        iterator.peek()
                    {
                        let (min, max) = match iterator.next() {
                            Some(PartiallyParsed::Lexed(RegexToken::Repetition { min, max })) => {
                                (min, max)
                            }
                            _ => panic!(),
                        };

                        output.push(PartiallyParsed::Repetition {
                            base: Box::new(part),
                            min,
                            max,
                        });
                    } else {
                        output.push(part);
                    }
                }
                None => break,
            }
        }

        Ok(output)
    }

    //TODO: make this be O(n)
    fn parse_alternation(mut input: Vec<PartiallyParsed>) -> Result<Vec<PartiallyParsed>, String> {
        loop {
            let mut did_make_change = false;

            for index in 0..input.len() {
                if let PartiallyParsed::Lexed(RegexToken::Alternation) = input[index] {
                    if index == 0 {
                        return Err("Found alternation token without preceding item.".into());
                    }

                    if index == input.len() - 1 {
                        return Err("Found alternation token without succeeding item.".into());
                    }

                    let preceeding = input[index - 1].clone();
                    let succeeding = input[index + 1].clone();

                    if let PartiallyParsed::Alternation(..) = preceeding {
                        if let PartiallyParsed::Alternation(inner) = &mut input[index - 1] {
                            inner.push(succeeding);
                        }
                        input.remove(index + 1);
                        input.remove(index);
                    } else {
                        input[index] = PartiallyParsed::Alternation(vec![preceeding, succeeding]);
                        input.remove(index + 1);
                        input.remove(index - 1);
                    }

                    did_make_change = true;
                    break;
                }
            }

            if !did_make_change {
                break;
            }
        }

        Ok(input)
    }
}

#[test]
fn test_grouping() {
    let grouped = RegexEntry::group(RegexEntry::lex("(.+(+.)){5}").unwrap());

    assert_eq!(
        grouped,
        vec![
            PartiallyParsed::Group(vec![
                PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                PartiallyParsed::Lexed(RegexToken::Repetition {
                    min: Some(1),
                    max: None
                }),
                PartiallyParsed::Group(vec![
                    PartiallyParsed::Lexed(RegexToken::Repetition {
                        min: Some(1),
                        max: None
                    }),
                    PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                ]),
            ]),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(5),
                max: Some(5)
            })
        ]
    )
}

#[test]
fn test_repetitions() {
    let grouped =
        RegexEntry::parse_repetitions(RegexEntry::group(RegexEntry::lex("(.+(+.)){5,6}").unwrap()))
            .unwrap();

    assert_eq!(
        grouped,
        vec![PartiallyParsed::Repetition {
            base: Box::new(PartiallyParsed::Group(vec![
                PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                PartiallyParsed::Lexed(RegexToken::Repetition {
                    min: Some(1),
                    max: None
                }),
                PartiallyParsed::Group(vec![
                    PartiallyParsed::Lexed(RegexToken::Repetition {
                        min: Some(1),
                        max: None
                    }),
                    PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                ]),
            ])),
            min: Some(5),
            max: Some(6)
        }]
    )
}

#[test]
fn test_alternations_basic() {
    let grouped = RegexEntry::parse_alternation(RegexEntry::lex(".|+").unwrap()).unwrap();

    assert_eq!(
        grouped,
        vec![PartiallyParsed::Alternation(vec![
            PartiallyParsed::Lexed(RegexToken::AnyCharacter),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(1),
                max: None
            })
        ])]
    );
}

#[test]
fn test_alternations_nested() {
    let grouped = RegexEntry::parse_alternation(RegexEntry::lex(".|+|*").unwrap()).unwrap();

    assert_eq!(
        grouped,
        vec![PartiallyParsed::Alternation(vec![
            PartiallyParsed::Lexed(RegexToken::AnyCharacter),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(1),
                max: None
            }),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(0),
                max: None
            })
        ])]
    );
}
