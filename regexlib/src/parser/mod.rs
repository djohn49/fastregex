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
    Literal(char),
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
        let repetitions =
            Self::parse_for_all_groups_recursively(grouped, &Self::parse_repetitions)?;
        let alternations =
            Self::parse_for_all_groups_recursively(repetitions, &Self::parse_alternation)?;
        let mut parsed = Self::finish_parsing(alternations);
        Self::simplify_ast(&mut parsed);

        Ok(parsed)
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

    fn finish_parsing(input: Vec<PartiallyParsed>) -> RegexEntry {
        let concatenation = input
            .into_iter()
            .map(|partially_parsed| Self::lower_single_partially_parsed(partially_parsed))
            .collect::<Vec<_>>();
        RegexEntry::Concatenation(concatenation)
    }

    fn lower_single_partially_parsed(partially_parsed: PartiallyParsed) -> RegexEntry {
        match partially_parsed {
            PartiallyParsed::Lexed(RegexToken::AnyCharacter) => RegexEntry::AnyCharacter,
            PartiallyParsed::Lexed(RegexToken::NonUnicodeCharacterClass(class)) => RegexEntry::NonUnicodeCharacterClass(class),
            PartiallyParsed::Lexed(RegexToken::NegatedUnicodeCharacterClass(categories)) => RegexEntry::NegatedUnicodeCharacterClass(categories),
            PartiallyParsed::Lexed(RegexToken::UnicodeCharacterClass(categories)) => RegexEntry::UnicodeCharacterClass(categories),
            PartiallyParsed::Lexed(RegexToken::Literal(literal)) => RegexEntry::Literal(literal),
            PartiallyParsed::Lexed(token) => panic!("Encountered unexpected lexed but not parsed token when lowering intermediate parsing representation. This is an internal error in the parsed. {:#?}", token),
            PartiallyParsed::Group(concatenation) => RegexEntry::Concatenation(concatenation.into_iter().map(|entry| Self::lower_single_partially_parsed(entry)).collect()),
            PartiallyParsed::Repetition { base, min, max } => RegexEntry::Repetition { base: Box::new(Self::lower_single_partially_parsed(*base)), min, max },
            PartiallyParsed::Alternation(entries) => RegexEntry::Alternation(entries.into_iter().map(|entry| Self::lower_single_partially_parsed(entry)).collect()),
        }
    }

    fn simplify_ast(input: &mut RegexEntry) {
        match input {
            RegexEntry::Alternation(members_ref) | RegexEntry::Concatenation(members_ref) => {
                if members_ref.len() == 1 {
                    let mut members = Vec::new();
                    std::mem::swap(&mut members, members_ref);
                    *input = members.into_iter().next().unwrap();
                    Self::simplify_ast(input);
                } else {
                    for child in members_ref {
                        Self::simplify_ast(child);
                    }
                }
            }
            RegexEntry::Repetition { base, .. } => Self::simplify_ast(base),
            _ => {}
        }
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

#[cfg(test)]
fn test_full_parse(to_parse: &str, expected: RegexEntry) {
    let parsed = RegexEntry::parse(to_parse).unwrap();
    println!("{:#?}", parsed);
    assert_eq!(parsed, expected);
}

#[test]
fn test_simple_parse() {
    test_full_parse(
        ".+",
        RegexEntry::Repetition {
            min: Some(1),
            max: None,
            base: Box::new(RegexEntry::AnyCharacter),
        },
    );
}

#[test]
fn test_complex_parse_1() {
    use unic_ucd_category::GeneralCategory::*;
    use RegexEntry::*;
    test_full_parse(
        r#"((\d\PL)*){1,3}"#,
        Repetition {
            base: Box::new(Repetition {
                base: Box::new(Concatenation(vec![
                    UnicodeCharacterClass(vec![DecimalNumber, OtherNumber, LetterNumber]),
                    NegatedUnicodeCharacterClass(vec![
                        UppercaseLetter,
                        LowercaseLetter,
                        TitlecaseLetter,
                        ModifierLetter,
                        OtherLetter,
                    ]),
                ])),
                min: Some(0),
                max: None,
            }),
            min: Some(1),
            max: Some(3),
        },
    );
}

#[test]
fn test_complex_parse_2() {
    use crate::parser::character_class::CharacterClass::Range;
    use unic_ucd_category::GeneralCategory::*;
    use RegexEntry::*;
    test_full_parse(
        r#"([A-Z]+[0-9]*)|(\d+)"#,
        Alternation(vec![
            Concatenation(vec![
                Repetition {
                    base: Box::new(NonUnicodeCharacterClass(Range {
                        start: 'A',
                        end: 'Z',
                    })),
                    min: Some(1),
                    max: None,
                },
                Repetition {
                    base: Box::new(NonUnicodeCharacterClass(Range {
                        start: '0',
                        end: '9',
                    })),
                    min: Some(0),
                    max: None,
                },
            ]),
            Repetition {
                base: Box::new(UnicodeCharacterClass(vec![
                    DecimalNumber,
                    OtherNumber,
                    LetterNumber,
                ])),
                min: Some(1),
                max: None,
            },
        ]),
    );
}

#[test]
fn test_url() {
    use crate::parser::character_class::CharacterClass::*;
    use unic_ucd_category::GeneralCategory::*;
    use RegexEntry::*;
    test_full_parse(
        r#"https?://([A-Za-z.]+/)*([A-Za-z.]+)?"#,
        Concatenation(vec![
            Literal('h'),
            Literal('t'),
            Literal('t'),
            Literal('p'),
            Repetition {
                base: Box::new(Literal('s')),
                min: Some(0),
                max: Some(1),
            },
            Literal(':'),
            Literal('/'),
            Literal('/'),
            Repetition {
                base: Box::new(Concatenation(vec![
                    Repetition {
                        base: Box::new(NonUnicodeCharacterClass(Disjunction(vec![
                            Range {
                                start: 'A',
                                end: 'Z',
                            },
                            Range {
                                start: 'a',
                                end: 'z',
                            },
                            Char('.'),
                        ]))),
                        min: Some(1),
                        max: None,
                    },
                    Literal('/'),
                ])),
                min: Some(0),
                max: None,
            },
            Repetition {
                base: Box::new(Repetition {
                    base: Box::new(NonUnicodeCharacterClass(Disjunction(vec![
                        Range {
                            start: 'A',
                            end: 'Z',
                        },
                        Range {
                            start: 'a',
                            end: 'z',
                        },
                        Char('.'),
                    ]))),
                    min: Some(1),
                    max: None,
                }),
                min: Some(0),
                max: Some(1),
            },
        ]),
    );
}
