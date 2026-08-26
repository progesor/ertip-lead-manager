use std::{collections::{BTreeMap, BTreeSet}, path::Path, str::FromStr};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, SqlitePool, Transaction, sqlite::{SqliteConnectOptions, SqlitePoolOptions}};
use thiserror::Error;

const SOURCE_SCHEMA_VERSION: i64 = 4;
const TARGET_MIN_SCHEMA_VERSION: i64 = 3;

const DOMAIN_TABLES: &[&str] = &[
    "lead_contacts",
    "import_batches",
    "lead_submissions",
    "submission_product_interests",
    "contact_product_interest_overrides",
    "lead_notes",
    "lead_activities",
    "follow_ups",
    "lead_data_quality_issues",
];

const ID_TABLES: &[&str] = &[
    "lead_contacts",
    "import_batches",
    "lead_submissions",
    "submission_product_interests",
    "contact_product_interest_overrides",
    "lead_notes",
    "lead_activities",
    "follow_ups",
    "lead_data_quality_issues",
];

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("source SQLite file does not exist")]
    SourceNotFound,
    #[error("unsupported source schema version {0}; expected v4")]
    UnsupportedSourceVersion(i64),
    #[error("target PostgreSQL schema version {0} is older than required")]
    TargetSchemaTooOld(i64),
    #[error("source SQLite foreign-key check failed with {0} violation(s)")]
    SourceForeignKeyViolation(usize),
    #[error("target PostgreSQL CRM tables must be empty before migration; {table} has {count} row(s)")]
    TargetNotEmpty { table: String, count: i64 },
    #[error("target app_users collision for source user {0}")]
    UserCollision(String),
    #[error("invalid timestamp in {field}: {value}")]
    InvalidTimestamp { field: String, value: String },
    #[error("reconciliation failed: {0}")]
    Reconciliation(String),
    #[error("SQLite error")]
    Sqlite(#[source] sqlx::Error),
    #[error("PostgreSQL error")]
    Postgres(#[source] sqlx::Error),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TableCountCheck {
    pub table: String,
    pub source_count: i64,
    pub target_count: i64,
    pub matches: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DigestCheck {
    pub source_sha256: String,
    pub target_sha256: String,
    pub matches: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub source_schema_version: i64,
    pub target_schema_version: i64,
    pub source_user_count: i64,
    pub table_counts: Vec<TableCountCheck>,
    pub migrated_user_ids: DigestCheck,
    pub domain_stable_ids: DigestCheck,
    pub raw_submission_payloads: DigestCheck,
    pub activity_audit: DigestCheck,
    pub assignments: DigestCheck,
    pub all_checks_passed: bool,
}

pub async fn migrate_sqlite_v4(
    sqlite_path: &Path,
    target: &PgPool,
) -> Result<MigrationReport, MigrationError> {
    if !sqlite_path.is_file() {
        return Err(MigrationError::SourceNotFound);
    }

    let source = open_source(sqlite_path).await?;
    let source_version = source_schema_version(&source).await?;
    if source_version != SOURCE_SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedSourceVersion(source_version));
    }

    let fk_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&source)
        .await
        .map_err(MigrationError::Sqlite)?;
    if !fk_violations.is_empty() {
        return Err(MigrationError::SourceForeignKeyViolation(fk_violations.len()));
    }

    let target_version = target_schema_version(target).await?;
    if target_version < TARGET_MIN_SCHEMA_VERSION {
        return Err(MigrationError::TargetSchemaTooOld(target_version));
    }

    let source_counts = source_counts(&source).await?;
    let source_user_ids = source_user_ids(&source).await?;
    let source_user_count = source_user_ids.len() as i64;

    let source_user_digest = digest_values(source_user_ids.iter().cloned());
    let source_domain_ids = collect_sqlite_domain_ids(&source).await?;
    let source_domain_digest = digest_values(source_domain_ids);
    let source_raw_digest = digest_sqlite_raw_payloads(&source).await?;
    let source_activity_digest = digest_sqlite_activities(&source).await?;
    let source_assignment_digest = digest_sqlite_assignments(&source).await?;

    let mut tx = target.begin().await.map_err(MigrationError::Postgres)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('ertip_lead_manager_sqlite_v4_migration'))")
        .execute(&mut *tx)
        .await
        .map_err(MigrationError::Postgres)?;

    lock_and_require_empty_target(&mut tx).await?;
    require_no_user_collisions(&source, &mut tx).await?;

    migrate_users(&source, &mut tx).await?;
    migrate_contacts(&source, &mut tx).await?;
    migrate_batches(&source, &mut tx).await?;
    migrate_submissions(&source, &mut tx).await?;
    migrate_submission_products(&source, &mut tx).await?;
    migrate_product_overrides(&source, &mut tx).await?;
    migrate_notes(&source, &mut tx).await?;
    migrate_activities(&source, &mut tx).await?;
    migrate_followups(&source, &mut tx).await?;
    migrate_quality_issues(&source, &mut tx).await?;

    tx.commit().await.map_err(MigrationError::Postgres)?;

    let target_counts = target_counts(target).await?;
    let table_counts = DOMAIN_TABLES
        .iter()
        .map(|table| {
            let source_count = *source_counts.get(*table).unwrap_or(&0);
            let target_count = *target_counts.get(*table).unwrap_or(&0);
            TableCountCheck {
                table: (*table).to_string(),
                source_count,
                target_count,
                matches: source_count == target_count,
            }
        })
        .collect::<Vec<_>>();

    let target_user_digest = digest_target_source_users(target, &source_user_ids).await?;
    let target_domain_digest = digest_values(collect_postgres_domain_ids(target).await?);
    let target_raw_digest = digest_postgres_raw_payloads(target).await?;
    let target_activity_digest = digest_postgres_activities(target).await?;
    let target_assignment_digest = digest_postgres_assignments(target).await?;

    let migrated_user_ids = digest_check(source_user_digest, target_user_digest);
    let domain_stable_ids = digest_check(source_domain_digest, target_domain_digest);
    let raw_submission_payloads = digest_check(source_raw_digest, target_raw_digest);
    let activity_audit = digest_check(source_activity_digest, target_activity_digest);
    let assignments = digest_check(source_assignment_digest, target_assignment_digest);

    let all_checks_passed = table_counts.iter().all(|check| check.matches)
        && migrated_user_ids.matches
        && domain_stable_ids.matches
        && raw_submission_payloads.matches
        && activity_audit.matches
        && assignments.matches;

    if !all_checks_passed {
        return Err(MigrationError::Reconciliation(
            "one or more count/key/hash checks did not match".to_string(),
        ));
    }

    Ok(MigrationReport {
        source_schema_version: source_version,
        target_schema_version: target_version,
        source_user_count,
        table_counts,
        migrated_user_ids,
        domain_stable_ids,
        raw_submission_payloads,
        activity_audit,
        assignments,
        all_checks_passed,
    })
}

