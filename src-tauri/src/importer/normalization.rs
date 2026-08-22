use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use super::headers::PRODUCT_INTEREST_HEADER;
use super::product_interest::{parse_product_answer, ProductAnswerMode, ProductCode};
use super::source::SourceRow;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NormalizationWarning {
    InvalidEmail,
    InvalidPhone,
    InvalidCountry,
    InvalidTimestamp,
    MissingContactMethod,
    UnknownProduct,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSubmission {
    pub row_number: usize,
    pub external_lead_id: String,
    pub created_at_utc: Option<String>,
    pub normalized_email: Option<String>,
    pub normalized_phone: Option<String>,
    pub country_code: Option<String>,
    pub product_interests: Vec<ProductCode>,
    pub warnings: Vec<NormalizationWarning>,
}

pub fn normalize_source_row(row: &SourceRow) -> NormalizedSubmission {
    let external_lead_id = row.get("id").unwrap_or_default().trim().to_string();
    let created_at_utc = normalize_timestamp(row.get("created_time").unwrap_or_default());

    let raw_email = row.get("email").unwrap_or_default();
    let normalized_email = normalize_email(raw_email);

    let raw_phone = row.get("phone_number").unwrap_or_default();
    let normalized_phone = normalize_phone(raw_phone);

    let raw_country = row.get("country").unwrap_or_default();
    let country_code = normalize_country(raw_country);

    let raw_product = row.get(PRODUCT_INTEREST_HEADER).unwrap_or_default();
    let parsed_product = parse_product_answer(raw_product);
    let mut product_interests = parsed_product.interests;
    let mut warnings = Vec::new();

    if !raw_email.trim().is_empty() && normalized_email.is_none() {
        warnings.push(NormalizationWarning::InvalidEmail);
    }
    if !raw_phone.trim().is_empty() && normalized_phone.is_none() {
        warnings.push(NormalizationWarning::InvalidPhone);
    }
    if !raw_country.trim().is_empty() && country_code.is_none() {
        warnings.push(NormalizationWarning::InvalidCountry);
    }
    if !row.get("created_time").unwrap_or_default().trim().is_empty() && created_at_utc.is_none() {
        warnings.push(NormalizationWarning::InvalidTimestamp);
    }
    if normalized_email.is_none() && normalized_phone.is_none() {
        warnings.push(NormalizationWarning::MissingContactMethod);
    }

    match parsed_product.mode {
        ProductAnswerMode::LegacyFreeText => {
            let legacy = normalize_legacy_product(raw_product);
            if legacy.is_empty() && !raw_product.trim().is_empty() {
                product_interests.push(ProductCode::Unknown);
                warnings.push(NormalizationWarning::UnknownProduct);
            } else {
                product_interests.extend(legacy);
            }
        }
        ProductAnswerMode::Structured if !parsed_product.unknown_tokens.is_empty() => {
            product_interests.push(ProductCode::Unknown);
            warnings.push(NormalizationWarning::UnknownProduct);
        }
        ProductAnswerMode::Empty | ProductAnswerMode::Structured => {}
    }

    deduplicate_products(&mut product_interests);
    deduplicate_warnings(&mut warnings);

    NormalizedSubmission {
        row_number: row.row_number,
        external_lead_id,
        created_at_utc,
        normalized_email,
        normalized_phone,
        country_code,
        product_interests,
        warnings,
    }
}

pub fn normalize_email(raw: &str) -> Option<String> {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }

    let mut parts = value.split('@');
    let local = parts.next()?;
    let domain = parts.next()?;
    if parts.next().is_some()
        || local.is_empty()
        || domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
    {
        return None;
    }

    Some(value)
}

pub fn normalize_phone(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let value = trimmed.strip_prefix("p:").unwrap_or(trimmed).trim();
    if value.is_empty() {
        return None;
    }

    let has_plus = value.starts_with('+');
    let digits: String = value.chars().filter(|character| character.is_ascii_digit()).collect();
    if !(7..=15).contains(&digits.len()) {
        return None;
    }

    Some(if has_plus {
        format!("+{digits}")
    } else {
        digits
    })
}

pub fn normalize_country(raw: &str) -> Option<String> {
    let value = raw.trim().to_ascii_uppercase();
    if value.len() == 2 && value.chars().all(|character| character.is_ascii_alphabetic()) {
        Some(value)
    } else {
        None
    }
}

pub fn normalize_timestamp(raw: &str) -> Option<String> {
    let parsed = DateTime::parse_from_rfc3339(raw.trim()).ok()?;
    Some(
        parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true),
    )
}

