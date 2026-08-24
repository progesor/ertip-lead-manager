use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::error::AppError;

const DEFAULT_GROUP_LIMIT: u32 = 6;
const MAX_GROUP_LIMIT: u32 = 20;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionRequest {
    pub now_utc: String,
    pub today_start_utc: String,
    pub tomorrow_start_utc: String,
    pub recent_repeat_since_utc: String,
    pub group_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardKpis {
    pub total_contacts: i64,
    pub new_contacts: i64,
    pub qualified_contacts: i64,
    pub quote_sent_contacts: i64,
    pub won_contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionLead {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub latest_submission_at: Option<String>,
    pub due_at: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionGroup {
    pub total: i64,
    pub items: Vec<DashboardAttentionLead>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionResponse {
    pub kpis: DashboardKpis,
    pub new_uncontacted: DashboardAttentionGroup,
    pub due_today: DashboardAttentionGroup,
    pub overdue: DashboardAttentionGroup,
    pub recent_repeats: DashboardAttentionGroup,
    pub open_quality_issues: DashboardAttentionGroup,
}

#[derive(Debug, Clone, FromRow)]
struct LeadRow {
    id: String,
    display_name: Option<String>,
    status: String,
    primary_phone: Option<String>,
    country_code: Option<String>,
    latest_submission_at: Option<String>,
    due_at: Option<String>,
    count: i64,
}

#[derive(Clone)]
pub struct DashboardService {
    pool: SqlitePool,
}

impl DashboardService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn attention(
        &self,
        request: DashboardAttentionRequest,
    ) -> Result<DashboardAttentionResponse, AppError> {
        let now = canonical_utc(&request.now_utc, "nowUtc")?;
        let today_start = canonical_utc(&request.today_start_utc, "todayStartUtc")?;
        let tomorrow_start = canonical_utc(&request.tomorrow_start_utc, "tomorrowStartUtc")?;
        let recent_since = canonical_utc(&request.recent_repeat_since_utc, "recentRepeatSinceUtc")?;
        if today_start >= tomorrow_start {
            return Err(AppError::Validation(
                "todayStartUtc must be before tomorrowStartUtc".to_string(),
            ));
        }

        let limit = request
            .group_limit
            .unwrap_or(DEFAULT_GROUP_LIMIT)
            .clamp(1, MAX_GROUP_LIMIT) as i64;

        Ok(DashboardAttentionResponse {
            kpis: self.kpis().await?,
            new_uncontacted: self.new_uncontacted(limit).await?,
            due_today: self
                .follow_up_group(&today_start, &tomorrow_start, None, limit)
                .await?,
            overdue: self
                .follow_up_group("", "", Some(&now), limit)
                .await?,
            recent_repeats: self.recent_repeats(&recent_since, limit).await?,
            open_quality_issues: self.open_quality_issues(limit).await?,
        })
    }

    async fn kpis(&self) -> Result<DashboardKpis, AppError> {
        let total_contacts = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM lead_contacts")
            .fetch_one(&self.pool)
            .await?;

        async fn count_status(pool: &SqlitePool, status: &str) -> Result<i64, AppError> {
            Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM lead_contacts WHERE status = ?")
                .bind(status)
                .fetch_one(pool)
                .await?)
        }

        Ok(DashboardKpis {
            total_contacts,
            new_contacts: count_status(&self.pool, "NEW").await?,
            qualified_contacts: count_status(&self.pool, "QUALIFIED").await?,
            quote_sent_contacts: count_status(&self.pool, "QUOTE_SENT").await?,
            won_contacts: count_status(&self.pool, "WON").await?,
        })
    }

    async fn new_uncontacted(&self, limit: i64) -> Result<DashboardAttentionGroup, AppError> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM lead_contacts WHERE status = 'NEW'")
            .fetch_one(&self.pool)
            .await?;
        let rows = sqlx::query_as::<_, LeadRow>(
            r#"
            SELECT id, display_name, status, primary_phone, country_code, latest_submission_at,
                   NULL AS due_at, 1 AS count
            FROM lead_contacts
            WHERE status = 'NEW'
            ORDER BY latest_submission_at IS NULL ASC, latest_submission_at DESC, id ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(group(total, rows))
    }

    async fn follow_up_group(
        &self,
        start: &str,
        end: &str,
        overdue_before: Option<&str>,
        limit: i64,
    ) -> Result<DashboardAttentionGroup, AppError> {
        let (total, rows) = if let Some(before) = overdue_before {
            let total = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM follow_ups WHERE status = 'OPEN' AND due_at < ?",
            )
            .bind(before)
            .fetch_one(&self.pool)
            .await?;
            let rows = sqlx::query_as::<_, LeadRow>(
                r#"
                SELECT c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                       c.latest_submission_at, MIN(f.due_at) AS due_at, COUNT(*) AS count
                FROM follow_ups f
                JOIN lead_contacts c ON c.id = f.lead_contact_id
                WHERE f.status = 'OPEN' AND f.due_at < ?
                GROUP BY c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                         c.latest_submission_at
                ORDER BY MIN(f.due_at) ASC, c.id ASC
                LIMIT ?
                "#,
            )
            .bind(before)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        } else {
            let total = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM follow_ups WHERE status = 'OPEN' AND due_at >= ? AND due_at < ?",
            )
            .bind(start)
            .bind(end)
            .fetch_one(&self.pool)
            .await?;
            let rows = sqlx::query_as::<_, LeadRow>(
                r#"
                SELECT c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                       c.latest_submission_at, MIN(f.due_at) AS due_at, COUNT(*) AS count
                FROM follow_ups f
                JOIN lead_contacts c ON c.id = f.lead_contact_id
                WHERE f.status = 'OPEN' AND f.due_at >= ? AND f.due_at < ?
                GROUP BY c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                         c.latest_submission_at
                ORDER BY MIN(f.due_at) ASC, c.id ASC
                LIMIT ?
                "#,
            )
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        };
        Ok(group(total, rows))
    }

    async fn recent_repeats(
        &self,
        recent_since: &str,
        limit: i64,
    ) -> Result<DashboardAttentionGroup, AppError> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM lead_contacts WHERE submission_count > 1 AND latest_submission_at >= ?",
        )
        .bind(recent_since)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query_as::<_, LeadRow>(
            r#"
            SELECT id, display_name, status, primary_phone, country_code, latest_submission_at,
                   NULL AS due_at, submission_count AS count
            FROM lead_contacts
            WHERE submission_count > 1 AND latest_submission_at >= ?
            ORDER BY latest_submission_at DESC, id ASC
            LIMIT ?
            "#,
        )
        .bind(recent_since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(group(total, rows))
    }

    async fn open_quality_issues(&self, limit: i64) -> Result<DashboardAttentionGroup, AppError> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM lead_data_quality_issues WHERE status = 'OPEN'",
        )
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query_as::<_, LeadRow>(
            r#"
            SELECT c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                   c.latest_submission_at, NULL AS due_at, COUNT(*) AS count
            FROM lead_data_quality_issues q
            JOIN lead_contacts c ON c.id = q.lead_contact_id
            WHERE q.status = 'OPEN'
            GROUP BY c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                     c.latest_submission_at
            ORDER BY COUNT(*) DESC, c.latest_submission_at DESC, c.id ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(group(total, rows))
    }
}

