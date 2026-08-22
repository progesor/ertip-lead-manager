use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};

use super::lead_workspace_service::{LeadListRequest, LeadWorkspaceService};
use crate::db::Database;

#[tokio::test]
async fn ten_thousand_contacts_and_twenty_five_thousand_submissions_remain_queryable() {
    let database = Database::connect_memory().await.expect("open database");
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    sqlx::query(
        r#"
        WITH digits(d) AS (
            VALUES (0),(1),(2),(3),(4),(5),(6),(7),(8),(9)
        ), numbers(n) AS (
            SELECT a.d * 1000 + b.d * 100 + c.d * 10 + d.d
            FROM digits a CROSS JOIN digits b CROSS JOIN digits c CROSS JOIN digits d
        )
        INSERT INTO lead_contacts (
            id, display_name, primary_email, normalized_email, primary_phone, normalized_phone,
            country_code, status, created_at, updated_at, latest_submission_at, submission_count
        )
        SELECT
            printf('perf-contact-%05d', n),
            printf('Performance Lead %05d', n),
            printf('perf-%05d@example.test', n),
            printf('perf-%05d@example.test', n),
            printf('+90555%06d', n),
            printf('+90555%06d', n),
            CASE n % 4 WHEN 0 THEN 'TR' WHEN 1 THEN 'GB' WHEN 2 THEN 'US' ELSE 'PT' END,
            'NEW', ?, ?, ?, CASE WHEN n % 2 = 0 THEN 3 ELSE 2 END
        FROM numbers
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(database.pool())
    .await
    .expect("seed contacts");

    sqlx::query(
        "INSERT INTO import_batches (id, file_name, sheet_name, started_at, completed_at, status, total_rows, app_version) VALUES ('perf-batch', 'performance.csv', 'CSV', ?, ?, 'COMMITTED', 25000, '0.1.0')",
    )
    .bind(&now)
    .bind(&now)
    .execute(database.pool())
    .await
    .expect("seed batch");

    for prefix in ["a", "b"] {
        sqlx::query(
            r#"
            INSERT INTO lead_submissions (
                id, lead_contact_id, import_batch_id, external_lead_id,
                source_created_at_utc, source_created_at_raw, platform,
                raw_email, normalized_email, raw_payload_json, created_at
            )
            SELECT
                ? || '-' || id,
                id,
                'perf-batch',
                'l:' || ? || '-' || id,
                ?, ?, CASE CAST(substr(id, -1) AS INTEGER) % 2 WHEN 0 THEN 'facebook' ELSE 'instagram' END,
                primary_email, normalized_email, '{}', ?
            FROM lead_contacts
            "#,
        )
        .bind(prefix)
        .bind(prefix)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("seed base submissions");
    }

    sqlx::query(
        r#"
        INSERT INTO lead_submissions (
            id, lead_contact_id, import_batch_id, external_lead_id,
            source_created_at_utc, source_created_at_raw, platform,
            raw_email, normalized_email, raw_payload_json, created_at
        )
        SELECT
            'c-' || id,
            id,
            'perf-batch',
            'l:c-' || id,
            ?, ?, 'instagram', primary_email, normalized_email, '{}', ?
        FROM lead_contacts
        WHERE submission_count = 3
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(database.pool())
    .await
    .expect("seed repeat submissions");

    let service = LeadWorkspaceService::new(database.pool().clone());
    let started = Instant::now();

    let page = service
        .list(LeadListRequest {
            page: Some(0),
            page_size: Some(25),
            sort: Some("LATEST_DESC".to_string()),
            ..LeadListRequest::default()
        })
        .await
        .expect("list large dataset");

    assert_eq!(page.total, 10_000);
    assert_eq!(page.items.len(), 25);

    let search = service
        .list(LeadListRequest {
            search: Some("perf-09999@example.test".to_string()),
            page: Some(0),
            page_size: Some(25),
            ..LeadListRequest::default()
        })
        .await
        .expect("search large dataset");

    assert_eq!(search.total, 1);
    assert_eq!(search.items[0].display_name, "Performance Lead 09999");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "large-dataset list + search exceeded the 5 second smoke-test budget: {:?}",
        started.elapsed()
    );
}
