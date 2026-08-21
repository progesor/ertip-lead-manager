use std::collections::BTreeMap;
use std::path::Path;

use calamine::{open_workbook_auto, Data, ExcelDateTime, ExcelDateTimeType, Reader};

use crate::error::AppError;

use super::headers::{has_required_headers, normalize_header};
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
            let values = headers
                .iter()
                .enumerate()
                .map(|(column, header)| {
                    row.get(column)
                        .map(|cell| source_cell_to_string(header, cell))
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();

            if values.iter().all(|value| value.trim().is_empty()) {
                continue;
            }

            let fields = headers
                .iter()
                .cloned()
                .zip(values.into_iter())
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

fn source_cell_to_string(header: &str, cell: &Data) -> String {
    if normalize_header(header) == "created_time" {
        match cell {
            Data::Float(value) => return excel_serial_to_iso(*value),
            Data::Int(value) => return excel_serial_to_iso(*value as f64),
            _ => {}
        }
    }

    cell_to_string(cell)
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::DateTime(value) if value.is_datetime() => excel_datetime_to_iso(*value),
        Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        _ => cell.to_string(),
    }
}

fn excel_serial_to_iso(value: f64) -> String {
    // Numeric fallback is restricted to the canonical created_time column. Meta exports
    // normally provide an RFC3339 string; this also tolerates workbooks that store the
    // same instant as a native/serial Excel datetime. A naked serial carries no timezone,
    // so the fallback is represented as UTC.
    excel_datetime_to_iso(ExcelDateTime::new(
        value,
        ExcelDateTimeType::DateTime,
        false,
    ))
}

fn excel_datetime_to_iso(value: ExcelDateTime) -> String {
    let (year, month, day, hour, minute, second, millis) = value.to_ymd_hms_milli();
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
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
    use crate::importer::headers::PRODUCT_INTEREST_HEADER;
    use crate::importer::normalization::normalize_source_row;
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

    #[test]
    fn parses_verified_multiselect_xlsx_without_losing_pipe_values() {
        let table = parse_xlsx(&fixture("leads_sample_multiselect_sanitized.xlsx"))
            .expect("parse multi-select XLSX fixture");

        assert_eq!(table.rows.len(), 6);
        assert_eq!(
            normalize_source_row(&table.rows[0]).created_at_utc.as_deref(),
            Some("2026-08-20T07:00:00.000Z")
        );
        assert_eq!(
            table.rows[2].get(PRODUCT_INTEREST_HEADER),
            Some("fue_micromotor_systems|fue_punches|long_hair_fue_solutions")
        );
        assert_eq!(
            table.rows[3].get(PRODUCT_INTEREST_HEADER),
            Some("fue_micromotor_systems|other_products_/_general_information|medical_chairs_&_clinic_furniture|implanters,_forceps_&_surgical_instruments|fue_punches|long_hair_fue_solutions")
        );
    }
}