fn canonical_utc(value: &str, field: &str) -> Result<String, AppError> {
    let parsed = DateTime::parse_from_rfc3339(value.trim())
        .map_err(|_| AppError::Validation(format!("{field} must be RFC3339")))?;
    Ok(parsed
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn group(total: i64, rows: Vec<LeadRow>) -> DashboardAttentionGroup {
    DashboardAttentionGroup {
        total,
        items: rows
            .into_iter()
            .map(|row| DashboardAttentionLead {
                id: row.id,
                display_name: row
                    .display_name
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "İsimsiz lead".to_string()),
                status: row.status,
                primary_phone: row.primary_phone,
                country_code: row.country_code,
                latest_submission_at: row.latest_submission_at,
                due_at: row.due_at,
                count: row.count,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{DashboardAttentionRequest, DashboardService};
    use crate::db::Database;

    #[tokio::test]
    async fn attention_groups_use_supplied_local_day_boundaries_and_include_phone() {
        let database = Database::connect_memory().await.expect("open database");
        for (id, status, submissions, latest, phone) in [
            (
                "dash-new",
                "NEW",
                1_i64,
                "2026-08-22T08:00:00.000Z",
                "+905551111111",
            ),
            (
                "dash-repeat",
                "CONTACTED",
                2_i64,
                "2026-08-22T09:00:00.000Z",
                "+905552222222",
            ),
        ] {
            sqlx::query(
                "INSERT INTO lead_contacts (id, display_name, primary_phone, status, created_at, updated_at, latest_submission_at, submission_count) VALUES (?, ?, ?, ?, '2026-08-20T00:00:00.000Z', '2026-08-20T00:00:00.000Z', ?, ?)",
            )
            .bind(id)
            .bind(id)
            .bind(phone)
            .bind(status)
            .bind(latest)
            .bind(submissions)
            .execute(database.pool())
            .await
            .expect("seed dashboard contact");
        }

        for (id, contact, due) in [
            ("dash-overdue", "dash-new", "2026-08-22T08:30:00.000Z"),
            ("dash-today", "dash-repeat", "2026-08-22T12:00:00.000Z"),
        ] {
            sqlx::query(
                "INSERT INTO follow_ups (id, lead_contact_id, due_at, status, created_at) VALUES (?, ?, ?, 'OPEN', '2026-08-20T00:00:00.000Z')",
            )
            .bind(id)
            .bind(contact)
            .bind(due)
            .execute(database.pool())
            .await
            .expect("seed follow-up");
        }

        sqlx::query(
            "INSERT INTO lead_data_quality_issues (id, lead_contact_id, issue_type, severity, status, created_at) VALUES ('dash-warning', 'dash-new', 'UNKNOWN_PRODUCT', 'WARNING', 'OPEN', '2026-08-20T00:00:00.000Z')",
        )
        .execute(database.pool())
        .await
        .expect("seed warning");

        let response = DashboardService::new(database.pool().clone())
            .attention(DashboardAttentionRequest {
                now_utc: "2026-08-22T09:47:00.000Z".to_string(),
                today_start_utc: "2026-08-22T09:47:00.000Z".to_string(),
                tomorrow_start_utc: "2026-08-22T21:00:00.000Z".to_string(),
                recent_repeat_since_utc: "2026-08-15T09:47:00.000Z".to_string(),
                group_limit: Some(6),
            })
            .await
            .expect("load dashboard attention");

        assert_eq!(response.kpis.total_contacts, 2);
        assert_eq!(response.new_uncontacted.total, 1);
        assert_eq!(response.overdue.total, 1);
        assert_eq!(response.due_today.total, 1);
        assert_eq!(response.recent_repeats.total, 1);
        assert_eq!(response.open_quality_issues.total, 1);
        assert_eq!(
            response.new_uncontacted.items[0].primary_phone.as_deref(),
            Some("+905551111111")
        );
    }
}
