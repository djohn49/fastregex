use crate::parser::character_class::CharacterClass;
use unic_ucd_category::GeneralCategory;

#[derive(Debug, Eq, PartialEq)]
pub enum RegexToken {
    AnyCharacter,
    UnicodeCharacterClass(Vec<GeneralCategory>),
    NegatedUnicodeCharacterClass(Vec<GeneralCategory>),
    NonUnicodeCharacterClass(CharacterClass),
    Alternation,
    OpenGroup,
    CloseGroup,
    Repetition { min: Option<u64>, max: Option<u64> },
}

impl RegexToken {
    pub fn parse(regex: impl AsRef<str>) -> Result<Vec<RegexToken>, String> {
        //this function is a parser combinator: https://en.wikipedia.org/wiki/Parser_combinator
        let mut remaining_regex = regex.as_ref();

        let mut entries = Vec::new();
        while !remaining_regex.is_empty() {
            match Self::try_parse_one_entry(remaining_regex) {
                Ok(Some((entry, new_remaining_regex))) => {
                    entries.push(entry);
                    remaining_regex = new_remaining_regex;
                }
                Ok(None) => return Err(format!("Failed to parse regex remaining at because no tokens matched: {remaining_regex}")),
                Err(msg) => return Err(format!("Error occurred with remaining regex \"{remaining_regex}\": {msg}")),
            }
        }

        Ok(entries)
    }

    fn try_parse_one_entry(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        macro_rules! try_entry {
            ($fn_path: path) => {
                if let Some(tuple) = $fn_path(remaining)? {
                    return Ok(Some(tuple));
                }
            };
        }

        try_entry!(Self::try_parse_open_group);
        try_entry!(Self::try_parse_close_group);
        try_entry!(Self::try_parse_dot);
        try_entry!(Self::try_parse_zero_or_more);
        try_entry!(Self::try_parse_one_or_more);
        try_entry!(Self::try_parse_optional);
        try_entry!(Self::try_parse_alternation);
        try_entry!(Self::try_parse_digit);
        try_entry!(Self::try_parse_not_digit);
        //we must parse the multi letter case here first so that \p{ is not seen as a single-unicode class name with the invalid identifier '{'. We could simply move on on such failures, but it is more user-friendly to return a useful error in the case of unknown class names
        try_entry!(Self::try_parse_multi_letter_unicode_class_name);
        try_entry!(Self::try_parse_one_letter_unicode_class_name);
        //we must parse the multi letter case here first so that \p{ is not seen as a single-unicode class name with the invalid identifier '{'. We could simply move on on such failures, but it is more user-friendly to return a useful error in the case of unknown class names
        try_entry!(Self::try_parse_negated_multi_letter_unicode_class_name);
        try_entry!(Self::try_parse_negated_one_letter_unicode_class_name);
        try_entry!(Self::try_parse_character_class);
        try_entry!(Self::try_parse_repetition);

        Ok(None)
    }

    fn try_parse_open_group(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(remaining, "(", RegexToken::OpenGroup)
    }

    fn try_parse_close_group(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(remaining, ")", RegexToken::CloseGroup)
    }

    fn try_parse_dot(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(remaining, ".", RegexToken::AnyCharacter)
    }

    fn try_parse_zero_or_more(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(
            remaining,
            "*",
            RegexToken::Repetition {
                min: Some(0),
                max: None,
            },
        )
    }

    fn try_parse_one_or_more(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(
            remaining,
            "+",
            RegexToken::Repetition {
                min: Some(1),
                max: None,
            },
        )
    }

    fn try_parse_optional(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(
            remaining,
            "?",
            RegexToken::Repetition {
                min: Some(0),
                max: Some(1),
            },
        )
    }

    fn try_parse_alternation(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        Self::try_parse_static_prefix_character(remaining, "|", RegexToken::Alternation)
    }

