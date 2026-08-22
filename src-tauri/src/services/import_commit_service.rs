use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::AppError;
use crate::importer::headers::PRODUCT_INTEREST_HEADER;
use crate::importer::identity::{ContactIdentity, IdentityDecision};
use crate::importer::normalization::NormalizationWarning;
use crate::importer::planning::{build_import_plan, ImportPlanSummary};
use crate::importer::product_interest::{parse_product_answer, ProductAnswerMode, ProductCode};
use crate::importer::source::{SourceFormat, SourceRow};
use crate::importer::parse_file;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitImportResult {
    pub batch_id: String,
    pub summary: ImportPlanSummary,
}

#[derive(Clone)]
pub struct ImportCommitService {
    pool: SqlitePool,
}

impl ImportCommitService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn commit(
        &self,
        path: &Path,
        app_version: &str,
    ) -> Result<CommitImportResult, AppError> {
        // Re-read the file at commit time. Preview is advisory; commit revalidates against
        // both the current source file and the current database state.
        let table = parse_file(path)?;
        let mut transaction = self.pool.begin().await?;

        let external_lead_ids = sqlx::query_scalar::<_, String>(
            "SELECT external_lead_id FROM lead_submissions",
        )
        .fetch_all(&mut *transaction)
        .await?;

        let contact_rows = sqlx::query(
            "SELECT id, normalized_email, normalized_phone FROM lead_contacts",
        )
        .fetch_all(&mut *transaction)
        .await?;

        let contacts = contact_rows
            .into_iter()
            .map(|row| ContactIdentity {
                contact_id: row.get("id"),
                normalized_email: row.get("normalized_email"),
                normalized_phone: row.get("normalized_phone"),
            })
            .collect::<Vec<_>>();

        let plan = build_import_plan(&table, external_lead_ids, contacts, |_| {
            Uuid::new_v4().to_string()
        });

        if plan.has_blocking_rows() {
            transaction.rollback().await?;
            return Err(AppError::ImportBlocked(format!(
                "{} identity conflicts, {} row errors",
                plan.summary.identity_conflicts, plan.summary.row_errors
            )));
        }

        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let batch_id = Uuid::new_v4().to_string();
        let sheet_name = table
            .sheet_name
            .clone()
            .unwrap_or_else(|| match table.format {
                SourceFormat::Csv => "CSV".to_string(),
                SourceFormat::Xlsx => "XLSX".to_string(),
            });
        let file_size = std::fs::metadata(path).ok().map(|metadata| metadata.len() as i64);

        sqlx::query(
            r#"
            INSERT INTO import_batches (
                id, file_name, file_size, file_sha256, sheet_name,
                started_at, completed_at, status, total_rows,
                new_submissions, exact_duplicates, repeat_candidates,
                warning_count, error_count, app_version
            ) VALUES (?, ?, ?, NULL, ?, ?, ?, 'COMMITTED', ?, ?, ?, ?, ?, 0, ?)
            "#,
        )
        .bind(&batch_id)
        .bind(&table.source_name)
        .bind(file_size)
        .bind(&sheet_name)
        .bind(&now)
        .bind(&now)
        .bind(plan.summary.total_rows as i64)
        .bind(plan.summary.importable_submissions as i64)
        .bind(plan.summary.exact_duplicates as i64)
        .bind(plan.summary.repeat_submissions as i64)
        .bind(plan.summary.warning_count as i64)
        .bind(app_version)
        .execute(&mut *transaction)
        .await?;

        for planned in &plan.rows {
            if matches!(
                planned.decision,
                IdentityDecision::ExactDuplicateSubmission { .. }
            ) {
                continue;
            }

            let target_contact_id = planned.target_contact_id.as_ref().ok_or_else(|| {
                AppError::ImportBlocked("importable row has no target contact".to_string())
            })?;

            let is_new_contact = matches!(planned.decision, IdentityDecision::NewContact);
            if is_new_contact {
                sqlx::query(
                    r#"
                    INSERT INTO lead_contacts (
                        id, status, created_at, updated_at, submission_count
                    ) VALUES (?, 'NEW', ?, ?, 0)
                    "#,
                )
                .bind(target_contact_id)
                .bind(&now)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;

                insert_activity(
                    &mut transaction,
                    target_contact_id,
                    "LEAD_CREATED",
                    &now,
                    serde_json::json!({ "importBatchId": batch_id }),
                )
                .await?;
            }

            update_contact_from_submission(
                &mut transaction,
                target_contact_id,
                &planned.source,
                &planned.normalized,
                &now,
            )
            .await?;

            let submission_id = Uuid::new_v4().to_string();
            let raw_payload_json = serde_json::to_string(&planned.source.fields)
                .map_err(|error| AppError::ImportSchema(error.to_string()))?;

            sqlx::query(
                r#"
                INSERT INTO lead_submissions (
                    id, lead_contact_id, import_batch_id, external_lead_id,
                    source_created_at_utc, source_created_at_raw,
                    ad_id, ad_name, adset_id, adset_name,
                    campaign_id, campaign_name, form_id, form_name,
                    is_organic, platform, raw_procedure_answer, raw_product_answer,
                    raw_full_name, raw_email, raw_phone, raw_country, raw_lead_status,
                    normalized_email, normalized_phone, raw_payload_json, created_at
                ) VALUES (
                    ?, ?, ?, ?, ?, ?,
                    ?, ?, ?, ?,
                    ?, ?, ?, ?,
                    ?, ?, ?, ?,
                    ?, ?, ?, ?, ?,
                    ?, ?, ?, ?
                )
                "#,
            )
            .bind(&submission_id)
            .bind(target_contact_id)
            .bind(&batch_id)
            .bind(&planned.normalized.external_lead_id)
            .bind(&planned.normalized.created_at_utc)
            .bind(source_value(&planned.source, "created_time"))
            .bind(source_value(&planned.source, "ad_id"))
            .bind(source_value(&planned.source, "ad_name"))
            .bind(source_value(&planned.source, "adset_id"))
            .bind(source_value(&planned.source, "adset_name"))
            .bind(source_value(&planned.source, "campaign_id"))
            .bind(source_value(&planned.source, "campaign_name"))
            .bind(source_value(&planned.source, "form_id"))
            .bind(source_value(&planned.source, "form_name"))
            .bind(parse_source_bool(source_value(&planned.source, "is_organic")))
            .bind(source_value(&planned.source, "platform"))
            .bind(source_value(
                &planned.source,
                "do_you_perform_hair_transplant_procedures?",
            ))
            .bind(source_value(&planned.source, PRODUCT_INTEREST_HEADER))
            .bind(source_value(&planned.source, "full_name"))
            .bind(source_value(&planned.source, "email"))
            .bind(source_value(&planned.source, "phone_number"))
            .bind(source_value(&planned.source, "country"))
            .bind(source_value(&planned.source, "lead_status"))
            .bind(&planned.normalized.normalized_email)
            .bind(&planned.normalized.normalized_phone)
            .bind(&raw_payload_json)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;

            insert_product_interests(
                &mut transaction,
                &submission_id,
                &planned.source,
                &planned.normalized.product_interests,
                &now,
            )
            .await?;

            insert_quality_issues(
                &mut transaction,
                target_contact_id,
                &submission_id,
                planned.source.row_number,
                &planned.normalized.warnings,
                &now,
            )
            .await?;

            insert_activity(
                &mut transaction,
                target_contact_id,
                "SUBMISSION_IMPORTED",
                &now,
                serde_json::json!({
                    "importBatchId": batch_id,
                    "externalLeadId": planned.normalized.external_lead_id,
                }),
            )
            .await?;
        }

        transaction.commit().await?;

        Ok(CommitImportResult {
            batch_id,
            summary: plan.summary,
        })
    }
}

