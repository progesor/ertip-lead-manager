use std::collections::BTreeMap;
use std::path::Path;

use csv::ReaderBuilder;

use crate::error::AppError;

use super::headers::has_required_headers;
use super::source::{SourceFormat, SourceRow, SourceTable};

pub fn parse_csv(path: &Path) -> Result<SourceTable, AppError> {
    let bytes = std::fs::read(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| AppError::CsvEncoding)?;
    let text = text.trim_start_matches('\u{feff}');

    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());

    let headers = reader
        .headers()?
        .iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if !has_required_headers(&headers) {
        return Err(AppError::ImportSchema(
            "required lead headers were not found in CSV".to_string(),
        ));
    }

    let mut rows = Vec::new();

    for (index, record) in reader.records().enumerate() {
        let record = record?;

        if record.iter().all(|value| value.trim().is_empty()) {
            continue;
        }

        let fields = headers
            .iter()
            .enumerate()
            .map(|(column, header)| {
                (
                    header.clone(),
                    record.get(column).unwrap_or_default().to_string(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        rows.push(SourceRow::new(index + 2, fields));
    }

    Ok(SourceTable::new(
        SourceFormat::Csv,
        file_name(path),
        None,
        headers,
        rows,
    ))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::parse_csv;
    use crate::error::AppError;
    use crate::importer::headers::PRODUCT_INTEREST_HEADER;
    use crate::importer::product_interest::{parse_product_answer, ProductAnswerMode};
    use crate::importer::source::SourceFormat;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join(name)
    }

    fn temporary_csv(bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ertip-leads-{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).expect("write temporary CSV");
        path
    }

    #[test]
    fn structured_fixture_preserves_quoted_product_field_and_agency_columns() {
        let table = parse_csv(&fixture("leads_sample_multiselect_sanitized.csv"))
            .expect("parse structured CSV fixture");

        assert_eq!(table.format, SourceFormat::Csv);
        assert_eq!(table.rows.len(), 6);

        let all_products_row = &table.rows[3];
        let product_raw = all_products_row
            .get(PRODUCT_INTEREST_HEADER)
            .expect("product field");

        assert!(product_raw.contains("implanters,_forceps_&_surgical_instruments"));
        assert_eq!(all_products_row.get("Status"), Some("Contacted"));
        assert_eq!(
            all_products_row.get("İletişime Geçme Tarihi"),
            Some("2026-08-21 10:00")
        );

        let parsed = parse_product_answer(product_raw);
        assert_eq!(parsed.mode, ProductAnswerMode::Structured);
        assert_eq!(parsed.interests.len(), 6);
    }

    #[test]
    fn exact_duplicate_external_id_remains_visible_to_identity_pipeline() {
        let table = parse_csv(&fixture("leads_sample_multiselect_sanitized.csv"))
            .expect("parse structured CSV fixture");

        assert_eq!(table.rows[1].get("id"), Some("l:demo2002"));
        assert_eq!(table.rows[5].get("id"), Some("l:demo2002"));
    }

    #[test]
    fn utf8_bom_and_unknown_optional_columns_are_supported() {
        let content = concat!(
            "\u{feff}id,created_time,full_name,email,phone_number,custom_agency_column\n",
            "l:bom,2026-08-21T12:00:00+03:00,BOM Demo,bom@example.test,p:+905551234567,keep-me\n"
        );
        let path = temporary_csv(content.as_bytes());

        let table = parse_csv(&path).expect("parse BOM CSV");
        std::fs::remove_file(&path).ok();

        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].get("id"), Some("l:bom"));
        assert_eq!(
            table.rows[0].get("custom_agency_column"),
            Some("keep-me")
        );
        assert!(table.rows[0].get("campaign_id").is_none());
    }

    #[test]
    fn invalid_utf8_is_rejected_with_encoding_error() {
        let path = temporary_csv(&[0xff, 0xfe, 0xfd]);
        let error = parse_csv(&path).expect_err("reject invalid UTF-8 CSV");
        std::fs::remove_file(&path).ok();

        assert!(matches!(error, AppError::CsvEncoding));
    }
}