    fn try_parse_static_prefix_character<'remaining, 'prefix>(
        remaining: &'remaining str,
        prefix: &'prefix str,
        to_return: RegexToken,
    ) -> Result<Option<(RegexToken, &'remaining str)>, String> {
        if remaining.starts_with(prefix) {
            Ok(Some((to_return, &remaining[1..])))
        } else {
            Ok(None)
        }
    }

    fn try_parse_digit(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        if remaining.starts_with("\\d") {
            Ok(Some((
                RegexToken::UnicodeCharacterClass(vec![
                    GeneralCategory::DecimalNumber,
                    GeneralCategory::OtherNumber,
                    GeneralCategory::LetterNumber,
                ]),
                &remaining[2..],
            )))
        } else {
            Ok(None)
        }
    }

    fn try_parse_not_digit(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        if remaining.starts_with("\\D") {
            Ok(Some((
                RegexToken::NegatedUnicodeCharacterClass(vec![
                    GeneralCategory::DecimalNumber,
                    GeneralCategory::OtherNumber,
                    GeneralCategory::LetterNumber,
                ]),
                &remaining[2..],
            )))
        } else {
            Ok(None)
        }
    }

    fn try_parse_one_letter_unicode_class_name(
        remaining: &str,
    ) -> Result<Option<(RegexToken, &str)>, String> {
        if remaining.starts_with("\\p") && remaining.len() >= 3 {
            let class_name_identifier = remaining.chars().nth(2).unwrap(); //unwrap will not panic since we checked length
            let classes = Self::get_unicode_classes_single_letter(class_name_identifier)?;
            Ok(Some((
                RegexToken::UnicodeCharacterClass(classes),
                &remaining[3..],
            )))
        } else {
            Ok(None)
        }
    }

    fn try_parse_multi_letter_unicode_class_name(
        remaining: &str,
    ) -> Result<Option<(RegexToken, &str)>, String> {
        if remaining.starts_with("\\p{") && remaining.len() >= 3 {
            let class_name_identifier = Self::parse_string_until_bracket(&remaining[3..]);
            let classes = Self::get_unicode_classes_multi_or_single_letter(&class_name_identifier)?;
            Ok(Some((
                RegexToken::UnicodeCharacterClass(classes),
                &remaining[(4 + class_name_identifier.len())..],
            )))
        } else {
            Ok(None)
        }
    }

    fn try_parse_negated_one_letter_unicode_class_name(
        remaining: &str,
    ) -> Result<Option<(RegexToken, &str)>, String> {
        if remaining.starts_with("\\P") && remaining.len() >= 3 {
            let class_name_identifier = remaining.chars().nth(2).unwrap(); //unwrap will not panic since we checked length
            let classes = Self::get_unicode_classes_single_letter(class_name_identifier)?;
            Ok(Some((
                RegexToken::NegatedUnicodeCharacterClass(classes),
                &remaining[3..],
            )))
        } else {
            Ok(None)
        }
    }

    fn try_parse_negated_multi_letter_unicode_class_name(
        remaining: &str,
    ) -> Result<Option<(RegexToken, &str)>, String> {
        if remaining.starts_with("\\P{") && remaining.len() >= 3 {
            let class_name_identifier = Self::parse_string_until_bracket(&remaining[3..]);
            let classes = Self::get_unicode_classes_multi_or_single_letter(&class_name_identifier)?;
            Ok(Some((
                RegexToken::NegatedUnicodeCharacterClass(classes),
                &remaining[(4 + class_name_identifier.len())..],
            )))
        } else {
            Ok(None)
        }
    }

    fn try_parse_character_class(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        match CharacterClass::try_parse(remaining)? {
            Some((class, remaining)) => Ok(Some((
                RegexToken::NonUnicodeCharacterClass(class),
                remaining,
            ))),
            None => Ok(None),
        }
    }

