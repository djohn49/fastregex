use unic_ucd_category::GeneralCategory;

use character_class::CharacterClass;

use self::tokenizer::RegexToken;

pub mod character_class;
mod tokenizer;

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
}

impl RegexEntry {
    pub fn parse(regex: &str) -> Result<Self, String> {
        let lexed = Self::lex(regex)?;
        let grouped = Self::group(lexed);
        let repetitions =
            Self::parse_for_all_groups_recursively(grouped, &Self::parse_repetitions)?;
        let alternations =
            Self::parse_for_all_groups_recursively(repetitions, &Self::parse_alternation)?;

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

    fn parse_for_all_groups_recursively(
        input: Vec<PartiallyParsed>,
        parser: &impl Fn(Vec<PartiallyParsed>) -> Result<Vec<PartiallyParsed>, String>,
    ) -> Result<Vec<PartiallyParsed>, String> {
        let mut input = (*parser)(input)?;

        for child in &mut input {
            Self::parse_child_recursively(child, parser)?;
        }

        Ok(input)
    }

    fn parse_child_recursively(
        child: &mut PartiallyParsed,
        parser: &impl Fn(Vec<PartiallyParsed>) -> Result<Vec<PartiallyParsed>, String>,
    ) -> Result<(), String> {
        match child {
            PartiallyParsed::Group(child) | PartiallyParsed::Alternation(child) => {
                let mut child_stack = Vec::new();
                std::mem::swap(&mut child_stack, child);
                child_stack = Self::parse_for_all_groups_recursively(child_stack, parser)?;
                std::mem::swap(&mut child_stack, child);
            }
            PartiallyParsed::Repetition { base, .. } => {
                Self::parse_child_recursively(base.as_mut(), parser)?;
            }
            PartiallyParsed::Lexed(_) => {}
        }
        Ok(())
    }

    fn parse_repetitions(mut input: Vec<PartiallyParsed>) -> Result<Vec<PartiallyParsed>, String> {
        let mut iterator = input.into_iter().peekable();

        let mut output = Vec::new();

        loop {
            match iterator.next() {
                Some(PartiallyParsed::Lexed(RegexToken::Repetition { min, max })) => {
                    return Err(format!(
                        "Encountered repetition from {:?} to {:?} not succeeding repeatable token or group.", min, max
                    ));
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
                    max: None,
                }),
                PartiallyParsed::Group(vec![
                    PartiallyParsed::Lexed(RegexToken::Repetition {
                        min: Some(1),
                        max: None,
                    }),
                    PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                ]),
            ]),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(5),
                max: Some(5),
            }),
        ]
    )
}

#[cfg(test)]
fn test_util_test_incremental_parser(
    to_lex: &str,
    incremental_parser: impl Fn(Vec<PartiallyParsed>) -> Result<Vec<PartiallyParsed>, String>,
    expected: &Vec<PartiallyParsed>,
) {
    let lexed = RegexEntry::lex(to_lex).expect("Lexing failed");
    let grouped = RegexEntry::group(lexed);
    let parsed = RegexEntry::parse_for_all_groups_recursively(grouped, &incremental_parser)
        .expect("Recursive parsing failed");
    assert_eq!(&parsed, expected);
}

#[test]
fn test_repetitions() {
    test_util_test_incremental_parser(
        "(.(.)){5,6}",
        RegexEntry::parse_repetitions,
        &vec![PartiallyParsed::Repetition {
            base: Box::new(PartiallyParsed::Group(vec![
                PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                PartiallyParsed::Group(vec![PartiallyParsed::Lexed(RegexToken::AnyCharacter)]),
            ])),
            min: Some(5),
            max: Some(6),
        }],
    );
}

#[test]
fn test_nested_repetitions() {
    test_util_test_incremental_parser(
        "(.(.){7,8}[A]{4,}){5,6}",
        RegexEntry::parse_repetitions,
        &vec![PartiallyParsed::Repetition {
            base: Box::new(PartiallyParsed::Group(vec![
                PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                PartiallyParsed::Repetition {
                    base: Box::new(PartiallyParsed::Group(vec![PartiallyParsed::Lexed(
                        RegexToken::AnyCharacter,
                    )])),
                    min: Some(7),
                    max: Some(8),
                },
                PartiallyParsed::Repetition {
                    base: Box::new(PartiallyParsed::Lexed(
                        RegexToken::NonUnicodeCharacterClass(CharacterClass::Char('A')),
                    )),
                    min: Some(4),
                    max: None,
                },
            ])),
            min: Some(5),
            max: Some(6),
        }],
    );
}

#[test]
fn test_alternations_basic() {
    test_util_test_incremental_parser(
        ".|+",
        RegexEntry::parse_alternation,
        &vec![PartiallyParsed::Alternation(vec![
            PartiallyParsed::Lexed(RegexToken::AnyCharacter),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(1),
                max: None,
            }),
        ])],
    );
}

#[test]
fn test_alternations_nested() {
    test_util_test_incremental_parser(
        ".|+|*",
        RegexEntry::parse_alternation,
        &vec![PartiallyParsed::Alternation(vec![
            PartiallyParsed::Lexed(RegexToken::AnyCharacter),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(1),
                max: None,
            }),
            PartiallyParsed::Lexed(RegexToken::Repetition {
                min: Some(0),
                max: None,
            }),
        ])],
    );
}

#[test]
fn test_alternations_nested_recursive() {
    test_util_test_incremental_parser(
        "(.|+|*)|(.)",
        RegexEntry::parse_alternation,
        &vec![PartiallyParsed::Alternation(vec![
            PartiallyParsed::Group(vec![PartiallyParsed::Alternation(vec![
                PartiallyParsed::Lexed(RegexToken::AnyCharacter),
                PartiallyParsed::Lexed(RegexToken::Repetition {
                    min: Some(1),
                    max: None,
                }),
                PartiallyParsed::Lexed(RegexToken::Repetition {
                    min: Some(0),
                    max: None,
                }),
            ])]),
            PartiallyParsed::Group(vec![PartiallyParsed::Lexed(RegexToken::AnyCharacter)]),
        ])],
    );
}
