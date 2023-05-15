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
            let mut disjuncton = Vec::new();

            loop {
                if remaining.starts_with(']') {
                    break;
                } else if let Some((class, new_remaining)) = Self::try_parse_no_prefix(remaining)? {
                    disjuncton.push(class);
                    remaining = new_remaining;
                } else {
                    return Err(format!("Failed to parse remaining character class pattern from start of string: {}", remaining));
                }
            }

            if disjuncton.len() == 1 {
                Ok(Some((
                    disjuncton.into_iter().nth(0).unwrap(),
                    &remaining[1..],
                )))
            } else {
                Ok(Some((
                    CharacterClass::Disjunction(disjuncton),
                    &remaining[1..],
                )))
            }
        } else {
            Ok(None)
        }
    }

    fn try_parse_no_prefix(remaining: &str) -> Result<Option<(CharacterClass, &str)>, String> {
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
        let mut chars = Vec::new();
        let mut index = 0;
        loop {
            match remaining.chars().nth(index) {
                Some(']') => break,
                Some(other) => chars.push(other),
                None => return Err("Failed to parse simple character list character class because found end of pattern before ].".into()),
            }
            index += 1;
        }

        let class = if chars.len() == 1 {
            CharacterClass::Char(chars[0])
        } else {
            CharacterClass::Disjunction(
                chars
                    .into_iter()
                    .map(|ch| CharacterClass::Char(ch))
                    .collect(),
            )
        };

        Ok(Some((class, &remaining[index..])))
    }
}

#[cfg(test)]
fn test_parse(remaining: &str) -> Option<CharacterClass> {
    match CharacterClass::try_parse(remaining) {
        Ok(Some((parsed, remaining))) => {
            assert_eq!(remaining.len(), 0);
            Some(parsed)
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