    fn try_parse_repetition(remaining: &str) -> Result<Option<(RegexToken, &str)>, String> {
        if !remaining.starts_with('{') {
            return Ok(None);
        }

        let mut inner_string = String::new();

        let mut chars = remaining.chars();
        chars.next();
        loop {
            match chars.next() {
                Some('}') => {
                    break;
                }
                Some(ch) => inner_string.push(ch),
                None => return Err("Started repetition token but did not finish".into()),
            }
        }

        let split = inner_string.split(",").collect::<Vec<_>>();
        let (min, max) = match split.len() {
            1 => {
                let number = split[0].parse().map_err(|err| {
                    format!(
                        "Failed to parse a number inside a repetition token: {} {:?}",
                        split[0], err
                    )
                })?;
                (Some(number), Some(number))
            }
            2 => {
                fn parse_number(number: &str) -> Result<Option<u64>, String> {
                    if number.is_empty() {
                        Ok(None)
                    } else {
                        let number = number.parse().map_err(|err| {
                            format!(
                                "Failed to parse a number inside a repetition token: {} {:?}",
                                number, err
                            )
                        })?;
                        Ok(Some(number))
                    }
                }

                let min = parse_number(split[0])?;
                let max = parse_number(split[1])?;

                (min, max)
            }
            _ => return Err("A repetition token must have exactly zero or one commas.".into()),
        };

        Ok(Some((
            RegexToken::Repetition { min, max },
            &remaining[(2 + inner_string.len())..],
        )))
    }

    fn parse_string_until_bracket(remaining: &str) -> String {
        let mut class_name = String::new();

        for char in remaining.chars() {
            match char {
                '}' => break,
                char => class_name.push(char),
            }
        }

        class_name
    }

    /// This function gets the set of unicode classes that refer to a named set of
    /// unicode classes as per the unicode standard.
    ///
    /// https://unicode.org/reports/tr44/#General_Category_Values
    fn get_unicode_classes_multi_or_single_letter(
        class_identifier: &str,
    ) -> Result<Vec<GeneralCategory>, String> {
        use GeneralCategory::*;

        if class_identifier.len() == 1 {
            if let Ok(category) =
                Self::get_unicode_classes_single_letter(class_identifier.chars().nth(0).unwrap())
            {
                //unwrap will not panic since we checked length
                return Ok(category);
            }
        }

        let class_identifier = match class_identifier {
            "Lu" | "Uppercase_Letter" => UppercaseLetter,
            "Ll" | "Lowercase_Letter" => LowercaseLetter,
            "Lt" | "Titlecase_Letter" => TitlecaseLetter,
            "Lm" | "Modifier_Letter" => ModifierLetter,
            "Lo" | "Other_Letter" => OtherLetter,
            "Mn" | "Nonspacing_Mark" => NonspacingMark,
            "Mc" | "Spacing_Mark" => SpacingMark,
            "Me" | "Enclosing_Mark" => EnclosingMark,
            "Nd" | "Decimal_Number" => DecimalNumber,
            "Nl" | "Letter_Number" => LetterNumber,
            "No" | "Other_Number" => OtherNumber,
            "Pc" | "Connector_Punctuation" => ConnectorPunctuation,
            "Pd" | "Dash_Punctuation" => DashPunctuation,
            "Ps" | "Open_Punctuation" => OpenPunctuation,
            "Pe" | "Close_Punctuation" => ClosePunctuation,
            "Pi" | "Initial_Punctuation" => InitialPunctuation,
            "Pf" | "Final_Punctuation" => FinalPunctuation,
            "Po" | "Other_Punctuation" => OtherPunctuation,
            "Sm" | "Math_Symbol" => MathSymbol,
            "Sc" | "Currency_Symbol" => CurrencySymbol,
            "Sk" | "Modifier_Symbol" => ModifierSymbol,
            "So" | "Other_Symbol" => OtherSymbol,
            "Zs" | "Space_Separator" => SpaceSeparator,
            "Zl" | "Line_Separator" => LineSeparator,
            "Zp" | "Paragraph_Separator" => ParagraphSeparator,
            "Cc" | "Control" => Control,
            "Cf" | "Format" => Format,
            "Cs" | "Surrogate" => Surrogate,
            "Co" | "Private_Use" => PrivateUse,
            "Cn" | "Unassigned" => Unassigned,
            unknown_class_identifier => {
                return Err(format!(
                    r#"{} is not a known single-character Unicode class name identifier. Expected one of "Lu", "Uppercase_Letter", "Ll", "Lowercase_Letter", "Lt", "Titlecase_Letter", "Lm", "Modifier_Letter", "Lo", "Other_Letter", "Mn", "Nonspacing_Mark", "Mc", "Spacing_Mark", "Me", "Enclosing_Mark", "Nd", "Decimal_Number", "Nl", "Letter_Number", "No", "Other_Number", "Pc", "Connector_Punctuation", "Pd", "Dash_Punctuation", "Ps", "Open_Punctuation", "Pe", "Close_Punctuation", "Pi", "Initial_Punctuation", "Pf", "Final_Punctuation", "Po", "Other_Punctuation", "Sm", "Math_Symbol", "Sc", "Currency_Symbol", "Sk", "Modifier_Symbol", "So", "Other_Symbol", "Zs", "Space_Separator", "Zl", "Line_Separator", "Zp", "Paragraph_Separator", "Cc", "Control", "Cf", "Format", "Cs", "Surrogate", "Co", "Private_Use", "Cn", "Unassigned", "L", "M", "N", "P", "S", "Z", "C"."#,
                    unknown_class_identifier
                ));
            }
        };

        Ok(vec![class_identifier])
    }

