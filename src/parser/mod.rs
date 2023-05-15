pub mod character_class;
mod tokenizer;

use character_class::CharacterClass;
use unic_ucd_category::GeneralCategory;

#[derive(Debug, Eq, PartialEq)]
pub enum RegexEntry {
    AnyCharacter,
    UnicodeCharacterClass(Vec<GeneralCategory>),
    NegatedUnicodeCharacterClass(Vec<GeneralCategory>),
    NonUnicodeCharacterClass(CharacterClass),
    Concatenation {
        left: Box<RegexEntry>,
        right: Box<RegexEntry>,
    },
    Alternation {
        left: Box<RegexEntry>,
        right: Box<RegexEntry>,
    },
    Repetition {
        base: Box<RegexEntry>,
        min: Option<u64>,
        max: Option<u64>,
    },
}
