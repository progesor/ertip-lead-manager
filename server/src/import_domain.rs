use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};

use calamine::{Data, ExcelDateTime, Reader, open_workbook_auto};
use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use csv::ReaderBuilder;
use serde::Serialize;
use thiserror::Error;

pub const PRODUCT_INTEREST_HEADER: &str =
    "which_product_would_you_like_to_receive_more_information_about?";
pub const REQUIRED_HEADERS: [&str; 5] = [
    "id",
    "created_time",
    "full_name",
    "email",
    "phone_number",
];
pub const AGENCY_IGNORED_CRM_COLUMNS: [&str; 2] = ["Status", "İletişime Geçme Tarihi"];

const MAX_HEADER_SCAN_ROWS: usize = 50;
const MILLIS_PER_DAY: f64 = 86_400_000.0;

#[derive(Debug, Error)]
pub enum ImportDomainError {
    #[error("unsupported import file format; expected .csv or .xlsx")]
    UnsupportedFormat,
    #[error("CSV must be UTF-8 encoded")]
    CsvEncoding,
    #[error("import schema error: {0}")]
    Schema(String),
    #[error("failed to read import file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error("failed to parse XLSX: {0}")]
    Xlsx(#[from] calamine::Error),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceFormat {
    Xlsx,
    Csv,
}

impl SourceFormat {
    pub fn from_file_name(file_name: &str) -> Result<Self, ImportDomainError> {
        let lower = file_name.trim().to_ascii_lowercase();
        if lower.ends_with(".csv") {
            Ok(Self::Csv)
        } else if lower.ends_with(".xlsx") {
            Ok(Self::Xlsx)
        } else {
            Err(ImportDomainError::UnsupportedFormat)
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Xlsx => "XLSX",
        }
    }
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
            (normalize_header(key) == wanted).then_some(value.as_str())
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

pub fn parse_source_file(
    path: &Path,
    source_name: &str,
    format: SourceFormat,
) -> Result<SourceTable, ImportDomainError> {
    match format {
        SourceFormat::Csv => parse_csv(path, source_name),
        SourceFormat::Xlsx => parse_xlsx(path, source_name),
    }
}

fn parse_csv(path: &Path, source_name: &str) -> Result<SourceTable, ImportDomainError> {
    let bytes = std::fs::read(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ImportDomainError::CsvEncoding)?;
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
        return Err(ImportDomainError::Schema(
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

    Ok(SourceTable {
        format: SourceFormat::Csv,
        source_name: source_name.to_string(),
        sheet_name: None,
        headers,
        rows,
    })
}

fn parse_xlsx(path: &Path, source_name: &str) -> Result<SourceTable, ImportDomainError> {
    let mut workbook = open_workbook_auto(path)?;
    for sheet_name in workbook.sheet_names() {
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

        return Ok(SourceTable {
            format: SourceFormat::Xlsx,
            source_name: source_name.to_string(),
            sheet_name: Some(sheet_name),
            headers,
            rows,
        });
    }

    Err(ImportDomainError::Schema(
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
            has_required_headers(&headers).then_some((index, headers))
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
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)
        .expect("valid Excel epoch")
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    let milliseconds = (value * MILLIS_PER_DAY).round() as i64;
    (epoch + Duration::milliseconds(milliseconds))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn excel_datetime_to_iso(value: ExcelDateTime) -> String {
    let (year, month, day, hour, minute, second, millis) = value.to_ymd_hms_milli();
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

pub fn normalize_header(value: &str) -> String {
    value
        .trim_start_matches('\u{feff}')
        .trim()
        .to_lowercase()
}

pub fn has_required_headers(headers: &[String]) -> bool {
    REQUIRED_HEADERS.iter().all(|required| {
        let required = normalize_header(required);
        headers
            .iter()
            .any(|header| normalize_header(header) == required)
    })
}

pub fn is_agency_ignored_crm_column(header: &str) -> bool {
    AGENCY_IGNORED_CRM_COLUMNS
        .iter()
        .any(|candidate| header.trim().eq_ignore_ascii_case(candidate))
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductCode {
    FueMicromotorSystems,
    LongHairFueSolutions,
    FuePunches,
    ImplantersForcepsSurgicalInstruments,
    MedicalChairsClinicFurniture,
    OtherGeneralInformation,
    Unknown,
}

impl ProductCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FueMicromotorSystems => "FUE_MICROMOTOR_SYSTEMS",
            Self::LongHairFueSolutions => "LONG_HAIR_FUE_SOLUTIONS",
            Self::FuePunches => "FUE_PUNCHES",
            Self::ImplantersForcepsSurgicalInstruments => {
                "IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS"
            }
            Self::MedicalChairsClinicFurniture => "MEDICAL_CHAIRS_CLINIC_FURNITURE",
            Self::OtherGeneralInformation => "OTHER_GENERAL_INFORMATION",
            Self::Unknown => "UNKNOWN",
        }
    }
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
    value.contains('|') || machine_value_to_code(value).is_some() || looks_like_machine_value(value)
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

fn machine_value_to_code(value: &str) -> Option<ProductCode> {
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

impl NormalizationWarning {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidEmail => "INVALID_EMAIL",
            Self::InvalidPhone => "INVALID_PHONE",
            Self::InvalidCountry => "INVALID_COUNTRY",
            Self::InvalidTimestamp => "INVALID_TIMESTAMP",
            Self::MissingContactMethod => "MISSING_CONTACT_METHOD",
            Self::UnknownProduct => "UNKNOWN_PRODUCT",
        }
    }
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

fn normalize_email(raw: &str) -> Option<String> {
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

fn normalize_phone(raw: &str) -> Option<String> {
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
    Some(if has_plus { format!("+{digits}") } else { digits })
}

fn normalize_country(raw: &str) -> Option<String> {
    let value = raw.trim().to_ascii_uppercase();
    (value.len() == 2 && value.chars().all(|character| character.is_ascii_alphabetic()))
        .then_some(value)
}

fn normalize_timestamp(raw: &str) -> Option<String> {
    let parsed = DateTime::parse_from_rfc3339(raw.trim()).ok()?;
    Some(
        parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true),
    )
}

fn normalize_legacy_product(raw: &str) -> Vec<ProductCode> {
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
    if matches!(
        value.as_str(),
        "all" | "all products" | "all product" | "general information"
    ) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactIdentity {
    pub contact_id: String,
    pub normalized_email: Option<String>,
    pub normalized_phone: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityMatchKind {
    Email,
    Phone,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityDecision {
    NewContact,
    RepeatContact {
        contact_id: String,
        matched_by: Vec<IdentityMatchKind>,
    },
    ExactDuplicateSubmission {
        external_lead_id: String,
    },
    IdentityConflictReview {
        candidate_contact_ids: Vec<String>,
    },
    RowError {
        code: &'static str,
    },
}

struct IdentityEngine {
    existing_external_ids: HashSet<String>,
    seen_external_ids: HashSet<String>,
    email_index: HashMap<String, BTreeSet<String>>,
    phone_index: HashMap<String, BTreeSet<String>>,
}

impl IdentityEngine {
    fn new(
        existing_external_ids: impl IntoIterator<Item = String>,
        contacts: impl IntoIterator<Item = ContactIdentity>,
    ) -> Self {
        let mut engine = Self {
            existing_external_ids: existing_external_ids.into_iter().collect(),
            seen_external_ids: HashSet::new(),
            email_index: HashMap::new(),
            phone_index: HashMap::new(),
        };
        for contact in contacts {
            engine.register_contact_identity(
                contact.contact_id,
                contact.normalized_email.as_deref(),
                contact.normalized_phone.as_deref(),
            );
        }
        engine
    }

    fn register_contact_identity(
        &mut self,
        contact_id: impl Into<String>,
        normalized_email: Option<&str>,
        normalized_phone: Option<&str>,
    ) {
        let contact_id = contact_id.into();
        if let Some(email) = normalized_email.filter(|value| !value.is_empty()) {
            self.email_index
                .entry(email.to_string())
                .or_default()
                .insert(contact_id.clone());
        }
        if let Some(phone) = normalized_phone.filter(|value| !value.is_empty()) {
            self.phone_index
                .entry(phone.to_string())
                .or_default()
                .insert(contact_id);
        }
    }

    fn decide(&mut self, submission: &NormalizedSubmission) -> IdentityDecision {
        let external_id = submission.external_lead_id.trim();
        if external_id.is_empty() {
            return IdentityDecision::RowError {
                code: "MISSING_EXTERNAL_LEAD_ID",
            };
        }
        if self.existing_external_ids.contains(external_id)
            || !self.seen_external_ids.insert(external_id.to_string())
        {
            return IdentityDecision::ExactDuplicateSubmission {
                external_lead_id: external_id.to_string(),
            };
        }

        let email_candidates = submission
            .normalized_email
            .as_ref()
            .and_then(|email| self.email_index.get(email))
            .cloned()
            .unwrap_or_default();
        let phone_candidates = submission
            .normalized_phone
            .as_ref()
            .and_then(|phone| self.phone_index.get(phone))
            .cloned()
            .unwrap_or_default();
        if email_candidates.len() > 1 || phone_candidates.len() > 1 {
            return conflict(email_candidates.union(&phone_candidates).cloned().collect());
        }
        let email_contact = email_candidates.iter().next().cloned();
        let phone_contact = phone_candidates.iter().next().cloned();
        match (email_contact, phone_contact) {
            (Some(email_id), Some(phone_id)) if email_id == phone_id => {
                IdentityDecision::RepeatContact {
                    contact_id: email_id,
                    matched_by: vec![IdentityMatchKind::Email, IdentityMatchKind::Phone],
                }
            }
            (Some(email_id), Some(phone_id)) => conflict(vec![email_id, phone_id]),
            (Some(contact_id), None) => IdentityDecision::RepeatContact {
                contact_id,
                matched_by: vec![IdentityMatchKind::Email],
            },
            (None, Some(contact_id)) => IdentityDecision::RepeatContact {
                contact_id,
                matched_by: vec![IdentityMatchKind::Phone],
            },
            (None, None) => IdentityDecision::NewContact,
        }
    }
}

fn conflict(mut candidate_contact_ids: Vec<String>) -> IdentityDecision {
    candidate_contact_ids.sort();
    candidate_contact_ids.dedup();
    IdentityDecision::IdentityConflictReview {
        candidate_contact_ids,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanSummary {
    pub total_rows: usize,
    pub importable_submissions: usize,
    pub new_contacts: usize,
    pub repeat_submissions: usize,
    pub exact_duplicates: usize,
    pub identity_conflicts: usize,
    pub row_errors: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedImportRow {
    pub source: SourceRow,
    pub normalized: NormalizedSubmission,
    pub decision: IdentityDecision,
    pub target_contact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub summary: ImportPlanSummary,
    pub rows: Vec<PlannedImportRow>,
}

impl ImportPlan {
    pub fn has_blocking_rows(&self) -> bool {
        self.summary.identity_conflicts > 0 || self.summary.row_errors > 0
    }
}

pub fn build_import_plan<F>(
    table: &SourceTable,
    existing_external_ids: impl IntoIterator<Item = String>,
    contacts: impl IntoIterator<Item = ContactIdentity>,
    mut create_contact_id: F,
) -> ImportPlan
where
    F: FnMut(&NormalizedSubmission) -> String,
{
    let mut identity = IdentityEngine::new(existing_external_ids, contacts);
    let mut summary = ImportPlanSummary {
        total_rows: table.rows.len(),
        ..ImportPlanSummary::default()
    };
    let mut rows = Vec::with_capacity(table.rows.len());

    for source in &table.rows {
        let normalized = normalize_source_row(source);
        summary.warning_count += normalized.warnings.len();
        let mut decision = identity.decide(&normalized);
        if matches!(decision, IdentityDecision::NewContact)
            && !has_useful_identity(source, &normalized)
        {
            decision = IdentityDecision::RowError {
                code: "MISSING_IDENTITY_FIELDS",
            };
        }
        let target_contact_id = match &decision {
            IdentityDecision::NewContact => {
                summary.new_contacts += 1;
                summary.importable_submissions += 1;
                let contact_id = create_contact_id(&normalized);
                identity.register_contact_identity(
                    contact_id.clone(),
                    normalized.normalized_email.as_deref(),
                    normalized.normalized_phone.as_deref(),
                );
                Some(contact_id)
            }
            IdentityDecision::RepeatContact { contact_id, .. } => {
                summary.repeat_submissions += 1;
                summary.importable_submissions += 1;
                identity.register_contact_identity(
                    contact_id.clone(),
                    normalized.normalized_email.as_deref(),
                    normalized.normalized_phone.as_deref(),
                );
                Some(contact_id.clone())
            }
            IdentityDecision::ExactDuplicateSubmission { .. } => {
                summary.exact_duplicates += 1;
                None
            }
            IdentityDecision::IdentityConflictReview { .. } => {
                summary.identity_conflicts += 1;
                None
            }
            IdentityDecision::RowError { .. } => {
                summary.row_errors += 1;
                None
            }
        };
        rows.push(PlannedImportRow {
            source: source.clone(),
            normalized,
            decision,
            target_contact_id,
        });
    }
    ImportPlan { summary, rows }
}

fn has_useful_identity(source: &SourceRow, normalized: &NormalizedSubmission) -> bool {
    !source.get("full_name").unwrap_or_default().trim().is_empty()
        || normalized.normalized_email.is_some()
        || normalized.normalized_phone.is_some()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ContactIdentity, IdentityDecision, ImportDomainError, ProductCode, SourceFormat, SourceRow,
        SourceTable, build_import_plan, normalize_source_row,
    };

    fn row(number: usize, id: &str, name: &str, email: &str, phone: &str) -> SourceRow {
        let mut fields = BTreeMap::new();
        fields.insert("id".to_string(), id.to_string());
        fields.insert(
            "created_time".to_string(),
            "2026-08-21T12:00:00+03:00".to_string(),
        );
        fields.insert("full_name".to_string(), name.to_string());
        fields.insert("email".to_string(), email.to_string());
        fields.insert("phone_number".to_string(), phone.to_string());
        SourceRow::new(number, fields)
    }

    #[test]
    fn server_domain_preserves_local_normalization_and_same_batch_repeat_semantics() {
        let mut product_row = row(
            2,
            "m6:1",
            "Demo",
            " Demo.Person@Example.COM ",
            "p:+90 (555) 123 45 67",
        );
        product_row.fields.insert("country".to_string(), " tr ".to_string());
        product_row.fields.insert(
            super::PRODUCT_INTEREST_HEADER.to_string(),
            "fue_micromotor_systems|fue_punches|long_hair_fue_solutions".to_string(),
        );
        let normalized = normalize_source_row(&product_row);
        assert_eq!(normalized.normalized_email.as_deref(), Some("demo.person@example.com"));
        assert_eq!(normalized.normalized_phone.as_deref(), Some("+905551234567"));
        assert_eq!(normalized.country_code.as_deref(), Some("TR"));
        assert_eq!(normalized.product_interests.len(), 3);
        assert!(normalized.product_interests.contains(&ProductCode::FuePunches));

        let table = SourceTable {
            format: SourceFormat::Csv,
            source_name: "fixture.csv".to_string(),
            sheet_name: None,
            headers: vec![],
            rows: vec![
                row(2, "m6:first", "First", "same@example.test", "p:+905551234567"),
                row(3, "m6:second", "Second", "same@example.test", "p:+905551234567"),
            ],
        };
        let plan = build_import_plan(&table, Vec::<String>::new(), vec![], |normalized| {
            format!("contact:{}", normalized.external_lead_id)
        });
        assert_eq!(plan.summary.new_contacts, 1);
        assert_eq!(plan.summary.repeat_submissions, 1);
        assert_eq!(plan.rows[1].target_contact_id.as_deref(), Some("contact:m6:first"));
    }

    #[test]
    fn identity_conflict_and_missing_external_id_remain_blocking() {
        let contacts = vec![
            ContactIdentity {
                contact_id: "a".to_string(),
                normalized_email: Some("same@example.test".to_string()),
                normalized_phone: None,
            },
            ContactIdentity {
                contact_id: "b".to_string(),
                normalized_email: None,
                normalized_phone: Some("+905551234567".to_string()),
            },
        ];
        let table = SourceTable {
            format: SourceFormat::Csv,
            source_name: "fixture.csv".to_string(),
            sheet_name: None,
            headers: vec![],
            rows: vec![
                row(2, "m6:conflict", "Conflict", "same@example.test", "p:+905551234567"),
                row(3, "", "No Id", "new@example.test", "p:+905559999999"),
            ],
        };
        let plan = build_import_plan(&table, Vec::<String>::new(), contacts, |_| "new".to_string());
        assert!(plan.has_blocking_rows());
        assert_eq!(plan.summary.identity_conflicts, 1);
        assert_eq!(plan.summary.row_errors, 1);
        assert!(matches!(plan.rows[0].decision, IdentityDecision::IdentityConflictReview { .. }));
        assert!(matches!(plan.rows[1].decision, IdentityDecision::RowError { .. }));
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        assert!(matches!(
            SourceFormat::from_file_name("leads.xls"),
            Err(ImportDomainError::UnsupportedFormat)
        ));
    }
}