    /// This function gets the set of unicode classes that refer to a single-letter-named set of
    /// unicode classes as per the unicode standard.
    ///
    /// https://unicode.org/reports/tr44/#General_Category_Values
    fn get_unicode_classes_single_letter(
        class_identifier: char,
    ) -> Result<Vec<GeneralCategory>, String> {
        use GeneralCategory::*;
        match class_identifier {
            'L' => Ok(vec![UppercaseLetter, LowercaseLetter, TitlecaseLetter, ModifierLetter, OtherLetter]),
            'M' => Ok(vec![NonspacingMark, SpacingMark, EnclosingMark]),
            'N' => Ok(vec![DecimalNumber, LetterNumber, OtherNumber]),
            'P' => Ok(vec![ConnectorPunctuation, DashPunctuation, OpenPunctuation, ClosePunctuation, InitialPunctuation, FinalPunctuation, OpenPunctuation]),
            'S' => Ok(vec![MathSymbol, CurrencySymbol, ModifierSymbol, OtherSymbol]),
            'Z' => Ok(vec![SpaceSeparator, LineSeparator, ParagraphSeparator]),
            'C' => Ok(vec![Control, Format, Surrogate, PrivateUse, Unassigned]),
            //we try to parse multi-letter names first so that \p{ is not seen as a single-unicode class name with the invalid identifier '{'. We could simply move on on such failures, but it is more user-friendly to return a useful error in the case of unknown class names
            unknown_identifier => Err(format!("{unknown_identifier} is not a known single-character Unicode class name identifier. Expected one of L, M, N, P, S, Z, or C."))
        }
    }
}

#[cfg(test)]
mod test {
    use super::RegexToken;
    use crate::parser::character_class::CharacterClass;

    fn assert_equal(regex: &str, expected: Vec<RegexToken>) {
        let parsed = match RegexToken::parse(regex) {
            Ok(parsed) => parsed,
            Err(msg) => {
                eprintln!("Parsing failed for regex \"{regex}\" with error: {msg}");
                panic!();
            }
        };

        if parsed != expected {
            eprintln!("Parsed {:#?} but expected {:#?}", parsed, expected);
            panic!();
        }
    }

