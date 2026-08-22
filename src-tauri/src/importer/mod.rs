mod csv_source;
pub mod headers;
pub mod identity;
pub mod normalization;
pub mod planning;
pub mod product_interest;
pub mod source;
mod xlsx_source;

use std::path::Path;

use crate::error::AppError;
use source::SourceTable;

pub fn parse_file(path: &Path) -> Result<SourceTable, AppError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "csv" => csv_source::parse_csv(path),
        "xlsx" => xlsx_source::parse_xlsx(path),
        _ => Err(AppError::UnsupportedFileType(extension)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::parse_file;
    use crate::error::AppError;
    use crate::importer::headers::PRODUCT_INTEREST_HEADER;
    use crate::importer::normalization::normalize_source_row;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn unsupported_extension_is_rejected_before_parsing() {
        let error = parse_file(Path::new("leads.txt")).expect_err("reject unsupported file");

        assert!(matches!(error, AppError::UnsupportedFileType(_)));
    }

    #[test]
    fn equivalent_multiselect_xlsx_and_csv_produce_equivalent_canonical_fields() {
        let csv = parse_file(&fixture("leads_sample_multiselect_sanitized.csv"))
            .expect("parse CSV fixture");
        let xlsx = parse_file(&fixture("leads_sample_multiselect_sanitized.xlsx"))
            .expect("parse XLSX fixture");

        assert_eq!(csv.rows.len(), xlsx.rows.len());

        // XLSX may represent a date/time as a native Excel serial, while CSV can preserve
        // the original timezone-bearing text. Raw representations can therefore differ;
        // the canonical UTC timestamp must be equivalent.
        let compared_raw_headers = [
            "id",
            "full_name",
            "email",
            "phone_number",
            "country",
            "lead_status",
            PRODUCT_INTEREST_HEADER,
            "Status",
            "İletişime Geçme Tarihi",
        ];

        for (csv_row, xlsx_row) in csv.rows.iter().zip(&xlsx.rows) {
            for header in compared_raw_headers {
                assert_eq!(
                    csv_row.get(header),
                    xlsx_row.get(header),
                    "adapter mismatch for header {header} on source row {}",
                    csv_row.row_number
                );
            }

            assert_eq!(
                normalize_source_row(csv_row).created_at_utc,
                normalize_source_row(xlsx_row).created_at_utc,
                "canonical timestamp mismatch on source row {}",
                csv_row.row_number
            );
        }
    }
}
