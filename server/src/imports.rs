use std::io::Write;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderMap, StatusCode, header::{AUTHORIZATION, COOKIE}},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use tempfile::{Builder, NamedTempFile};
use thiserror::Error;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::error;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::{AuthError, AuthService},
    authz::{Action, Actor, AuthorizationError},
    import_domain::{
        ContactIdentity, IdentityDecision, ImportDomainError, ImportPlan, ImportPlanSummary,
        NormalizationWarning, NormalizedSubmission, ProductAnswerMode, SourceFormat, SourceRow,
        SourceTable, PRODUCT_INTEREST_HEADER, build_import_plan, is_agency_ignored_crm_column,
        parse_product_answer, parse_source_file,
    },
};

const SESSION_COOKIE_NAME: &str = "elm_session";
const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;
const DEFAULT_HISTORY_LIMIT: i64 = 20;
const MAX_HISTORY_LIMIT: i64 = 100;

#[derive(Debug, Error)]
pub enum ImportServiceError {
    #[error(transparent)]
    Authorization(#[from] AuthorizationError),
    #[error("{0}")]
    Validation(String),
    #[error("import blocked: {identity_conflicts} identity conflicts, {row_errors} row errors")]
    Blocked {
        identity_conflicts: usize,
        row_errors: usize,
    },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl From<ImportDomainError> for ImportServiceError {
    fn from(error: ImportDomainError) -> Self {
        Self::Validation(error.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct UploadedImport {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewSource {
    pub file_name: String,
    pub file_size: i64,
    pub format: SourceFormat,
    pub sheet_name: Option<String>,
    pub column_count: usize,
    pub ignored_agency_columns: Vec<String>,
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
    pub summary: ImportPlanSummary,
    pub rows: Vec<ImportPreviewRow>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitImportResult {
    pub batch_id: String,
    pub summary: ImportPlanSummary,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportHistoryItem {
    pub batch_id: String,
    pub file_name: String,
    pub format: String,
    pub sheet_name: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub total_rows: i64,
    pub imported_submissions: i64,
    pub exact_duplicates: i64,
    pub repeat_submissions: i64,
    pub warning_count: i64,
    pub error_count: i64,
    pub app_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportHistoryQuery {
    limit: Option<i64>,
}

#[derive(Clone)]
pub struct ImportService {
    pool: PgPool,
}

impl ImportService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn preview(
        &self,
        actor: &Actor,
        upload: UploadedImport,
    ) -> Result<ImportPreview, ImportServiceError> {
        actor.require(Action::ImportManage)?;
        let (table, format) = parse_upload_table(&upload)?;
        let existing_external_ids = sqlx::query_scalar::<_, String>(
            "SELECT external_lead_id FROM lead_submissions",
        )
        .fetch_all(&self.pool)
        .await?;
        let contacts = load_contacts(&self.pool).await?;
        let plan = build_import_plan(&table, existing_external_ids, contacts, |normalized| {
            format!("preview:{}", normalized.external_lead_id.trim())
        });

        let ignored_agency_columns = table
            .headers
            .iter()
            .filter(|header| is_agency_ignored_crm_column(header))
            .cloned()
            .collect::<Vec<_>>();
        let rows = plan
            .rows
            .into_iter()
            .map(|planned| ImportPreviewRow {
                row_number: planned.source.row_number,
                full_name: source_value(&planned.source, "full_name").to_string(),
                raw_email: source_value(&planned.source, "email").to_string(),
                raw_phone: source_value(&planned.source, "phone_number").to_string(),
                raw_country: source_value(&planned.source, "country").to_string(),
                raw_product_answer: source_value(&planned.source, PRODUCT_INTEREST_HEADER)
                    .to_string(),
                normalized: planned.normalized,
                decision: planned.decision,
            })
            .collect();

        Ok(ImportPreview {
            source: ImportPreviewSource {
                file_name: upload.file_name,
                file_size: upload.bytes.len() as i64,
                format,
                sheet_name: table.sheet_name,
                column_count: table.headers.len(),
                ignored_agency_columns,
            },
            summary: plan.summary,
            rows,
        })
    }

    pub async fn commit(
        &self,
        actor: &Actor,
        upload: UploadedImport,
    ) -> Result<CommitImportResult, ImportServiceError> {
        actor.require(Action::ImportManage)?;
        let (table, format) = parse_upload_table(&upload)?;
        let mut transaction = self.pool.begin().await?;

        // Manual imports are serialized inside PostgreSQL so two users cannot both plan
        // against the same pre-import identity snapshot and race the unique submission ID.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('ertip_lead_manager_manual_import'))")
            .execute(&mut *transaction)
            .await?;

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
            let error = ImportServiceError::Blocked {
                identity_conflicts: plan.summary.identity_conflicts,
                row_errors: plan.summary.row_errors,
            };
            transaction.rollback().await?;
            return Err(error);
        }

        let now = Utc::now();
        let batch_id = Uuid::new_v4().to_string();
        let sheet_name = table
            .sheet_name
            .clone()
            .unwrap_or_else(|| format.as_str().to_string());
        let file_sha256 = hex::encode(Sha256::digest(&upload.bytes));

        sqlx::query(
            r#"
            INSERT INTO import_batches (
                id, file_name, file_size, file_sha256, file_format, sheet_name,
                started_at, completed_at, status, total_rows,
                new_submissions, exact_duplicates, repeat_candidates,
                warning_count, error_count, app_version
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $7, 'COMMITTED', $8,
                $9, $10, $11,
                $12, 0, $13
            )
            "#,
        )
        .bind(&batch_id)
        .bind(&upload.file_name)
        .bind(upload.bytes.len() as i64)
        .bind(&file_sha256)
        .bind(format.as_str())
        .bind(&sheet_name)
        .bind(now)
        .bind(plan.summary.total_rows as i32)
        .bind(plan.summary.importable_submissions as i32)
        .bind(plan.summary.exact_duplicates as i32)
        .bind(plan.summary.repeat_submissions as i32)
        .bind(plan.summary.warning_count as i32)
        .bind(env!("CARGO_PKG_VERSION"))
        .execute(&mut *transaction)
        .await?;

        persist_plan(&mut transaction, actor, &batch_id, &plan, now).await?;
        transaction.commit().await?;

        Ok(CommitImportResult {
            batch_id,
            summary: plan.summary,
        })
    }

    pub async fn history(
        &self,
        actor: &Actor,
        limit: i64,
    ) -> Result<Vec<ImportHistoryItem>, ImportServiceError> {
        actor.require(Action::ImportManage)?;
        let limit = limit.clamp(1, MAX_HISTORY_LIMIT);
        let rows = sqlx::query(
            r#"
            SELECT id, file_name, file_format, sheet_name, completed_at, status,
                   total_rows, new_submissions, exact_duplicates, repeat_candidates,
                   warning_count, error_count, app_version
            FROM import_batches
            ORDER BY COALESCE(completed_at, started_at) DESC, id DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let completed_at: Option<DateTime<Utc>> = row.try_get("completed_at")?;
                Ok(ImportHistoryItem {
                    batch_id: row.try_get("id")?,
                    file_name: row.try_get("file_name")?,
                    format: row.try_get("file_format")?,
                    sheet_name: row.try_get("sheet_name")?,
                    completed_at: completed_at.map(format_utc),
                    status: row.try_get("status")?,
                    total_rows: row.try_get::<i32, _>("total_rows")? as i64,
                    imported_submissions: row.try_get::<i32, _>("new_submissions")? as i64,
                    exact_duplicates: row.try_get::<i32, _>("exact_duplicates")? as i64,
                    repeat_submissions: row.try_get::<i32, _>("repeat_candidates")? as i64,
                    warning_count: row.try_get::<i32, _>("warning_count")? as i64,
                    error_count: row.try_get::<i32, _>("error_count")? as i64,
                    app_version: row.try_get("app_version")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(ImportServiceError::Database)
    }
}

async fn load_contacts(pool: &PgPool) -> Result<Vec<ContactIdentity>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, normalized_email, normalized_phone FROM lead_contacts")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| ContactIdentity {
            contact_id: row.get("id"),
            normalized_email: row.get("normalized_email"),
            normalized_phone: row.get("normalized_phone"),
        })
        .collect())
}

fn parse_upload_table(
    upload: &UploadedImport,
) -> Result<(SourceTable, SourceFormat), ImportServiceError> {
    if upload.bytes.is_empty() {
        return Err(ImportServiceError::Validation(
            "import file must not be empty".to_string(),
        ));
    }
    if upload.bytes.len() > MAX_UPLOAD_BYTES {
        return Err(ImportServiceError::Validation(format!(
            "import file exceeds {} MiB limit",
            MAX_UPLOAD_BYTES / 1024 / 1024
        )));
    }
    let format = SourceFormat::from_file_name(&upload.file_name)?;
    let suffix = match format {
        SourceFormat::Csv => ".csv",
        SourceFormat::Xlsx => ".xlsx",
    };
    let mut temp = Builder::new()
        .prefix("ertip-lead-import-")
        .suffix(suffix)
        .tempfile()
        .map_err(|error| ImportServiceError::Validation(error.to_string()))?;
    temp.write_all(&upload.bytes)
        .map_err(|error| ImportServiceError::Validation(error.to_string()))?;
    temp.flush()
        .map_err(|error| ImportServiceError::Validation(error.to_string()))?;
    let table = parse_source_file(temp.path(), &upload.file_name, format)?;
    Ok((table, format))
}

async fn persist_plan(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    batch_id: &str,
    plan: &ImportPlan,
    now: DateTime<Utc>,
) -> Result<(), ImportServiceError> {
    for planned in &plan.rows {
        if matches!(
            planned.decision,
            IdentityDecision::ExactDuplicateSubmission { .. }
        ) {
            continue;
        }

        let target_contact_id = planned.target_contact_id.as_ref().ok_or_else(|| {
            ImportServiceError::Validation("importable row has no target contact".to_string())
        })?;
        let is_new_contact = matches!(planned.decision, IdentityDecision::NewContact);
        if is_new_contact {
            sqlx::query(
                r#"
                INSERT INTO lead_contacts (
                    id, status, revision, created_at, updated_at, submission_count
                ) VALUES ($1, 'NEW', 0, $2, $2, 0)
                "#,
            )
            .bind(target_contact_id)
            .bind(now)
            .execute(&mut **transaction)
            .await?;
            insert_activity(
                transaction,
                actor,
                target_contact_id,
                "LEAD_CREATED",
                now,
                serde_json::json!({ "importBatchId": batch_id }),
            )
            .await?;
        }

        update_contact_from_submission(
            transaction,
            target_contact_id,
            &planned.source,
            &planned.normalized,
            now,
        )
        .await?;

        let submission_id = Uuid::new_v4().to_string();
        let raw_payload_json = serde_json::to_string(&planned.source.fields)
            .map_err(|error| ImportServiceError::Validation(error.to_string()))?;
        let source_created = parse_normalized_timestamp(planned.normalized.created_at_utc.as_deref());

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
                $1, $2, $3, $4,
                $5, $6,
                $7, $8, $9, $10,
                $11, $12, $13, $14,
                $15, $16, $17, $18,
                $19, $20, $21, $22, $23,
                $24, $25, $26, $27
            )
            "#,
        )
        .bind(&submission_id)
        .bind(target_contact_id)
        .bind(batch_id)
        .bind(&planned.normalized.external_lead_id)
        .bind(source_created)
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
        .bind(now)
        .execute(&mut **transaction)
        .await?;

        insert_product_interests(
            transaction,
            &submission_id,
            &planned.source,
            &planned.normalized.product_interests,
            now,
        )
        .await?;
        insert_quality_issues(
            transaction,
            target_contact_id,
            &submission_id,
            planned.source.row_number,
            &planned.normalized.warnings,
            now,
        )
        .await?;
        insert_activity(
            transaction,
            actor,
            target_contact_id,
            "SUBMISSION_IMPORTED",
            now,
            serde_json::json!({
                "importBatchId": batch_id,
                "externalLeadId": planned.normalized.external_lead_id,
            }),
        )
        .await?;
    }
    Ok(())
}

async fn update_contact_from_submission(
    transaction: &mut Transaction<'_, Postgres>,
    contact_id: &str,
    source: &SourceRow,
    normalized: &NormalizedSubmission,
    now: DateTime<Utc>,
) -> Result<(), ImportServiceError> {
    let display_name = non_empty(source_value(source, "full_name"));
    let primary_email = normalized
        .normalized_email
        .as_ref()
        .and_then(|_| non_empty(source_value(source, "email")));
    let primary_phone = normalized
        .normalized_phone
        .as_ref()
        .and_then(|_| display_phone(source_value(source, "phone_number")));
    let latest = parse_normalized_timestamp(normalized.created_at_utc.as_deref());

    sqlx::query(
        r#"
        UPDATE lead_contacts SET
            display_name = COALESCE(display_name, $1),
            primary_email = COALESCE(primary_email, $2),
            normalized_email = COALESCE(normalized_email, $3),
            primary_phone = COALESCE(primary_phone, $4),
            normalized_phone = COALESCE(normalized_phone, $5),
            country_code = COALESCE(country_code, $6),
            latest_submission_at = CASE
                WHEN $7::timestamptz IS NULL THEN latest_submission_at
                WHEN latest_submission_at IS NULL OR latest_submission_at < $7 THEN $7
                ELSE latest_submission_at
            END,
            submission_count = submission_count + 1,
            revision = revision + 1,
            updated_at = $8
        WHERE id = $9
        "#,
    )
    .bind(display_name)
    .bind(primary_email)
    .bind(&normalized.normalized_email)
    .bind(primary_phone)
    .bind(&normalized.normalized_phone)
    .bind(&normalized.country_code)
    .bind(latest)
    .bind(now)
    .bind(contact_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_product_interests(
    transaction: &mut Transaction<'_, Postgres>,
    submission_id: &str,
    source: &SourceRow,
    products: &[crate::import_domain::ProductCode],
    now: DateTime<Utc>,
) -> Result<(), ImportServiceError> {
    let mode = parse_product_answer(source_value(source, PRODUCT_INTEREST_HEADER)).mode;
    let (origin, confidence) = match mode {
        ProductAnswerMode::Structured => ("DIRECT_MULTI_SELECT", "HIGH"),
        ProductAnswerMode::LegacyFreeText | ProductAnswerMode::Empty => {
            ("LEGACY_NORMALIZED", "LOW")
        }
    };
    for product in products {
        sqlx::query(
            r#"
            INSERT INTO submission_product_interests (
                id, lead_submission_id, product_code, origin, confidence, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(submission_id)
        .bind(product.as_str())
        .bind(origin)
        .bind(confidence)
        .bind(now)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn insert_quality_issues(
    transaction: &mut Transaction<'_, Postgres>,
    contact_id: &str,
    submission_id: &str,
    row_number: usize,
    warnings: &[NormalizationWarning],
    now: DateTime<Utc>,
) -> Result<(), ImportServiceError> {
    for warning in warnings {
        sqlx::query(
            r#"
            INSERT INTO lead_data_quality_issues (
                id, lead_contact_id, lead_submission_id, issue_type,
                severity, details_json, status, created_at
            ) VALUES ($1, $2, $3, $4, 'WARNING', $5, 'OPEN', $6)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(contact_id)
        .bind(submission_id)
        .bind(warning.as_str())
        .bind(serde_json::json!({ "sourceRow": row_number }).to_string())
        .bind(now)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn insert_activity(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    contact_id: &str,
    activity_type: &str,
    occurred_at: DateTime<Utc>,
    payload: serde_json::Value,
) -> Result<(), ImportServiceError> {
    sqlx::query(
        r#"
        INSERT INTO lead_activities (
            id, lead_contact_id, actor_user_id, activity_type, occurred_at, payload_json
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(contact_id)
    .bind(&actor.user_id)
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

fn parse_source_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn parse_normalized_timestamp(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

fn format_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: ApiErrorBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug)]
struct ApiHttpError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiHttpError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorResponse {
                error: ApiErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

impl From<AuthError> for ApiHttpError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unauthorized => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Oturum gerekli veya oturum süresi dolmuş.",
            ),
            AuthError::InvalidCredentials => Self::new(
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "E-posta veya parola geçersiz.",
            ),
            AuthError::TemporarilyLocked => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "LOGIN_TEMPORARILY_LOCKED",
                "Çok sayıda başarısız deneme nedeniyle giriş geçici olarak kilitlendi.",
            ),
            other => {
                error!(error = %other, "import authentication operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTH_INTERNAL_ERROR",
                    "Kimlik doğrulama işlemi tamamlanamadı.",
                )
            }
        }
    }
}

impl From<ImportServiceError> for ApiHttpError {
    fn from(error: ImportServiceError) -> Self {
        match error {
            ImportServiceError::Authorization(AuthorizationError::Forbidden) => Self::new(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Bu işlem için yetkiniz yok.",
            ),
            ImportServiceError::Authorization(AuthorizationError::InvalidRole(role)) => {
                error!(persisted_role = %role, "unsupported persisted authorization role");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTHORIZATION_INTERNAL_ERROR",
                    "Yetkilendirme işlemi tamamlanamadı.",
                )
            }
            ImportServiceError::Validation(message) => Self::new(
                StatusCode::BAD_REQUEST,
                "IMPORT_VALIDATION_ERROR",
                message,
            ),
            ImportServiceError::Blocked {
                identity_conflicts,
                row_errors,
            } => Self::new(
                StatusCode::CONFLICT,
                "IMPORT_BLOCKED",
                format!(
                    "Import yeniden doğrulamada bloklandı: {identity_conflicts} identity conflict, {row_errors} row error."
                ),
            ),
            ImportServiceError::Database(database_error) => {
                error!(error = %database_error, "manual import operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "IMPORT_INTERNAL_ERROR",
                    "Import işlemi tamamlanamadı.",
                )
            }
        }
    }
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");
    Router::new()
        .route("/api/v1/imports/preview", post(preview_import))
        .route("/api/v1/imports/commit", post(commit_import))
        .route("/api/v1/imports/history", get(import_history))
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES + 1024 * 1024))
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn preview_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let upload = read_upload(multipart).await?;
    let response = ImportService::new(state.pool.clone())
        .preview(&actor, upload)
        .await?;
    Ok(Json(response).into_response())
}

async fn commit_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let upload = read_upload(multipart).await?;
    let response = ImportService::new(state.pool.clone())
        .commit(&actor, upload)
        .await?;
    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn import_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ImportHistoryQuery>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let response = ImportService::new(state.pool.clone())
        .history(&actor, query.limit.unwrap_or(DEFAULT_HISTORY_LIMIT))
        .await?;
    Ok(Json(response).into_response())
}

async fn read_upload(mut multipart: Multipart) -> Result<UploadedImport, ApiHttpError> {
    let mut upload = None;
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiHttpError::new(
            StatusCode::BAD_REQUEST,
            "IMPORT_MULTIPART_ERROR",
            error.to_string(),
        )
    })? {
        if field.name() != Some("file") {
            continue;
        }
        if upload.is_some() {
            return Err(ApiHttpError::new(
                StatusCode::BAD_REQUEST,
                "IMPORT_MULTIPART_ERROR",
                "Tek bir import dosyası gönderilmelidir.",
            ));
        }
        let file_name = field
            .file_name()
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                ApiHttpError::new(
                    StatusCode::BAD_REQUEST,
                    "IMPORT_MULTIPART_ERROR",
                    "Import dosya adı eksik.",
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiHttpError::new(
                StatusCode::BAD_REQUEST,
                "IMPORT_MULTIPART_ERROR",
                error.to_string(),
            )
        })?;
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(ApiHttpError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "IMPORT_FILE_TOO_LARGE",
                "Import dosyası 20 MiB sınırını aşıyor.",
            ));
        }
        upload = Some(UploadedImport {
            file_name,
            bytes: bytes.to_vec(),
        });
    }
    upload.ok_or_else(|| {
        ApiHttpError::new(
            StatusCode::BAD_REQUEST,
            "IMPORT_MULTIPART_ERROR",
            "multipart form içinde 'file' alanı bulunamadı.",
        )
    })
}

async fn authenticated_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Actor, ApiHttpError> {
    let token = session_token_from_headers(headers).ok_or(AuthError::Unauthorized)?;
    let session = AuthService::new(state.pool.clone(), state.session_ttl_hours)
        .resolve(&token)
        .await?;
    Actor::from_auth_user(&session.user)
        .map_err(|error| ApiHttpError::from(ImportServiceError::Authorization(error)))
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(raw) = headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        if let Some(token) = raw.strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE_NAME && !value.is_empty()).then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::{ImportService, UploadedImport};
    use crate::{
        authz::{Actor, Role},
        db::run_migrations,
    };

    fn fixture_upload() -> UploadedImport {
        let csv = concat!(
            "id,created_time,full_name,email,phone_number,country,which_product_would_you_like_to_receive_more_information_about?,Status,İletişime Geçme Tarihi\n",
            "m6:import:1,2026-08-20T10:00:00+03:00,M6 Import Alpha,alpha.m6@example.test,p:+905551111111,TR,fue_punches,Contacted,2026-08-21 10:00\n",
            "m6:import:2,2026-08-20T11:00:00+03:00,M6 Import Beta,beta.m6@example.test,p:+905552222222,GB,fue_punches,New,\n",
            "m6:import:3,2026-08-20T12:00:00+03:00,M6 Import Gamma,gamma.m6@example.test,p:+905553333333,DE,fue_punches,New,\n",
            "m6:import:4,2026-08-20T13:00:00+03:00,M6 Import Delta,delta.m6@example.test,p:+905554444444,FR,fue_punches,New,\n",
            "m6:import:5,2026-08-20T14:00:00+03:00,M6 Import Alpha Repeat,alpha.m6@example.test,p:+905551111111,TR,fue_punches,Contacted,\n",
            "m6:import:2,2026-08-20T15:00:00+03:00,M6 Import Duplicate,beta.m6@example.test,p:+905552222222,GB,fue_punches,Contacted,\n"
        );
        UploadedImport {
            file_name: "m6-import-fixture.csv".to_string(),
            bytes: csv.as_bytes().to_vec(),
        }
    }

    async fn cleanup(pool: &sqlx::PgPool) {
        sqlx::query("DELETE FROM lead_data_quality_issues WHERE lead_submission_id IN (SELECT id FROM lead_submissions WHERE external_lead_id LIKE 'm6:import:%')")
            .execute(pool).await.expect("cleanup quality");
        sqlx::query("DELETE FROM lead_activities WHERE lead_contact_id IN (SELECT id FROM lead_contacts WHERE display_name LIKE 'M6 Import%')")
            .execute(pool).await.expect("cleanup activities");
        sqlx::query("DELETE FROM lead_submissions WHERE external_lead_id LIKE 'm6:import:%'")
            .execute(pool).await.expect("cleanup submissions");
        sqlx::query("DELETE FROM import_batches WHERE file_name = 'm6-import-fixture.csv'")
            .execute(pool).await.expect("cleanup batches");
        sqlx::query("DELETE FROM lead_contacts WHERE display_name LIKE 'M6 Import%'")
            .execute(pool).await.expect("cleanup contacts");
    }

    #[tokio::test]
    async fn postgres_manual_import_preserves_preview_commit_idempotency_raw_and_actor_semantics() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping import integration test");
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect test postgres");
        run_migrations(&pool).await.expect("migrations");
        cleanup(&pool).await;

        let manager_id = "m6-import-manager";
        sqlx::query("DELETE FROM auth_credentials WHERE app_user_id = $1")
            .bind(manager_id).execute(&pool).await.expect("cleanup credential");
        sqlx::query("DELETE FROM app_users WHERE id = $1")
            .bind(manager_id).execute(&pool).await.expect("cleanup manager");
        sqlx::query("INSERT INTO app_users (id, display_name, role, is_active, revision, created_at, updated_at) VALUES ($1, 'M6 Import Manager', 'MANAGER', TRUE, 0, now(), now())")
            .bind(manager_id).execute(&pool).await.expect("seed manager");

        let manager = Actor { user_id: manager_id.to_string(), role: Role::Manager };
        let sales = Actor { user_id: "m6-import-sales".to_string(), role: Role::Sales };
        let service = ImportService::new(pool.clone());

        let forbidden = service.preview(&sales, fixture_upload()).await.expect_err("sales import forbidden");
        assert!(matches!(forbidden, super::ImportServiceError::Authorization(_)));

        let preview = service.preview(&manager, fixture_upload()).await.expect("preview");
        assert_eq!(preview.summary.total_rows, 6);
        assert_eq!(preview.summary.new_contacts, 4);
        assert_eq!(preview.summary.repeat_submissions, 1);
        assert_eq!(preview.summary.exact_duplicates, 1);
        assert_eq!(preview.summary.importable_submissions, 5);
        assert_eq!(preview.source.ignored_agency_columns, vec!["Status", "İletişime Geçme Tarihi"]);

        let first = service.commit(&manager, fixture_upload()).await.expect("first commit");
        assert_eq!(first.summary.importable_submissions, 5);
        assert_eq!(first.summary.new_contacts, 4);

        let submission_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_submissions WHERE external_lead_id LIKE 'm6:import:%'")
            .fetch_one(&pool).await.expect("count submissions");
        assert_eq!(submission_count, 5);

        let raw_payload: String = sqlx::query_scalar("SELECT raw_payload_json FROM lead_submissions WHERE external_lead_id = 'm6:import:1'")
            .fetch_one(&pool).await.expect("raw payload");
        let raw: serde_json::Value = serde_json::from_str(&raw_payload).expect("decode payload");
        assert_eq!(raw["Status"], "Contacted");
        assert_eq!(raw["İletişime Geçme Tarihi"], "2026-08-21 10:00");

        let contact_id: String = sqlx::query_scalar("SELECT lead_contact_id FROM lead_submissions WHERE external_lead_id = 'm6:import:1'")
            .fetch_one(&pool).await.expect("contact id");
        sqlx::query("UPDATE lead_contacts SET status = 'QUALIFIED' WHERE id = $1")
            .bind(&contact_id).execute(&pool).await.expect("set crm status");

        let second = service.commit(&manager, fixture_upload()).await.expect("second commit");
        assert_eq!(second.summary.importable_submissions, 0);
        assert_eq!(second.summary.exact_duplicates, 6);
        let status: String = sqlx::query_scalar("SELECT status FROM lead_contacts WHERE id = $1")
            .bind(&contact_id).fetch_one(&pool).await.expect("status");
        assert_eq!(status, "QUALIFIED");

        let batch_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM import_batches WHERE file_name = 'm6-import-fixture.csv'")
            .fetch_one(&pool).await.expect("batch count");
        assert_eq!(batch_count, 2);
        let actor_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_activities WHERE actor_user_id = $1 AND activity_type IN ('LEAD_CREATED', 'SUBMISSION_IMPORTED')")
            .bind(manager_id).fetch_one(&pool).await.expect("actor events");
        assert_eq!(actor_events, 9);

        let history = service.history(&manager, 10).await.expect("history");
        assert!(history.iter().filter(|item| item.file_name == "m6-import-fixture.csv").count() >= 2);

        cleanup(&pool).await;
        sqlx::query("DELETE FROM app_users WHERE id = $1")
            .bind(manager_id).execute(&pool).await.expect("cleanup manager");
    }
}