async fn open_source(path: &Path) -> Result<SqlitePool, MigrationError> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(MigrationError::Sqlite)?
        .read_only(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(MigrationError::Sqlite)
}

async fn source_schema_version(source: &SqlitePool) -> Result<i64, MigrationError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
    )
    .fetch_one(source)
    .await
    .map_err(MigrationError::Sqlite)
}

async fn target_schema_version(target: &PgPool) -> Result<i64, MigrationError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = TRUE",
    )
    .fetch_one(target)
    .await
    .map_err(MigrationError::Postgres)
}

async fn source_counts(source: &SqlitePool) -> Result<BTreeMap<String, i64>, MigrationError> {
    let mut counts = BTreeMap::new();
    for table in DOMAIN_TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count = sqlx::query_scalar::<_, i64>(&sql)
            .fetch_one(source)
            .await
            .map_err(MigrationError::Sqlite)?;
        counts.insert((*table).to_string(), count);
    }
    Ok(counts)
}

async fn target_counts(target: &PgPool) -> Result<BTreeMap<String, i64>, MigrationError> {
    let mut counts = BTreeMap::new();
    for table in DOMAIN_TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count = sqlx::query_scalar::<_, i64>(&sql)
            .fetch_one(target)
            .await
            .map_err(MigrationError::Postgres)?;
        counts.insert((*table).to_string(), count);
    }
    Ok(counts)
}

async fn lock_and_require_empty_target(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), MigrationError> {
    sqlx::query(
        "LOCK TABLE lead_data_quality_issues, follow_ups, lead_notes, contact_product_interest_overrides, submission_product_interests, lead_activities, lead_submissions, import_batches, lead_contacts IN ACCESS EXCLUSIVE MODE",
    )
    .execute(&mut **tx)
    .await
    .map_err(MigrationError::Postgres)?;

    for table in DOMAIN_TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count = sqlx::query_scalar::<_, i64>(&sql)
            .fetch_one(&mut **tx)
            .await
            .map_err(MigrationError::Postgres)?;
        if count != 0 {
            return Err(MigrationError::TargetNotEmpty {
                table: (*table).to_string(),
                count,
            });
        }
    }
    Ok(())
}