    #[test]
    fn test_tokenize() {
        use unic_ucd_category::GeneralCategory::*;

        assert_equal(
            r#"(.+*?|)\d\D\pL\pM\pC\p{Lu}\p{Math_Symbol}\PL\PM\PC\P{Lu}\P{Math_Symbol}[a][xyz][^a][^xyz][a-z][^a-z]{55}{50,}{,51}{52,53}"#,
            vec![
                RegexToken::OpenGroup,
                RegexToken::AnyCharacter,
                RegexToken::Repetition {
                    min: Some(1),
                    max: None,
                },
                RegexToken::Repetition {
                    min: Some(0),
                    max: None,
                },
                RegexToken::Repetition {
                    min: Some(0),
                    max: Some(1),
                },
                RegexToken::Alternation,
                RegexToken::CloseGroup,
                RegexToken::UnicodeCharacterClass(vec![DecimalNumber, OtherNumber, LetterNumber]),
                RegexToken::NegatedUnicodeCharacterClass(vec![
                    DecimalNumber,
                    OtherNumber,
                    LetterNumber,
                ]),
                RegexToken::UnicodeCharacterClass(vec![
                    UppercaseLetter,
                    LowercaseLetter,
                    TitlecaseLetter,
                    ModifierLetter,
                    OtherLetter,
                ]),
                RegexToken::UnicodeCharacterClass(vec![NonspacingMark, SpacingMark, EnclosingMark]),
                RegexToken::UnicodeCharacterClass(vec![
                    Control, Format, Surrogate, PrivateUse, Unassigned,
                ]),
                RegexToken::UnicodeCharacterClass(vec![UppercaseLetter]),
                RegexToken::UnicodeCharacterClass(vec![MathSymbol]),
                RegexToken::NegatedUnicodeCharacterClass(vec![
                    UppercaseLetter,
                    LowercaseLetter,
                    TitlecaseLetter,
                    ModifierLetter,
                    OtherLetter,
                ]),
                RegexToken::NegatedUnicodeCharacterClass(vec![
                    NonspacingMark,
                    SpacingMark,
                    EnclosingMark,
                ]),
                RegexToken::NegatedUnicodeCharacterClass(vec![
                    Control, Format, Surrogate, PrivateUse, Unassigned,
                ]),
                RegexToken::NegatedUnicodeCharacterClass(vec![UppercaseLetter]),
                RegexToken::NegatedUnicodeCharacterClass(vec![MathSymbol]),
                RegexToken::NonUnicodeCharacterClass(CharacterClass::Char('a')),
                RegexToken::NonUnicodeCharacterClass(CharacterClass::Disjunction(vec![
                    CharacterClass::Char('x'),
                    CharacterClass::Char('y'),
                    CharacterClass::Char('z'),
                ])),
                RegexToken::NonUnicodeCharacterClass(CharacterClass::Negated(Box::new(
                    CharacterClass::Char('a'),
                ))),
                RegexToken::NonUnicodeCharacterClass(CharacterClass::Negated(Box::new(
                    CharacterClass::Disjunction(vec![
                        CharacterClass::Char('x'),
                        CharacterClass::Char('y'),
                        CharacterClass::Char('z'),
                    ]),
                ))),
                RegexToken::NonUnicodeCharacterClass(CharacterClass::Range {
                    start: 'a',
                    end: 'z',
                }),
                RegexToken::NonUnicodeCharacterClass(CharacterClass::Negated(Box::new(
                    CharacterClass::Range {
                        start: 'a',
                        end: 'z',
                    },
                ))),
                RegexToken::Repetition {
                    min: Some(55),
                    max: Some(55),
                },
                RegexToken::Repetition {
                    min: Some(50),
                    max: None,
                },
                RegexToken::Repetition {
                    min: None,
                    max: Some(51),
                },
                RegexToken::Repetition {
                    min: Some(52),
                    max: Some(53),
                },
            ],
        );
    }
}
