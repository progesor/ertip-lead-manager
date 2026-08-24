use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::analytics_repository::{
    AnalyticsBreakdownRecord, AnalyticsRepository, AnalyticsStatusRecord, AnalyticsTrendRecord,
};

const STATUS_ORDER: [&str; 8] = [
    "NEW",
    "CONTACTED",
    "REPLIED",
    "QUALIFIED",
    "QUOTE_SENT",
    "WON",
    "LOST",
    "INVALID",
];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRequest {
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRange {
    pub earliest_submission_at: Option<String>,
    pub latest_submission_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTrendPoint {
    pub day: String,
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsStatusPoint {
    pub status: String,
    pub contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsBreakdownPoint {
    pub key: String,
    pub submissions: i64,
    pub unique_contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsResponse {
    pub range: AnalyticsRange,
    pub summary: AnalyticsSummary,
    pub trend: Vec<AnalyticsTrendPoint>,
    pub current_status_funnel: Vec<AnalyticsStatusPoint>,
    pub country_breakdown: Vec<AnalyticsBreakdownPoint>,
    pub platform_breakdown: Vec<AnalyticsBreakdownPoint>,
    pub product_breakdown: Vec<AnalyticsBreakdownPoint>,
}

#[derive(Clone)]
pub struct AnalyticsService {
    repository: AnalyticsRepository,
}

impl AnalyticsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: AnalyticsRepository::new(pool),
        }
    }

    pub async fn report(&self, request: AnalyticsRequest) -> Result<AnalyticsResponse, AppError> {
        let from_utc = canonical_optional(request.from_utc, "fromUtc")?;
        let to_utc = canonical_optional(request.to_utc, "toUtc")?;
        if let (Some(from), Some(to)) = (&from_utc, &to_utc) {
            if from >= to {
                return Err(AppError::Validation(
                    "fromUtc must be before toUtc".to_string(),
                ));
            }
        }

        let from = from_utc.as_deref();
        let to = to_utc.as_deref();
        let range = self.repository.data_range().await?;
        let summary = self.repository.summary(from, to).await?;
        let trend = self.repository.trend(from, to).await?;
        let statuses = self.repository.current_statuses(from, to).await?;
        let countries = self.repository.country_breakdown(from, to).await?;
        let platforms = self.repository.platform_breakdown(from, to).await?;
        let products = self.repository.product_breakdown(from, to).await?;

        Ok(AnalyticsResponse {
            range: AnalyticsRange {
                earliest_submission_at: range.earliest_submission_at,
                latest_submission_at: range.latest_submission_at,
            },
            summary: AnalyticsSummary {
                submissions: summary.submissions,
                unique_contacts: summary.unique_contacts,
                repeat_submissions: summary.repeat_submissions,
            },
            trend: trend.into_iter().map(trend_point).collect(),
            current_status_funnel: ordered_statuses(statuses),
            country_breakdown: countries.into_iter().map(breakdown_point).collect(),
            platform_breakdown: platforms.into_iter().map(breakdown_point).collect(),
            product_breakdown: products.into_iter().map(breakdown_point).collect(),
        })
    }
}

fn canonical_optional(value: Option<String>, field: &str) -> Result<Option<String>, AppError> {
    let Some(value) = value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(&value)
        .map_err(|_| AppError::Validation(format!("{field} must be RFC3339")))?;
    Ok(Some(
        parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true),
    ))
}

fn trend_point(record: AnalyticsTrendRecord) -> AnalyticsTrendPoint {
    AnalyticsTrendPoint {
        day: record.day,
        submissions: record.submissions,
        unique_contacts: record.unique_contacts,
        repeat_submissions: record.repeat_submissions,
    }
}

fn breakdown_point(record: AnalyticsBreakdownRecord) -> AnalyticsBreakdownPoint {
    AnalyticsBreakdownPoint {
        key: record.key,
        submissions: record.submissions,
        unique_contacts: record.unique_contacts,
    }
}

fn ordered_statuses(records: Vec<AnalyticsStatusRecord>) -> Vec<AnalyticsStatusPoint> {
    let mut counts = std::collections::BTreeMap::new();
    for record in records {
        counts.insert(record.status, record.contacts);
    }
    STATUS_ORDER
        .iter()
        .map(|status| AnalyticsStatusPoint {
            status: (*status).to_string(),
            contacts: counts.remove(*status).unwrap_or(0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AnalyticsRequest, AnalyticsService};
    use crate::db::Database;

    #[tokio::test]
    async fn report_separates_contacts_submissions_repeats_and_multi_product_membership() {
        let database = Database::connect_memory().await.expect("open database");
        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, country_code, status, created_at, updated_at, latest_submission_at, submission_count) VALUES ('a', 'Alpha', 'TR', 'CONTACTED', '2026-08-20T00:00:00.000Z', '2026-08-20T00:00:00.000Z', '2026-08-21T10:00:00.000Z', 2), ('b', 'Beta', 'GB', 'WON', '2026-08-20T00:00:00.000Z', '2026-08-20T00:00:00.000Z', '2026-08-21T11:00:00.000Z', 1)"
        )
        .execute(database.pool())
        .await
        .expect("seed contacts");
        sqlx::query(
            "INSERT INTO import_batches (id, file_name, sheet_name, started_at, completed_at, status, total_rows, app_version) VALUES ('batch', 'fixture.csv', 'CSV', '2026-08-20T00:00:00.000Z', '2026-08-20T00:00:00.000Z', 'COMMITTED', 3, '0.1.0')"
        )
        .execute(database.pool())
        .await
        .expect("seed batch");
        for (id, contact, external_id, timestamp, platform) in [
            ("a1", "a", "l:a1", "2026-08-20T09:00:00.000Z", "facebook"),
            ("a2", "a", "l:a2", "2026-08-21T10:00:00.000Z", "instagram"),
            ("b1", "b", "l:b1", "2026-08-21T11:00:00.000Z", "facebook"),
        ] {
            sqlx::query("INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_utc, source_created_at_raw, platform, raw_payload_json, created_at) VALUES (?, ?, 'batch', ?, ?, ?, ?, '{}', ?)")
                .bind(id)
                .bind(contact)
                .bind(external_id)
                .bind(timestamp)
                .bind(timestamp)
                .bind(platform)
                .bind(timestamp)
                .execute(database.pool())
                .await
                .expect("seed submission");
        }
        for (id, submission, product) in [
            ("p1", "a1", "FUE_PUNCHES"),
            ("p2", "a1", "LONG_HAIR_FUE_SOLUTIONS"),
            ("p3", "a2", "FUE_PUNCHES"),
        ] {
            sqlx::query("INSERT INTO submission_product_interests (id, lead_submission_id, product_code, origin, confidence, created_at) VALUES (?, ?, ?, 'DIRECT_MULTI_SELECT', 'HIGH', '2026-08-21T00:00:00.000Z')")
                .bind(id)
                .bind(submission)
                .bind(product)
                .execute(database.pool())
                .await
                .expect("seed product");
        }

        let report = AnalyticsService::new(database.pool().clone())
            .report(AnalyticsRequest {
                from_utc: Some("2026-08-20T00:00:00Z".to_string()),
                to_utc: Some("2026-08-22T00:00:00Z".to_string()),
            })
            .await
            .expect("analytics report");

        assert_eq!(report.summary.submissions, 3);
        assert_eq!(report.summary.unique_contacts, 2);
        assert_eq!(report.summary.repeat_submissions, 1);
        assert_eq!(report.trend.len(), 2);
        assert_eq!(report.trend[1].submissions, 2);
        assert_eq!(
            report
                .current_status_funnel
                .iter()
                .find(|item| item.status == "WON")
                .expect("won status")
                .contacts,
            1
        );
        assert_eq!(
            report
                .product_breakdown
                .iter()
                .find(|item| item.key == "FUE_PUNCHES")
                .expect("punch product")
                .submissions,
            2
        );
        assert_eq!(
            report
                .product_breakdown
                .iter()
                .find(|item| item.key == "LONG_HAIR_FUE_SOLUTIONS")
                .expect("long hair product")
                .submissions,
            1
        );
        assert_eq!(
            report
                .product_breakdown
                .iter()
                .find(|item| item.key == "NO_PRODUCT")
                .expect("no product")
                .submissions,
            1
        );
    }
}