async fn require_no_user_collisions(
    source: &SqlitePool,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, email, auth_subject FROM app_users ORDER BY id")
        .fetch_all(source)
        .await
        .map_err(MigrationError::Sqlite)?;

    for row in rows {
        let id: String = row.try_get("id").map_err(MigrationError::Sqlite)?;
        let email: Option<String> = row.try_get("email").map_err(MigrationError::Sqlite)?;
        let auth_subject: Option<String> = row.try_get("auth_subject").map_err(MigrationError::Sqlite)?;
        let collision = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM app_users
            WHERE id = $1
               OR ($2::text IS NOT NULL AND email IS NOT NULL AND lower(email) = lower($2))
               OR ($3::text IS NOT NULL AND auth_subject = $3)
            "#,
        )
        .bind(&id)
        .bind(email.as_deref())
        .bind(auth_subject.as_deref())
        .fetch_one(&mut **tx)
        .await
        .map_err(MigrationError::Postgres)?;
        if collision != 0 {
            return Err(MigrationError::UserCollision(id));
        }
    }
    Ok(())
}

async fn migrate_users(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, display_name, email, role, is_active, auth_subject, created_at, updated_at FROM app_users ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        let updated_raw: String = row.try_get("updated_at").map_err(MigrationError::Sqlite)?;
        sqlx::query(
            "INSERT INTO app_users (id, display_name, email, role, is_active, auth_subject, revision, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,0,$7,$8)",
        )
        .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("display_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("email").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("role").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<i64,_>("is_active").map_err(MigrationError::Sqlite)? != 0)
        .bind(row.try_get::<Option<String>,_>("auth_subject").map_err(MigrationError::Sqlite)?)
        .bind(parse_ts(&created_raw, "app_users.created_at")?)
        .bind(parse_ts(&updated_raw, "app_users.updated_at")?)
        .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_contacts(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, display_name, primary_email, normalized_email, primary_phone, normalized_phone, country_code, status, assigned_user_id, created_at, updated_at, latest_submission_at, submission_count FROM lead_contacts ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        let updated_raw: String = row.try_get("updated_at").map_err(MigrationError::Sqlite)?;
        let latest_raw: Option<String> = row.try_get("latest_submission_at").map_err(MigrationError::Sqlite)?;
        sqlx::query(
            r#"INSERT INTO lead_contacts (
                id, display_name, primary_email, normalized_email, primary_phone, normalized_phone,
                country_code, status, assigned_user_id, revision, created_at, updated_at,
                latest_submission_at, submission_count
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,0,$10,$11,$12,$13)"#,
        )
        .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("display_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("primary_email").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("normalized_email").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("primary_phone").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("normalized_phone").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("country_code").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("status").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("assigned_user_id").map_err(MigrationError::Sqlite)?)
        .bind(parse_ts(&created_raw, "lead_contacts.created_at")?)
        .bind(parse_ts(&updated_raw, "lead_contacts.updated_at")?)
        .bind(parse_ts_opt(latest_raw.as_deref(), "lead_contacts.latest_submission_at")?)
        .bind(row.try_get::<i64,_>("submission_count").map_err(MigrationError::Sqlite)? as i32)
        .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_batches(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, file_name, file_size, file_sha256, file_format, sheet_name, started_at, completed_at, status, total_rows, new_submissions, exact_duplicates, repeat_candidates, warning_count, error_count, app_version FROM import_batches ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let started_raw: String = row.try_get("started_at").map_err(MigrationError::Sqlite)?;
        let completed_raw: Option<String> = row.try_get("completed_at").map_err(MigrationError::Sqlite)?;
        sqlx::query(
            r#"INSERT INTO import_batches (
                id,file_name,file_size,file_sha256,file_format,sheet_name,started_at,completed_at,status,
                total_rows,new_submissions,exact_duplicates,repeat_candidates,warning_count,error_count,app_version
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)"#,
        )
        .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("file_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<i64>,_>("file_size").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("file_sha256").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("file_format").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("sheet_name").map_err(MigrationError::Sqlite)?)
        .bind(parse_ts(&started_raw, "import_batches.started_at")?)
        .bind(parse_ts_opt(completed_raw.as_deref(), "import_batches.completed_at")?)
        .bind(row.try_get::<String,_>("status").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<i64,_>("total_rows").map_err(MigrationError::Sqlite)? as i32)
        .bind(row.try_get::<i64,_>("new_submissions").map_err(MigrationError::Sqlite)? as i32)
        .bind(row.try_get::<i64,_>("exact_duplicates").map_err(MigrationError::Sqlite)? as i32)
        .bind(row.try_get::<i64,_>("repeat_candidates").map_err(MigrationError::Sqlite)? as i32)
        .bind(row.try_get::<i64,_>("warning_count").map_err(MigrationError::Sqlite)? as i32)
        .bind(row.try_get::<i64,_>("error_count").map_err(MigrationError::Sqlite)? as i32)
        .bind(row.try_get::<String,_>("app_version").map_err(MigrationError::Sqlite)?)
        .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_submissions(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT * FROM lead_submissions ORDER BY id").fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let source_created_raw: Option<String> = row.try_get("source_created_at_utc").map_err(MigrationError::Sqlite)?;
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        sqlx::query(
            r#"INSERT INTO lead_submissions (
                id,lead_contact_id,import_batch_id,external_lead_id,source_created_at_utc,source_created_at_raw,
                ad_id,ad_name,adset_id,adset_name,campaign_id,campaign_name,form_id,form_name,is_organic,platform,
                raw_procedure_answer,raw_product_answer,raw_full_name,raw_email,raw_phone,raw_country,raw_lead_status,
                normalized_email,normalized_phone,raw_payload_json,created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27)"#,
        )
        .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("lead_contact_id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("import_batch_id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("external_lead_id").map_err(MigrationError::Sqlite)?)
        .bind(parse_ts_opt(source_created_raw.as_deref(), "lead_submissions.source_created_at_utc")?)
        .bind(row.try_get::<String,_>("source_created_at_raw").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("ad_id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("ad_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("adset_id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("adset_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("campaign_id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("campaign_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("form_id").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("form_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<i64>,_>("is_organic").map_err(MigrationError::Sqlite)?.map(|v| v != 0))
        .bind(row.try_get::<Option<String>,_>("platform").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_procedure_answer").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_product_answer").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_full_name").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_email").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_phone").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_country").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("raw_lead_status").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("normalized_email").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<Option<String>,_>("normalized_phone").map_err(MigrationError::Sqlite)?)
        .bind(row.try_get::<String,_>("raw_payload_json").map_err(MigrationError::Sqlite)?)
        .bind(parse_ts(&created_raw, "lead_submissions.created_at")?)
        .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_submission_products(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, lead_submission_id, product_code, origin, confidence, created_at FROM submission_product_interests ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        sqlx::query("INSERT INTO submission_product_interests (id,lead_submission_id,product_code,origin,confidence,created_at) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("lead_submission_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("product_code").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("origin").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<Option<String>,_>("confidence").map_err(MigrationError::Sqlite)?)
            .bind(parse_ts(&created_raw, "submission_product_interests.created_at")?)
            .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_product_overrides(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, product_code, action, created_at FROM contact_product_interest_overrides ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        sqlx::query("INSERT INTO contact_product_interest_overrides (id,lead_contact_id,product_code,action,created_at) VALUES ($1,$2,$3,$4,$5)")
            .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("lead_contact_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("product_code").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("action").map_err(MigrationError::Sqlite)?)
            .bind(parse_ts(&created_raw, "contact_product_interest_overrides.created_at")?)
            .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_notes(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, body, created_at, updated_at FROM lead_notes ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        let updated_raw: String = row.try_get("updated_at").map_err(MigrationError::Sqlite)?;
        sqlx::query("INSERT INTO lead_notes (id,lead_contact_id,body,revision,created_at,updated_at) VALUES ($1,$2,$3,0,$4,$5)")
            .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("lead_contact_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("body").map_err(MigrationError::Sqlite)?)
            .bind(parse_ts(&created_raw, "lead_notes.created_at")?)
            .bind(parse_ts(&updated_raw, "lead_notes.updated_at")?)
            .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_activities(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, actor_user_id, activity_type, occurred_at, payload_json FROM lead_activities ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let occurred_raw: String = row.try_get("occurred_at").map_err(MigrationError::Sqlite)?;
        sqlx::query("INSERT INTO lead_activities (id,lead_contact_id,actor_user_id,activity_type,occurred_at,payload_json) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("lead_contact_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<Option<String>,_>("actor_user_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("activity_type").map_err(MigrationError::Sqlite)?)
            .bind(parse_ts(&occurred_raw, "lead_activities.occurred_at")?)
            .bind(row.try_get::<String,_>("payload_json").map_err(MigrationError::Sqlite)?)
            .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_followups(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, due_at, status, note, created_at, completed_at FROM follow_ups ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let due_raw: String = row.try_get("due_at").map_err(MigrationError::Sqlite)?;
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        let completed_raw: Option<String> = row.try_get("completed_at").map_err(MigrationError::Sqlite)?;
        let created_at = parse_ts(&created_raw, "follow_ups.created_at")?;
        let completed_at = parse_ts_opt(completed_raw.as_deref(), "follow_ups.completed_at")?;
        let updated_at = completed_at.unwrap_or(created_at);
        sqlx::query("INSERT INTO follow_ups (id,lead_contact_id,due_at,status,note,revision,created_at,updated_at,completed_at) VALUES ($1,$2,$3,$4,$5,0,$6,$7,$8)")
            .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("lead_contact_id").map_err(MigrationError::Sqlite)?)
            .bind(parse_ts(&due_raw, "follow_ups.due_at")?)
            .bind(row.try_get::<String,_>("status").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<Option<String>,_>("note").map_err(MigrationError::Sqlite)?)
            .bind(created_at)
            .bind(updated_at)
            .bind(completed_at)
            .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

async fn migrate_quality_issues(source: &SqlitePool, tx: &mut Transaction<'_, Postgres>) -> Result<(), MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, lead_submission_id, issue_type, severity, details_json, status, created_at, resolved_at FROM lead_data_quality_issues ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    for row in rows {
        let created_raw: String = row.try_get("created_at").map_err(MigrationError::Sqlite)?;
        let resolved_raw: Option<String> = row.try_get("resolved_at").map_err(MigrationError::Sqlite)?;
        sqlx::query("INSERT INTO lead_data_quality_issues (id,lead_contact_id,lead_submission_id,issue_type,severity,details_json,status,created_at,resolved_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<Option<String>,_>("lead_contact_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<Option<String>,_>("lead_submission_id").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("issue_type").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("severity").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("details_json").map_err(MigrationError::Sqlite)?)
            .bind(row.try_get::<String,_>("status").map_err(MigrationError::Sqlite)?)
            .bind(parse_ts(&created_raw, "lead_data_quality_issues.created_at")?)
            .bind(parse_ts_opt(resolved_raw.as_deref(), "lead_data_quality_issues.resolved_at")?)
            .execute(&mut **tx).await.map_err(MigrationError::Postgres)?;
    }
    Ok(())
}

fn parse_ts(value: &str, field: &str) -> Result<DateTime<Utc>, MigrationError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| MigrationError::InvalidTimestamp { field: field.to_string(), value: value.to_string() })
}

fn parse_ts_opt(value: Option<&str>, field: &str) -> Result<Option<DateTime<Utc>>, MigrationError> {
    value.filter(|value| !value.trim().is_empty()).map(|value| parse_ts(value, field)).transpose()
}

fn canonical_ts(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

async fn source_user_ids(source: &SqlitePool) -> Result<BTreeSet<String>, MigrationError> {
    let ids = sqlx::query_scalar::<_, String>("SELECT id FROM app_users ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    Ok(ids.into_iter().collect())
}

async fn collect_sqlite_domain_ids(source: &SqlitePool) -> Result<Vec<String>, MigrationError> {
    let mut values = Vec::new();
    for table in ID_TABLES {
        let sql = format!("SELECT id FROM {table} ORDER BY id");
        let ids = sqlx::query_scalar::<_, String>(&sql).fetch_all(source).await.map_err(MigrationError::Sqlite)?;
        values.extend(ids.into_iter().map(|id| format!("{table}|{id}")));
    }
    Ok(values)
}

async fn collect_postgres_domain_ids(target: &PgPool) -> Result<Vec<String>, MigrationError> {
    let mut values = Vec::new();
    for table in ID_TABLES {
        let sql = format!("SELECT id FROM {table} ORDER BY id");
        let ids = sqlx::query_scalar::<_, String>(&sql).fetch_all(target).await.map_err(MigrationError::Postgres)?;
        values.extend(ids.into_iter().map(|id| format!("{table}|{id}")));
    }
    Ok(values)
}

async fn digest_target_source_users(target: &PgPool, source_ids: &BTreeSet<String>) -> Result<String, MigrationError> {
    let rows = sqlx::query_scalar::<_, String>("SELECT id FROM app_users ORDER BY id")
        .fetch_all(target).await.map_err(MigrationError::Postgres)?;
    Ok(digest_values(rows.into_iter().filter(|id| source_ids.contains(id))))
}

async fn digest_sqlite_raw_payloads(source: &SqlitePool) -> Result<String, MigrationError> {
    let rows = sqlx::query("SELECT id, raw_payload_json FROM lead_submissions ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    Ok(digest_values(rows.into_iter().map(|row| format!("{}\u{0}{}", row.get::<String,_>("id"), row.get::<String,_>("raw_payload_json")))))
}

async fn digest_postgres_raw_payloads(target: &PgPool) -> Result<String, MigrationError> {
    let rows = sqlx::query("SELECT id, raw_payload_json FROM lead_submissions ORDER BY id")
        .fetch_all(target).await.map_err(MigrationError::Postgres)?;
    Ok(digest_values(rows.into_iter().map(|row| format!("{}\u{0}{}", row.get::<String,_>("id"), row.get::<String,_>("raw_payload_json")))))
}

async fn digest_sqlite_activities(source: &SqlitePool) -> Result<String, MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, actor_user_id, activity_type, occurred_at, payload_json FROM lead_activities ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    let mut values = Vec::new();
    for row in rows {
        let raw: String = row.try_get("occurred_at").map_err(MigrationError::Sqlite)?;
        values.push(format!("{}|{}|{}|{}|{}|{}",
            row.try_get::<String,_>("id").map_err(MigrationError::Sqlite)?,
            row.try_get::<String,_>("lead_contact_id").map_err(MigrationError::Sqlite)?,
            row.try_get::<Option<String>,_>("actor_user_id").map_err(MigrationError::Sqlite)?.unwrap_or_default(),
            row.try_get::<String,_>("activity_type").map_err(MigrationError::Sqlite)?,
            canonical_ts(parse_ts(&raw, "lead_activities.occurred_at")?),
            row.try_get::<String,_>("payload_json").map_err(MigrationError::Sqlite)?,
        ));
    }
    Ok(digest_values(values))
}

async fn digest_postgres_activities(target: &PgPool) -> Result<String, MigrationError> {
    let rows = sqlx::query("SELECT id, lead_contact_id, actor_user_id, activity_type, occurred_at, payload_json FROM lead_activities ORDER BY id")
        .fetch_all(target).await.map_err(MigrationError::Postgres)?;
    Ok(digest_values(rows.into_iter().map(|row| format!("{}|{}|{}|{}|{}|{}",
        row.get::<String,_>("id"), row.get::<String,_>("lead_contact_id"),
        row.get::<Option<String>,_>("actor_user_id").unwrap_or_default(), row.get::<String,_>("activity_type"),
        canonical_ts(row.get::<DateTime<Utc>,_>("occurred_at")), row.get::<String,_>("payload_json")))))
}

async fn digest_sqlite_assignments(source: &SqlitePool) -> Result<String, MigrationError> {
    let rows = sqlx::query("SELECT id, assigned_user_id FROM lead_contacts ORDER BY id")
        .fetch_all(source).await.map_err(MigrationError::Sqlite)?;
    Ok(digest_values(rows.into_iter().map(|row| format!("{}|{}", row.get::<String,_>("id"), row.get::<Option<String>,_>("assigned_user_id").unwrap_or_default()))))
}

async fn digest_postgres_assignments(target: &PgPool) -> Result<String, MigrationError> {
    let rows = sqlx::query("SELECT id, assigned_user_id FROM lead_contacts ORDER BY id")
        .fetch_all(target).await.map_err(MigrationError::Postgres)?;
    Ok(digest_values(rows.into_iter().map(|row| format!("{}|{}", row.get::<String,_>("id"), row.get::<Option<String>,_>("assigned_user_id").unwrap_or_default()))))
}

fn digest_values<I>(values: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

fn digest_check(source_sha256: String, target_sha256: String) -> DigestCheck {
    DigestCheck {
        matches: source_sha256 == target_sha256,
        source_sha256,
        target_sha256,
    }
}