async fn update_contact_from_submission(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    contact_id: &str,
    source: &SourceRow,
    normalized: &crate::importer::normalization::NormalizedSubmission,
    now: &str,
) -> Result<(), AppError> {
    let display_name = non_empty(source_value(source, "full_name"));
    let primary_email = normalized
        .normalized_email
        .as_ref()
        .and_then(|_| non_empty(source_value(source, "email")));
    let primary_phone = normalized
        .normalized_phone
        .as_ref()
        .and_then(|_| display_phone(source_value(source, "phone_number")));
    let latest = normalized.created_at_utc.clone();

    sqlx::query(
        r#"
        UPDATE lead_contacts SET
            display_name = COALESCE(display_name, ?),
            primary_email = COALESCE(primary_email, ?),
            normalized_email = COALESCE(normalized_email, ?),
            primary_phone = COALESCE(primary_phone, ?),
            normalized_phone = COALESCE(normalized_phone, ?),
            country_code = COALESCE(country_code, ?),
            latest_submission_at = CASE
                WHEN ? IS NULL THEN latest_submission_at
                WHEN latest_submission_at IS NULL OR latest_submission_at < ? THEN ?
                ELSE latest_submission_at
            END,
            submission_count = submission_count + 1,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(display_name)
    .bind(primary_email)
    .bind(&normalized.normalized_email)
    .bind(primary_phone)
    .bind(&normalized.normalized_phone)
    .bind(&normalized.country_code)
    .bind(&latest)
    .bind(&latest)
    .bind(&latest)
    .bind(now)
    .bind(contact_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn insert_product_interests(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    submission_id: &str,
    source: &SourceRow,
    products: &[ProductCode],
    now: &str,
) -> Result<(), AppError> {
    let raw_product = source_value(source, PRODUCT_INTEREST_HEADER);
    let mode = parse_product_answer(raw_product).mode;
    let (origin, confidence) = match mode {
        ProductAnswerMode::Structured => ("DIRECT_MULTI_SELECT", "HIGH"),
        ProductAnswerMode::LegacyFreeText => ("LEGACY_NORMALIZED", "LOW"),
        ProductAnswerMode::Empty => ("LEGACY_NORMALIZED", "LOW"),
    };

    for product in products {
        sqlx::query(
            r#"
            INSERT INTO submission_product_interests (
                id, lead_submission_id, product_code, origin, confidence, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(submission_id)
        .bind(product_code_str(*product))
        .bind(origin)
        .bind(confidence)
        .bind(now)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn insert_quality_issues(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    contact_id: &str,
    submission_id: &str,
    row_number: usize,
    warnings: &[NormalizationWarning],
    now: &str,
) -> Result<(), AppError> {
    for warning in warnings {
        sqlx::query(
            r#"
            INSERT INTO lead_data_quality_issues (
                id, lead_contact_id, lead_submission_id, issue_type,
                severity, details_json, status, created_at
            ) VALUES (?, ?, ?, ?, 'WARNING', ?, 'OPEN', ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(contact_id)
        .bind(submission_id)
        .bind(warning_code(*warning))
        .bind(serde_json::json!({ "sourceRow": row_number }).to_string())
        .bind(now)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn insert_activity(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    contact_id: &str,
    activity_type: &str,
    occurred_at: &str,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO lead_activities (id, lead_contact_id, activity_type, occurred_at, payload_json) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(contact_id)
    .bind(activity_type)
    .bind(occurred_at)
    .bind(payload.to_string())
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

fn source_value<'a>(source: &'a SourceRow, header: &str) -> &'a str {
    source.get(header).unwrap_or_default()
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn display_phone(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_prefix("p:").unwrap_or(trimmed).trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn parse_source_bool(value: &str) -> Option<i64> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(1),
        "false" | "0" | "no" => Some(0),
        _ => None,
    }
}

fn product_code_str(value: ProductCode) -> &'static str {
    match value {
        ProductCode::FueMicromotorSystems => "FUE_MICROMOTOR_SYSTEMS",
        ProductCode::LongHairFueSolutions => "LONG_HAIR_FUE_SOLUTIONS",
        ProductCode::FuePunches => "FUE_PUNCHES",
        ProductCode::ImplantersForcepsSurgicalInstruments => {
            "IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS"
        }
        ProductCode::MedicalChairsClinicFurniture => "MEDICAL_CHAIRS_CLINIC_FURNITURE",
        ProductCode::OtherGeneralInformation => "OTHER_GENERAL_INFORMATION",
        ProductCode::Unknown => "UNKNOWN",
    }
}

fn warning_code(value: NormalizationWarning) -> &'static str {
    match value {
        NormalizationWarning::InvalidEmail => "INVALID_EMAIL",
        NormalizationWarning::InvalidPhone => "INVALID_PHONE",
        NormalizationWarning::InvalidCountry => "INVALID_COUNTRY",
        NormalizationWarning::InvalidTimestamp => "INVALID_TIMESTAMP",
        NormalizationWarning::MissingContactMethod => "MISSING_CONTACT_METHOD",
        NormalizationWarning::UnknownProduct => "UNKNOWN_PRODUCT",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::{SecondsFormat, Utc};

    use super::ImportCommitService;
    use crate::db::Database;
    use crate::error::AppError;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/leads_sample_multiselect_sanitized.csv")
    }

    #[tokio::test]
    async fn commit_writes_importable_rows_once_and_preserves_agency_fields_only_in_raw_payload() {
        let database = Database::connect_memory().await.expect("open database");
        let result = ImportCommitService::new(database.pool().clone())
            .commit(&fixture(), "0.1.0")
            .await
            .expect("commit fixture");

        assert_eq!(result.summary.total_rows, 6);
        assert_eq!(result.summary.new_contacts, 4);
        assert_eq!(result.summary.repeat_submissions, 1);
        assert_eq!(result.summary.exact_duplicates, 1);
        assert_eq!(result.summary.importable_submissions, 5);

        let contact_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_contacts")
            .fetch_one(database.pool())
            .await
            .expect("count contacts");
        let submission_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_submissions")
            .fetch_one(database.pool())
            .await
            .expect("count submissions");
        let product_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM submission_product_interests",
        )
        .fetch_one(database.pool())
        .await
        .expect("count products");

        assert_eq!(contact_count, 4);
        assert_eq!(submission_count, 5);
        assert_eq!(product_count, 12);

        let taylor_status: String = sqlx::query_scalar(
            "SELECT c.status FROM lead_contacts c JOIN lead_submissions s ON s.lead_contact_id = c.id WHERE s.external_lead_id = 'l:demo2004'",
        )
        .fetch_one(database.pool())
        .await
        .expect("read Taylor status");
        assert_eq!(taylor_status, "NEW");

        let raw_payload: String = sqlx::query_scalar(
            "SELECT raw_payload_json FROM lead_submissions WHERE external_lead_id = 'l:demo2004'",
        )
        .fetch_one(database.pool())
        .await
        .expect("read payload");
        let raw: serde_json::Value = serde_json::from_str(&raw_payload).expect("decode payload");
        assert_eq!(raw["Status"], "Contacted");
        assert_eq!(raw["İletişime Geçme Tarihi"], "2026-08-21 10:00");
    }

    #[tokio::test]
    async fn reimport_is_idempotent_for_submissions_and_still_records_batch_history() {
        let database = Database::connect_memory().await.expect("open database");
        let service = ImportCommitService::new(database.pool().clone());

        service
            .commit(&fixture(), "0.1.0")
            .await
            .expect("first commit");
        let second = service
            .commit(&fixture(), "0.1.0")
            .await
            .expect("second commit");

        assert_eq!(second.summary.importable_submissions, 0);
        assert_eq!(second.summary.exact_duplicates, 6);

        let submissions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_submissions")
            .fetch_one(database.pool())
            .await
            .expect("count submissions");
        let batches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM import_batches")
            .fetch_one(database.pool())
            .await
            .expect("count batches");
        assert_eq!(submissions, 5);
        assert_eq!(batches, 2);
    }

    #[tokio::test]
    async fn repeat_import_does_not_overwrite_existing_contact_status() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, normalized_email, normalized_phone, status, created_at, updated_at, submission_count) VALUES ('existing-alex', 'Existing Alex', 'alex.demo@example.test', '+351910000001', 'QUALIFIED', ?, ?, 0)",
        )
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert contact");

        ImportCommitService::new(database.pool().clone())
            .commit(&fixture(), "0.1.0")
            .await
            .expect("commit fixture");

        let status: String = sqlx::query_scalar(
            "SELECT status FROM lead_contacts WHERE id = 'existing-alex'",
        )
        .fetch_one(database.pool())
        .await
        .expect("read status");
        assert_eq!(status, "QUALIFIED");
    }

    #[tokio::test]
    async fn conflict_blocks_entire_transaction_before_batch_or_submission_writes() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        for (id, email, phone) in [
            ("email-contact", Some("alex.demo@example.test"), None),
            ("phone-contact", None, Some("+351910000001")),
        ] {
            sqlx::query(
                "INSERT INTO lead_contacts (id, normalized_email, normalized_phone, status, created_at, updated_at, submission_count) VALUES (?, ?, ?, 'NEW', ?, ?, 0)",
            )
            .bind(id)
            .bind(email)
            .bind(phone)
            .bind(&now)
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("insert conflict contact");
        }

        let error = ImportCommitService::new(database.pool().clone())
            .commit(&fixture(), "0.1.0")
            .await
            .expect_err("conflict must block");
        assert!(matches!(error, AppError::ImportBlocked(_)));

        let batches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM import_batches")
            .fetch_one(database.pool())
            .await
            .expect("count batches");
        let submissions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_submissions")
            .fetch_one(database.pool())
            .await
            .expect("count submissions");
        assert_eq!(batches, 0);
        assert_eq!(submissions, 0);
    }
}