pub fn normalize_legacy_product(raw: &str) -> Vec<ProductCode> {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Vec::new();
    }

    let mut products = Vec::new();

    if value.contains("micromotor") || value.contains("micro motor") {
        products.push(ProductCode::FueMicromotorSystems);
    }
    if value.contains("long hair") {
        products.push(ProductCode::LongHairFueSolutions);
    }
    if value.contains("punch") {
        products.push(ProductCode::FuePunches);
    }
    if value.contains("implanter") || value.contains("forceps") {
        products.push(ProductCode::ImplantersForcepsSurgicalInstruments);
    }
    if value.contains("chair")
        || value.contains("furniture")
        || value.contains("stool")
        || value.contains("medical bed")
    {
        products.push(ProductCode::MedicalChairsClinicFurniture);
    }
    if matches!(value.as_str(), "all" | "all products" | "all product" | "general information") {
        products.push(ProductCode::OtherGeneralInformation);
    }

    deduplicate_products(&mut products);
    products
}

fn deduplicate_products(values: &mut Vec<ProductCode>) {
    let mut unique = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    *values = unique;
}

fn deduplicate_warnings(values: &mut Vec<NormalizationWarning>) {
    let mut unique = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    *values = unique;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        normalize_country, normalize_email, normalize_legacy_product, normalize_phone,
        normalize_source_row, normalize_timestamp, NormalizationWarning,
    };
    use crate::importer::headers::PRODUCT_INTEREST_HEADER;
    use crate::importer::product_interest::ProductCode;
    use crate::importer::source::SourceRow;

    #[test]
    fn email_is_trimmed_and_lowercased() {
        assert_eq!(
            normalize_email("  Demo.Person@Example.COM "),
            Some("demo.person@example.com".to_string())
        );
        assert_eq!(normalize_email("not-an-email"), None);
    }

    #[test]
    fn phone_removes_meta_prefix_and_formatting_without_inventing_country_code() {
        assert_eq!(
            normalize_phone(" p:+90 (555) 123 45 67 "),
            Some("+905551234567".to_string())
        );
        assert_eq!(normalize_phone("555 123 4567"), Some("5551234567".to_string()));
        assert_eq!(normalize_phone("123"), None);
    }

    #[test]
    fn timestamp_offsets_normalize_to_the_same_utc_instant() {
        assert_eq!(
            normalize_timestamp("2026-08-20T04:37:27-05:00"),
            normalize_timestamp("2026-08-20T12:37:27+03:00")
        );
    }

    #[test]
    fn country_accepts_two_letter_code_only() {
        assert_eq!(normalize_country(" tr "), Some("TR".to_string()));
        assert_eq!(normalize_country("Türkiye"), None);
    }

    #[test]
    fn legacy_long_hair_micromotor_maps_to_two_categories() {
        let products = normalize_legacy_product("Long hair micro motor");
        assert_eq!(
            products,
            vec![
                ProductCode::FueMicromotorSystems,
                ProductCode::LongHairFueSolutions
            ]
        );
    }

    #[test]
    fn ambiguous_legacy_product_is_preserved_as_unknown_with_warning() {
        let mut fields = BTreeMap::new();
        fields.insert("id".into(), "l:test".into());
        fields.insert("created_time".into(), "2026-08-21T12:00:00+03:00".into());
        fields.insert("email".into(), "demo@example.test".into());
        fields.insert("phone_number".into(), "p:+905551234567".into());
        fields.insert("country".into(), "TR".into());
        fields.insert(PRODUCT_INTEREST_HEADER.into(), "Information".into());

        let normalized = normalize_source_row(&SourceRow::new(2, fields));
        assert!(normalized.product_interests.contains(&ProductCode::Unknown));
        assert!(normalized.warnings.contains(&NormalizationWarning::UnknownProduct));
    }

    #[test]
    fn verified_structured_multi_select_survives_normalization() {
        let mut fields = BTreeMap::new();
        fields.insert("id".into(), "l:test".into());
        fields.insert("created_time".into(), "2026-08-21T12:00:00+03:00".into());
        fields.insert("email".into(), "demo@example.test".into());
        fields.insert("phone_number".into(), "p:+905551234567".into());
        fields.insert("country".into(), "TR".into());
        fields.insert(
            PRODUCT_INTEREST_HEADER.into(),
            "fue_micromotor_systems|fue_punches|long_hair_fue_solutions".into(),
        );

        let normalized = normalize_source_row(&SourceRow::new(2, fields));
        assert_eq!(normalized.product_interests.len(), 3);
        assert!(normalized.warnings.is_empty());
    }
}
