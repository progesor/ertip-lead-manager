use std::path::Path;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::importer::headers::{is_agency_ignored_crm_column, PRODUCT_INTEREST_HEADER};
use crate::importer::identity::{IdentityDecision, IdentityEngine};
use crate::importer::normalization::{normalize_source_row, NormalizedSubmission};
use crate::importer::source::SourceFormat;
use crate::importer::parse_file;
use crate::repositories::import_preview_repository::ImportPreviewRepository;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewSource {
    pub file_name: String,
    pub file_size: Option<u64>,
    pub format: SourceFormat,
    pub sheet_name: Option<String>,
    pub column_count: usize,
    pub ignored_agency_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewSummary {
    pub total_rows: usize,
    pub importable_submissions: usize,
    pub new_contacts: usize,
    pub repeat_submissions: usize,
    pub exact_duplicates: usize,
    pub identity_conflicts: usize,
    pub row_errors: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewRow {
    pub row_number: usize,
    pub full_name: String,
    pub raw_email: String,
    pub raw_phone: String,
    pub raw_country: String,
    pub raw_product_answer: String,
    pub normalized: NormalizedSubmission,
    pub decision: IdentityDecision,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub source: ImportPreviewSource,
    pub summary: ImportPreviewSummary,
    pub rows: Vec<ImportPreviewRow>,
}

#[derive(Clone)]
pub struct ImportPreviewService {
    repository: ImportPreviewRepository,
}

impl ImportPreviewService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: ImportPreviewRepository::new(pool),
        }
    }

    pub async fn preview(&self, path: &Path) -> Result<ImportPreview, AppError> {
        let table = parse_file(path)?;
        let snapshot = self.repository.load_identity_snapshot().await?;
        let mut identity = IdentityEngine::new(snapshot.external_lead_ids, snapshot.contacts);
        let mut rows = Vec::with_capacity(table.rows.len());
        let mut summary = ImportPreviewSummary {
            total_rows: table.rows.len(),
            ..ImportPreviewSummary::default()
        };

        for source_row in &table.rows {
            let normalized = normalize_source_row(source_row);
            summary.warning_count += normalized.warnings.len();

            let decision = identity.decide(&normalized);
            match &decision {
                IdentityDecision::NewContact => {
                    summary.new_contacts += 1;
                    summary.importable_submissions += 1;

                    let provisional_contact_id = format!(
                        "preview:{}",
                        normalized.external_lead_id.trim()
                    );
                    identity.register_contact_identity(
                        provisional_contact_id,
                        normalized.normalized_email.as_deref(),
                        normalized.normalized_phone.as_deref(),
                    );
                }
                IdentityDecision::RepeatContact { contact_id, .. } => {
                    summary.repeat_submissions += 1;
                    summary.importable_submissions += 1;

                    identity.register_contact_identity(
                        contact_id.clone(),
                        normalized.normalized_email.as_deref(),
                        normalized.normalized_phone.as_deref(),
                    );
                }
                IdentityDecision::ExactDuplicateSubmission { .. } => {
                    summary.exact_duplicates += 1;
                }
                IdentityDecision::IdentityConflictReview { .. } => {
                    summary.identity_conflicts += 1;
                }
                IdentityDecision::RowError { .. } => {
                    summary.row_errors += 1;
                }
            }

            rows.push(ImportPreviewRow {
                row_number: source_row.row_number,
                full_name: source_row.get("full_name").unwrap_or_default().to_string(),
                raw_email: source_row.get("email").unwrap_or_default().to_string(),
                raw_phone: source_row.get("phone_number").unwrap_or_default().to_string(),
                raw_country: source_row.get("country").unwrap_or_default().to_string(),
                raw_product_answer: source_row
                    .get(PRODUCT_INTEREST_HEADER)
                    .unwrap_or_default()
                    .to_string(),
                normalized,
                decision,
            });
        }

        let ignored_agency_columns = table
            .headers
            .iter()
            .filter(|header| is_agency_ignored_crm_column(header))
            .cloned()
            .collect();

        Ok(ImportPreview {
            source: ImportPreviewSource {
                file_name: table.source_name,
                file_size: std::fs::metadata(path).ok().map(|metadata| metadata.len()),
                format: table.format,
                sheet_name: table.sheet_name,
                column_count: table.headers.len(),
                ignored_agency_columns,
            },
            summary,
            rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::{SecondsFormat, Utc};

    use super::ImportPreviewService;
    use crate::db::Database;
    use crate::importer::identity::IdentityDecision;

    fn multiselect_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/leads_sample_multiselect_sanitized.csv")
    }

    #[tokio::test]
    async fn preview_classifies_new_repeat_and_duplicate_rows_within_one_file() {
        let database = Database::connect_memory().await.expect("open database");
        let preview = ImportPreviewService::new(database.pool().clone())
            .preview(&multiselect_fixture())
            .await
            .expect("preview fixture");

        assert_eq!(preview.summary.total_rows, 6);
        assert_eq!(preview.summary.new_contacts, 4);
        assert_eq!(preview.summary.repeat_submissions, 1);
        assert_eq!(preview.summary.exact_duplicates, 1);
        assert_eq!(preview.summary.importable_submissions, 5);
        assert_eq!(preview.summary.identity_conflicts, 0);
        assert_eq!(preview.summary.row_errors, 0);
        assert_eq!(preview.summary.warning_count, 0);
        assert_eq!(
            preview.source.ignored_agency_columns,
            vec!["Status", "İletişime Geçme Tarihi"]
        );

        assert!(matches!(
            preview.rows[4].decision,
            IdentityDecision::RepeatContact { .. }
        ));
        assert!(matches!(
            preview.rows[5].decision,
            IdentityDecision::ExactDuplicateSubmission { .. }
        ));
    }

    #[tokio::test]
    async fn preview_uses_database_snapshot_without_mutating_existing_crm_data() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, normalized_email, normalized_phone, status, created_at, updated_at, submission_count) VALUES (?, ?, ?, ?, 'QUALIFIED', ?, ?, 1)",
        )
        .bind("contact-alex")
        .bind("Alex Existing")
        .bind("alex.demo@example.test")
        .bind("+351910000001")
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert contact");

        sqlx::query(
            "INSERT INTO import_batches (id, file_name, sheet_name, started_at, status, total_rows, app_version) VALUES (?, ?, ?, ?, 'COMMITTED', 1, ?)",
        )
        .bind("batch-existing")
        .bind("old.csv")
        .bind("CSV")
        .bind(&now)
        .bind("0.1.0")
        .execute(database.pool())
        .await
        .expect("insert batch");

        sqlx::query(
            "INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_raw, raw_payload_json, created_at) VALUES (?, ?, ?, ?, ?, '{}', ?)",
        )
        .bind("submission-existing")
        .bind("contact-alex")
        .bind("batch-existing")
        .bind("l:demo2001")
        .bind("2026-08-20T10:00:00+03:00")
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert submission");

        let preview = ImportPreviewService::new(database.pool().clone())
            .preview(&multiselect_fixture())
            .await
            .expect("preview fixture");

        assert_eq!(preview.summary.new_contacts, 3);
        assert_eq!(preview.summary.repeat_submissions, 1);
        assert_eq!(preview.summary.exact_duplicates, 2);

        let status: String = sqlx::query_scalar("SELECT status FROM lead_contacts WHERE id = ?")
            .bind("contact-alex")
            .fetch_one(database.pool())
            .await
            .expect("read status");
        let submission_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_submissions")
            .fetch_one(database.pool())
            .await
            .expect("count submissions");

        assert_eq!(status, "QUALIFIED");
        assert_eq!(submission_count, 1);
    }
}
