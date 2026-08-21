use std::collections::BTreeMap;

use serde::Serialize;

use super::headers::normalize_header;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceFormat {
    Xlsx,
    Csv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub row_number: usize,
    pub fields: BTreeMap<String, String>,
}

impl SourceRow {
    pub fn new(row_number: usize, fields: BTreeMap<String, String>) -> Self {
        Self { row_number, fields }
    }

    pub fn get(&self, header: &str) -> Option<&str> {
        let wanted = normalize_header(header);

        self.fields.iter().find_map(|(key, value)| {
            if normalize_header(key) == wanted {
                Some(value.as_str())
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTable {
    pub format: SourceFormat,
    pub source_name: String,
    pub sheet_name: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<SourceRow>,
}

impl SourceTable {
    pub fn new(
        format: SourceFormat,
        source_name: String,
        sheet_name: Option<String>,
        headers: Vec<String>,
        rows: Vec<SourceRow>,
    ) -> Self {
        Self {
            format,
            source_name,
            sheet_name,
            headers,
            rows,
        }
    }
}
