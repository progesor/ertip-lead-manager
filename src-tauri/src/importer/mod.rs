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
    use std::path::Path;

    use super::parse_file;
    use crate::error::AppError;

    #[test]
    fn unsupported_extension_is_rejected_before_parsing() {
        let error = parse_file(Path::new("leads.txt")).expect_err("reject unsupported file");

        assert!(matches!(error, AppError::UnsupportedFileType(_)));
    }
}
