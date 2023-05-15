#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CharacterClass {
    Char(char),
    Range { start: char, end: char },
    Disjunction(Vec<CharacterClass>),
    Negated(Box<CharacterClass>),
}

impl CharacterClass {
    pub fn try_parse(mut remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
        if remaining.chars().nth(0) == Some('[') {
            remaining = &remaining[1..];
            match Self::try_parse_no_prefix(remaining)? {
                Some((parsed, remaining)) => {
                    if !remaining.starts_with(']') {
                        return Err(format!(
                            "Expected ] after character class, found {}",
                            remaining
                        ));
                    }
                    Ok(Some((parsed, &remaining[1..])))
                }
                one => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn try_parse_no_prefix(mut remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
        let mut disjuncton = Vec::new();

        loop {
            if remaining.starts_with(']') {
                break;
            } else if let Some((class, new_remaining)) = Self::try_parse_single_class(remaining)? {
                disjuncton.push(class);
                remaining = new_remaining;
            } else {
                return Err(format!(
                    "Failed to parse remaining character class pattern from start of string: {}",
                    remaining
                ));
            }
        }

        if disjuncton.len() == 1 {
            Ok(Some((disjuncton.into_iter().nth(0).unwrap(), remaining)))
        } else {
            Ok(Some((CharacterClass::Disjunction(disjuncton), remaining)))
        }
    }

    fn try_parse_single_class(remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
        if let Some(tuple) = Self::try_parse_negated(remaining)? {
            return Ok(Some(tuple));
        }

        if let Some(tuple) = Self::try_parse_range(remaining)? {
            return Ok(Some(tuple));
        }

        if let Some(tuple) = Self::try_parse_simple_char(remaining)? {
            return Ok(Some(tuple));
        }

        Ok(None)
    }

    fn try_parse_negated(remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
        if remaining.chars().nth(0) == Some('^') {
            match Self::try_parse_no_prefix(&remaining[1..])? {
                Some((to_negate, remaining)) => Ok(Some((
                    CharacterClass::Negated(Box::new(to_negate)),
                    remaining,
                ))),
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn try_parse_range(remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
        if remaining.len() < 3 {
            return Ok(None);
        }

        if remaining.chars().nth(1) != Some('-') {
            return Ok(None);
        }

        //won't panic since we checked if the length is < 3
        let start = remaining.chars().nth(0).unwrap();
        let end = remaining.chars().nth(2).unwrap();

        Ok(Some((
            CharacterClass::Range { start, end },
            &remaining[3..],
        )))
    }

    fn try_parse_simple_char(remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
        if remaining.len() < 1 {
            return Ok(None);
        }

        Ok(Some((
            CharacterClass::Char(remaining.chars().nth(0).unwrap()),
            &remaining[1..],
        )))
    }
}

#[cfg(test)]
fn test_parse(remaining: &str) -> Option<CharacterClass> {
    match CharacterClass::try_parse(remaining) {
        Ok(Some((parsed, remaining))) => {
            assert_eq!(remaining.len(), 0);
            Some(parsed)
        }
        Err(e) => {
            eprintln!("{}", e);
            None
        }
        _ => None,
    }
}

#[test]
fn test_single_char() {
    assert_eq!(test_parse("[a]").unwrap(), CharacterClass::Char('a'));
}

#[test]
fn test_multi_char() {
    assert_eq!(
        test_parse("[xyz]").unwrap(),
        CharacterClass::Disjunction(vec![
            CharacterClass::Char('x'),
            CharacterClass::Char('y'),
            CharacterClass::Char('z'),
        ])
    );
}

#[test]
fn test_negated_single_char() {
    assert_eq!(
        test_parse("[^a]").unwrap(),
        CharacterClass::Negated(Box::new(CharacterClass::Char('a')))
    );
}

#[test]
fn test_negated_multi_char() {
    assert_eq!(
        test_parse("[^xyz]").unwrap(),
        CharacterClass::Negated(Box::new(CharacterClass::Disjunction(vec![
            CharacterClass::Char('x'),
            CharacterClass::Char('y'),
            CharacterClass::Char('z'),
        ])))
    );
}

#[test]
fn test_range() {
    assert_eq!(
        test_parse("[a-z]").unwrap(),
        CharacterClass::Range {
            start: 'a',
            end: 'z',
        }
    );
}

#[test]
fn test_double_range() {
    assert_eq!(
        test_parse("[a-z0-9]").unwrap(),
        CharacterClass::Disjunction(vec![
            CharacterClass::Range {
                start: 'a',
                end: 'z',
            },
            CharacterClass::Range {
                start: '0',
                end: '9',
            },
        ])
    );
}

#[test]
fn test_negated_range() {
    assert_eq!(
        test_parse("[^a-z]").unwrap(),
        CharacterClass::Negated(Box::new(CharacterClass::Range {
            start: 'a',
            end: 'z',
        }))
    );
}
