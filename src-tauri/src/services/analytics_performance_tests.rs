use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};

use super::analytics_service::{AnalyticsRequest, AnalyticsService};
use crate::db::Database;

#[tokio::test]
async fn ten_thousand_contacts_and_twenty_five_thousand_submissions_remain_analytics_queryable() {
    let database = Database::connect_memory().await.expect("open database");
    let now = Utc::now();
    let now_text = now.to_rfc3339_opts(SecondsFormat::Millis, true);
    let start_text = (now - ChronoDuration::days(45)).to_rfc3339_opts(SecondsFormat::Millis, true);

    sqlx::query(
        r#"
        WITH digits(d) AS (VALUES (0),(1),(2),(3),(4),(5),(6),(7),(8),(9)),
        numbers(n) AS (
            SELECT a.d * 1000 + b.d * 100 + c.d * 10 + d.d
            FROM digits a CROSS JOIN digits b CROSS JOIN digits c CROSS JOIN digits d
        )
        INSERT INTO lead_contacts (
            id, display_name, primary_email, normalized_email, primary_phone, normalized_phone,
            country_code, status, created_at, updated_at, latest_submission_at, submission_count
        )
        SELECT
            printf('analytics-contact-%05d', n),
            printf('Analytics Lead %05d', n),
            printf('analytics-%05d@example.test', n),
            printf('analytics-%05d@example.test', n),
            printf('+90555%06d', n),
            printf('+90555%06d', n),
            CASE n % 4 WHEN 0 THEN 'TR' WHEN 1 THEN 'GB' WHEN 2 THEN 'US' ELSE 'PT' END,
            CASE n % 8
                WHEN 0 THEN 'NEW' WHEN 1 THEN 'CONTACTED' WHEN 2 THEN 'REPLIED'
                WHEN 3 THEN 'QUALIFIED' WHEN 4 THEN 'QUOTE_SENT' WHEN 5 THEN 'WON'
                WHEN 6 THEN 'LOST' ELSE 'INVALID' END,
            ?, ?, ?, CASE WHEN n % 2 = 0 THEN 3 ELSE 2 END
        FROM numbers
        "#,
    )
    .bind(&start_text)
    .bind(&now_text)
    .bind(&now_text)
    .execute(database.pool())
    .await
    .expect("seed contacts");

    sqlx::query(
        "INSERT INTO import_batches (id, file_name, sheet_name, started_at, completed_at, status, total_rows, app_version) VALUES ('analytics-perf-batch', 'analytics-performance.csv', 'CSV', ?, ?, 'COMMITTED', 25000, '0.1.0')",
    )
    .bind(&start_text)
    .bind(&now_text)
    .execute(database.pool())
    .await
    .expect("seed batch");

    for (prefix, day_offset) in [("a", 40_i64), ("b", 20_i64)] {
        let timestamp = (now - ChronoDuration::days(day_offset))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        sqlx::query(
            r#"
            INSERT INTO lead_submissions (
                id, lead_contact_id, import_batch_id, external_lead_id,
                source_created_at_utc, source_created_at_raw, platform,
                campaign_id, campaign_name, form_id, form_name,
                adset_id, adset_name, ad_id, ad_name,
                raw_email, normalized_email, raw_payload_json, created_at
            )
            SELECT
                ? || '-' || id,
                id,
                'analytics-perf-batch',
                'l:' || ? || '-' || id,
                ?, ?,
                CASE CAST(substr(id, -1) AS INTEGER) % 2 WHEN 0 THEN 'facebook' ELSE 'instagram' END,
                printf('cmp-%02d', CAST(substr(id, -2) AS INTEGER) % 20),
                printf('Campaign %02d', CAST(substr(id, -2) AS INTEGER) % 20),
                printf('form-%02d', CAST(substr(id, -2) AS INTEGER) % 8),
                printf('Form %02d', CAST(substr(id, -2) AS INTEGER) % 8),
                printf('set-%02d', CAST(substr(id, -2) AS INTEGER) % 30),
                printf('Ad Set %02d', CAST(substr(id, -2) AS INTEGER) % 30),
                printf('ad-%03d', CAST(substr(id, -3) AS INTEGER) % 120),
                printf('Creative %03d', CAST(substr(id, -3) AS INTEGER) % 120),
                primary_email, normalized_email, '{}', ?
            FROM lead_contacts
            "#,
        )
        .bind(prefix)
        .bind(prefix)
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(database.pool())
        .await
        .expect("seed base submissions");
    }

    let recent_timestamp = (now - ChronoDuration::days(3))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query(
        r#"
        INSERT INTO lead_submissions (
            id, lead_contact_id, import_batch_id, external_lead_id,
            source_created_at_utc, source_created_at_raw, platform,
            campaign_id, campaign_name, form_id, form_name,
            adset_id, adset_name, ad_id, ad_name,
            raw_email, normalized_email, raw_payload_json, created_at
        )
        SELECT
            'c-' || id, id, 'analytics-perf-batch', 'l:c-' || id,
            ?, ?, 'instagram',
            'cmp-repeat', 'Repeat Campaign', 'form-repeat', 'Repeat Form',
            'set-repeat', 'Repeat Set', 'ad-repeat', 'Repeat Creative',
            primary_email, normalized_email, '{}', ?
        FROM lead_contacts
        WHERE submission_count = 3
        "#,
    )
    .bind(&recent_timestamp)
    .bind(&recent_timestamp)
    .bind(&recent_timestamp)
    .execute(database.pool())
    .await
    .expect("seed third submissions");

    sqlx::query(
        r#"
        INSERT INTO submission_product_interests (
            id, lead_submission_id, product_code, origin, confidence, created_at
        )
        SELECT
            'product-' || id,
            id,
            CASE CAST(substr(lead_contact_id, -1) AS INTEGER) % 4
                WHEN 0 THEN 'FUE_MICROMOTOR_SYSTEMS'
                WHEN 1 THEN 'LONG_HAIR_FUE_SOLUTIONS'
                WHEN 2 THEN 'FUE_PUNCHES'
                ELSE 'MEDICAL_CHAIRS_CLINIC_FURNITURE' END,
            'DIRECT_MULTI_SELECT', 'HIGH', ?
        FROM lead_submissions
        "#,
    )
    .bind(&now_text)
    .execute(database.pool())
    .await
    .expect("seed product interests");

    let service = AnalyticsService::new(database.pool().clone());
    let started = Instant::now();
    let report = service
        .report(AnalyticsRequest {
            from_utc: Some(start_text),
            to_utc: Some((now + ChronoDuration::days(1)).to_rfc3339_opts(SecondsFormat::Millis, true)),
        })
        .await
        .expect("run full analytics report");

    assert_eq!(report.summary.submissions, 25_000);
    assert_eq!(report.summary.unique_contacts, 10_000);
    assert_eq!(report.summary.repeat_submissions, 15_000);
    assert_eq!(report.country_breakdown.len(), 4);
    assert_eq!(report.platform_breakdown.len(), 2);
    assert_eq!(report.product_breakdown.len(), 4);
    assert!(report.campaign_breakdown.len() >= 20);
    assert!(report.form_breakdown.len() >= 8);
    assert!(report.adset_breakdown.len() >= 30);
    assert!(report.ad_breakdown.len() >= 100);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "full 10k/25k analytics report exceeded the 10 second smoke-test budget: {:?}",
        started.elapsed()
    );
}
