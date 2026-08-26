use sqlx::{PgPool, Row, sqlite::{SqliteConnectOptions, SqlitePoolOptions}};
use tempfile::NamedTempFile;

#[path = "../src/sqlite_migration.rs"]
mod sqlite_migration;

async fn clear_target(pool: &PgPool) {
    for statement in [
        "DELETE FROM auth_security_events",
        "DELETE FROM auth_one_time_tokens",
        "DELETE FROM auth_sessions",
        "DELETE FROM app_credentials",
        "DELETE FROM lead_data_quality_issues",
        "DELETE FROM follow_ups",
        "DELETE FROM lead_notes",
        "DELETE FROM contact_product_interest_overrides",
        "DELETE FROM submission_product_interests",
        "DELETE FROM lead_activities",
        "DELETE FROM lead_submissions",
        "DELETE FROM import_batches",
        "DELETE FROM lead_contacts",
        "DELETE FROM app_users",
    ] {
        sqlx::query(statement)
            .execute(pool)
            .await
            .expect("clear target fixture state");
    }
}

#[tokio::test]
async fn representative_schema_v4_migrates_with_stable_ids_raw_payloads_audit_and_assignments() {
    let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
        eprintln!("ELM_TEST_DATABASE_URL is not set; skipping SQLite migration integration test");
        return;
    };

    let target = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("connect target PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&target)
        .await
        .expect("target migrations");
    clear_target(&target).await;

    // Existing centralized/bootstrap identity is allowed as long as it does not collide
    // with stable IDs, e-mail addresses or auth subjects from the local v4 database.
    sqlx::query(
        r#"
        INSERT INTO app_users (
            id, display_name, email, role, is_active, auth_subject,
            revision, created_at, updated_at
        ) VALUES (
            'existing-central-admin', 'Existing Central Admin', 'central-admin@example.test',
            'ADMIN', TRUE, 'password:existing-central-admin', 0,
            '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z'
        )
        "#,
    )
    .execute(&target)
    .await
    .expect("seed existing central user");

    let source_file = NamedTempFile::new().expect("create SQLite fixture file");
    let source_options = SqliteConnectOptions::new()
        .filename(source_file.path())
        .create_if_missing(true)
        .foreign_keys(true);
    let source = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(source_options)
        .await
        .expect("open SQLite fixture");
    sqlx::migrate!("../src-tauri/migrations")
        .run(&source)
        .await
        .expect("apply local schema v4 migrations");

    sqlx::query(
        r#"
        INSERT INTO app_users (
            id, display_name, email, role, is_active, auth_subject, created_at, updated_at
        ) VALUES
            ('src-user-manager', 'Source Manager', 'source-manager@example.test', 'MANAGER', 1, NULL,
             '2026-08-10T08:00:00Z', '2026-08-10T08:00:00Z'),
            ('src-user-sales', 'Source Sales', 'source-sales@example.test', 'SALES', 1, NULL,
             '2026-08-10T08:05:00Z', '2026-08-10T08:05:00Z')
        "#,
    )
    .execute(&source)
    .await
    .expect("seed source users");

    sqlx::query(
        r#"
        INSERT INTO lead_contacts (
            id, display_name, primary_email, normalized_email, primary_phone,
            normalized_phone, country_code, status, created_at, updated_at,
            latest_submission_at, submission_count, assigned_user_id
        ) VALUES (
            'src-contact-1', 'Migration Test Lead', 'Migrate.Lead@example.test',
            'migrate.lead@example.test', '+90 555 000 00 01', '+905550000001',
            'TR', 'QUALIFIED', '2026-08-11T09:00:00Z', '2026-08-20T12:30:00Z',
            '2026-08-20T12:00:00Z', 2, 'src-user-sales'
        )
        "#,
    )
    .execute(&source)
    .await
    .expect("seed source contact");

    sqlx::query(
        r#"
        INSERT INTO import_batches (
            id, file_name, file_size, file_sha256, file_format, sheet_name,
            started_at, completed_at, status, total_rows, new_submissions,
            exact_duplicates, repeat_candidates, warning_count, error_count, app_version
        ) VALUES (
            'src-batch-1', 'migration-fixture.csv', 512, 'fixture-sha256', 'CSV', 'CSV',
            '2026-08-20T12:01:00Z', '2026-08-20T12:02:00Z', 'COMMITTED',
            2, 2, 0, 0, 1, 0, '0.1.0-local'
        )
        "#,
    )
    .execute(&source)
    .await
    .expect("seed source batch");

    let raw_one = r#"{"Status":"Contacted","İletişime Geçme Tarihi":"2026-08-21 10:00","note":"Türkçe ✓"}"#;
    let raw_two = r#"{"Status":"New","nested":{"source":"local-v4"},"emoji":"🧪"}"#;

    sqlx::query(
        r#"
        INSERT INTO lead_submissions (
            id, lead_contact_id, import_batch_id, external_lead_id,
            source_created_at_utc, source_created_at_raw,
            ad_id, ad_name, adset_id, adset_name, campaign_id, campaign_name,
            form_id, form_name, is_organic, platform,
            raw_procedure_answer, raw_product_answer, raw_full_name, raw_email,
            raw_phone, raw_country, raw_lead_status, normalized_email,
            normalized_phone, raw_payload_json, created_at
        ) VALUES
            (
                'src-submission-1', 'src-contact-1', 'src-batch-1', 'external:migration:1',
                '2026-08-20T10:00:00Z', '2026-08-20T13:00:00+03:00',
                'ad-1', 'Ad One', 'adset-1', 'Adset One', 'campaign-1', 'Campaign One',
                'form-1', 'Form One', 1, 'facebook', 'yes', 'fue_punches',
                'Migration Test Lead', 'Migrate.Lead@example.test', '+90 555 000 00 01',
                'TR', 'Contacted', 'migrate.lead@example.test', '+905550000001', $1,
                '2026-08-20T12:01:10Z'
            ),
            (
                'src-submission-2', 'src-contact-1', 'src-batch-1', 'external:migration:2',
                '2026-08-20T12:00:00Z', '2026-08-20T15:00:00+03:00',
                NULL, NULL, NULL, NULL, 'campaign-2', 'Campaign Two',
                'form-2', 'Form Two', 0, 'instagram', 'yes', 'fue_punches',
                'Migration Test Lead', 'Migrate.Lead@example.test', '+90 555 000 00 01',
                'TR', 'New', 'migrate.lead@example.test', '+905550000001', $2,
                '2026-08-20T12:02:10Z'
            )
        "#,
    )
    .bind(raw_one)
    .bind(raw_two)
    .execute(&source)
    .await
    .expect("seed source submissions");

    sqlx::query(
        r#"
        INSERT INTO submission_product_interests (
            id, lead_submission_id, product_code, origin, confidence, created_at
        ) VALUES
            ('src-product-1', 'src-submission-1', 'FUE_PUNCHES', 'DIRECT_MULTI_SELECT', 'HIGH', '2026-08-20T12:01:11Z'),
            ('src-product-2', 'src-submission-2', 'FUE_PUNCHES', 'DIRECT_MULTI_SELECT', 'HIGH', '2026-08-20T12:02:11Z')
        "#,
    )
    .execute(&source)
    .await
    .expect("seed product interests");

    sqlx::query(
        r#"
        INSERT INTO contact_product_interest_overrides (
            id, lead_contact_id, product_code, action, created_at
        ) VALUES (
            'src-override-1', 'src-contact-1', 'OTHER_GENERAL_INFORMATION', 'ADD',
            '2026-08-20T12:10:00Z'
        )
        "#,
    )
    .execute(&source)
    .await
    .expect("seed product override");

    sqlx::query(
        r#"
        INSERT INTO lead_notes (id, lead_contact_id, body, created_at, updated_at)
        VALUES ('src-note-1', 'src-contact-1', 'Local v4 note — korunmalı',
                '2026-08-20T12:15:00Z', '2026-08-20T12:16:00Z')
        "#,
    )
    .execute(&source)
    .await
    .expect("seed note");

    sqlx::query(
        r#"
        INSERT INTO lead_activities (
            id, lead_contact_id, activity_type, occurred_at, payload_json, actor_user_id
        ) VALUES (
            'src-activity-1', 'src-contact-1', 'STATUS_CHANGED',
            '2026-08-20T12:20:00Z', '{"from":"REPLIED","to":"QUALIFIED"}',
            'src-user-manager'
        )
        "#,
    )
    .execute(&source)
    .await
    .expect("seed audit activity");

    sqlx::query(
        r#"
        INSERT INTO follow_ups (
            id, lead_contact_id, due_at, status, note, created_at, completed_at
        ) VALUES (
            'src-followup-1', 'src-contact-1', '2026-08-28T09:00:00Z', 'OPEN',
            'Migration follow-up', '2026-08-20T12:25:00Z', NULL
        )
        "#,
    )
    .execute(&source)
    .await
    .expect("seed follow-up");

    sqlx::query(
        r#"
        INSERT INTO lead_data_quality_issues (
            id, lead_contact_id, lead_submission_id, issue_type, severity,
            details_json, status, created_at, resolved_at
        ) VALUES (
            'src-quality-1', 'src-contact-1', 'src-submission-2', 'UNKNOWN_PRODUCT',
            'WARNING', '{"sourceRow":3}', 'OPEN', '2026-08-20T12:30:00Z', NULL
        )
        "#,
    )
    .execute(&source)
    .await
    .expect("seed quality issue");

    source.close().await;

    let report = sqlite_migration::migrate_sqlite_v4(source_file.path(), &target)
        .await
        .expect("migrate schema v4 fixture");

    assert_eq!(report.source_schema_version, 4);
    assert_eq!(report.source_user_count, 2);
    assert!(report.all_checks_passed);
    assert!(report.table_counts.iter().all(|item| item.matches));
    assert!(report.migrated_user_ids.matches);
    assert!(report.domain_stable_ids.matches);
    assert!(report.raw_submission_payloads.matches);
    assert!(report.activity_audit.matches);
    assert!(report.assignments.matches);

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_users")
        .fetch_one(&target)
        .await
        .expect("count target users");
    assert_eq!(user_count, 3, "existing central user + two stable source users");

    let contact = sqlx::query(
        "SELECT assigned_user_id, status, revision, submission_count FROM lead_contacts WHERE id = 'src-contact-1'",
    )
    .fetch_one(&target)
    .await
    .expect("read migrated contact");
    assert_eq!(contact.get::<Option<String>, _>("assigned_user_id").as_deref(), Some("src-user-sales"));
    assert_eq!(contact.get::<String, _>("status"), "QUALIFIED");
    assert_eq!(contact.get::<i64, _>("revision"), 0);
    assert_eq!(contact.get::<i32, _>("submission_count"), 2);

    let payload: String = sqlx::query_scalar(
        "SELECT raw_payload_json FROM lead_submissions WHERE id = 'src-submission-1'",
    )
    .fetch_one(&target)
    .await
    .expect("read exact raw payload");
    assert_eq!(payload, raw_one);

    let organic: Option<bool> = sqlx::query_scalar(
        "SELECT is_organic FROM lead_submissions WHERE id = 'src-submission-1'",
    )
    .fetch_one(&target)
    .await
    .expect("read mapped boolean");
    assert_eq!(organic, Some(true));

    let actor: Option<String> = sqlx::query_scalar(
        "SELECT actor_user_id FROM lead_activities WHERE id = 'src-activity-1'",
    )
    .fetch_one(&target)
    .await
    .expect("read migrated audit actor");
    assert_eq!(actor.as_deref(), Some("src-user-manager"));

    let note_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM lead_notes WHERE id = 'src-note-1'",
    )
    .fetch_one(&target)
    .await
    .expect("read migrated note revision");
    let followup_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM follow_ups WHERE id = 'src-followup-1'",
    )
    .fetch_one(&target)
    .await
    .expect("read migrated follow-up revision");
    let source_user_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM app_users WHERE id = 'src-user-sales'",
    )
    .fetch_one(&target)
    .await
    .expect("read migrated user revision");
    assert_eq!((note_revision, followup_revision, source_user_revision), (0, 0, 0));

    let rerun = sqlite_migration::migrate_sqlite_v4(source_file.path(), &target)
        .await
        .expect_err("second migration must fail closed on non-empty target");
    assert!(matches!(rerun, sqlite_migration::MigrationError::TargetNotEmpty { .. }));

    clear_target(&target).await;
    target.close().await;
}
