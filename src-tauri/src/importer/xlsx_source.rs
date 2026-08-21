use std::collections::BTreeMap;
use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};

use crate::error::AppError;

use super::headers::has_required_headers;
use super::source::{SourceFormat, SourceRow, SourceTable};

const MAX_HEADER_SCAN_ROWS: usize = 50;

pub fn parse_xlsx(path: &Path) -> Result<SourceTable, AppError> {
    let mut workbook = open_workbook_auto(path)?;
    let sheet_names = workbook.sheet_names();

    for sheet_name in sheet_names {
        let range = workbook.worksheet_range(&sheet_name)?;

        let Some((header_row_index, headers)) = find_header_row(&range) else {
            continue;
        };

        let mut rows = Vec::new();

        for (offset, row) in range.rows().skip(header_row_index + 1).enumerate() {
            let values = row.iter().map(cell_to_string).collect::<Vec<_>>();

            if values.iter().all(|value| value.trim().is_empty()) {
                continue;
            }

            let fields = headers
                .iter()
                .enumerate()
                .map(|(column, header)| {
                    (
                        header.clone(),
                        values.get(column).cloned().unwrap_or_default(),
                    )
                })
                .collect::<BTreeMap<_, _>>();

            rows.push(SourceRow::new(header_row_index + offset + 2, fields));
        }

        return Ok(SourceTable::new(
            SourceFormat::Xlsx,
            file_name(path),
            Some(sheet_name),
            headers,
            rows,
        ));
    }

    Err(AppError::ImportSchema(
        "required lead headers were not found in any XLSX worksheet".to_string(),
    ))
}

fn find_header_row(range: &calamine::Range<Data>) -> Option<(usize, Vec<String>)> {
    range
        .rows()
        .take(MAX_HEADER_SCAN_ROWS)
        .enumerate()
        .find_map(|(index, row)| {
            let headers = row.iter().map(cell_to_string).collect::<Vec<_>>();

            if has_required_headers(&headers) {
                Some((index, headers))
            } else {
                None
            }
        })
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        _ => cell.to_string(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::parse_xlsx;
    use crate::importer::source::SourceFormat;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn parses_legacy_xlsx_fixture_into_canonical_source_rows() {
        let table = parse_xlsx(&fixture("leads_sample_sanitized.xlsx"))
            .expect("parse legacy XLSX fixture");

        assert_eq!(table.format, SourceFormat::Xlsx);
        assert!(table.sheet_name.is_some());
        assert!(!table.rows.is_empty());
        assert!(table.rows[0].get("id").is_some());
        assert!(table.rows[0].get("created_time").is_some());
    }
}
