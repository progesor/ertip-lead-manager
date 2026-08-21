use std::collections::HashSet;

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductCode {
    FueMicromotorSystems,
    LongHairFueSolutions,
    FuePunches,
    ImplantersForcepsSurgicalInstruments,
    MedicalChairsClinicFurniture,
    OtherGeneralInformation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductAnswerMode {
    Empty,
    LegacyFreeText,
    Structured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProductAnswer {
    pub mode: ProductAnswerMode,
    pub interests: Vec<ProductCode>,
    pub unknown_tokens: Vec<String>,
}

pub fn parse_product_answer(raw: &str) -> ParsedProductAnswer {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return ParsedProductAnswer {
            mode: ProductAnswerMode::Empty,
            interests: Vec::new(),
            unknown_tokens: Vec::new(),
        };
    }

    if is_structured_answer(trimmed) {
        return parse_structured_answer(trimmed);
    }

    ParsedProductAnswer {
        mode: ProductAnswerMode::LegacyFreeText,
        interests: Vec::new(),
        unknown_tokens: Vec::new(),
    }
}

fn is_structured_answer(value: &str) -> bool {
    value.contains('|')
        || machine_value_to_code(value).is_some()
        || looks_like_machine_value(value)
}

fn looks_like_machine_value(value: &str) -> bool {
    value.contains('_')
        && !value.chars().any(char::is_whitespace)
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | ',' | '&' | '/')
        })
}

fn parse_structured_answer(value: &str) -> ParsedProductAnswer {
    let mut seen = HashSet::new();
    let mut interests = Vec::new();
    let mut unknown_tokens = Vec::new();

    for token in value.split('|').map(str::trim).filter(|token| !token.is_empty()) {
        match machine_value_to_code(token) {
            Some(code) if seen.insert(code) => interests.push(code),
            Some(_) => {}
            None => unknown_tokens.push(token.to_string()),
        }
    }

    ParsedProductAnswer {
        mode: ProductAnswerMode::Structured,
        interests,
        unknown_tokens,
    }
}

pub fn machine_value_to_code(value: &str) -> Option<ProductCode> {
    match value.trim() {
        "fue_micromotor_systems" => Some(ProductCode::FueMicromotorSystems),
        "long_hair_fue_solutions" => Some(ProductCode::LongHairFueSolutions),
        "fue_punches" => Some(ProductCode::FuePunches),
        "implanters,_forceps_&_surgical_instruments" => {
            Some(ProductCode::ImplantersForcepsSurgicalInstruments)
        }
        "medical_chairs_&_clinic_furniture" => Some(ProductCode::MedicalChairsClinicFurniture),
        "other_products_/_general_information" => Some(ProductCode::OtherGeneralInformation),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_product_answer, ProductAnswerMode, ProductCode};

    #[test]
    fn parses_structured_single_selection() {
        let parsed = parse_product_answer("fue_punches");

        assert_eq!(parsed.mode, ProductAnswerMode::Structured);
        assert_eq!(parsed.interests, vec![ProductCode::FuePunches]);
        assert!(parsed.unknown_tokens.is_empty());
    }

    #[test]
    fn parses_all_verified_machine_values_without_comma_splitting() {
        let parsed = parse_product_answer(
            "fue_micromotor_systems|other_products_/_general_information|medical_chairs_&_clinic_furniture|implanters,_forceps_&_surgical_instruments|fue_punches|long_hair_fue_solutions",
        );

        assert_eq!(parsed.mode, ProductAnswerMode::Structured);
        assert_eq!(parsed.interests.len(), 6);
        assert!(parsed
            .interests
            .contains(&ProductCode::ImplantersForcepsSurgicalInstruments));
        assert!(parsed.unknown_tokens.is_empty());
    }

    #[test]
    fn de_duplicates_repeated_structured_tokens() {
        let parsed = parse_product_answer("fue_punches|fue_punches");

        assert_eq!(parsed.interests, vec![ProductCode::FuePunches]);
    }

    #[test]
    fn unknown_machine_token_is_preserved_for_warning() {
        let parsed = parse_product_answer("fue_punches|future_product_group");

        assert_eq!(parsed.mode, ProductAnswerMode::Structured);
        assert_eq!(parsed.interests, vec![ProductCode::FuePunches]);
        assert_eq!(parsed.unknown_tokens, vec!["future_product_group"]);
    }

    #[test]
    fn legacy_free_text_is_not_misclassified_as_structured() {
        let parsed = parse_product_answer("Long hair micro motor");

        assert_eq!(parsed.mode, ProductAnswerMode::LegacyFreeText);
        assert!(parsed.interests.is_empty());
    }
}
