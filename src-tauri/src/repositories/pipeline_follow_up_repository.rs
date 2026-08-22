use std::collections::HashMap;

use sqlx::{FromRow, SqlitePool};

use crate::error::AppError;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PipelineFollowUpSummaryRecord {
    pub lead_contact_id: String,
    pub next_due_at: String,
    pub open_count: i64,
}

#[derive(Clone)]
pub struct PipelineFollowUpRepository {
    pool: SqlitePool,
}

impl PipelineFollowUpRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn open_summaries(
        &self,
    ) -> Result<HashMap<String, PipelineFollowUpSummaryRecord>, AppError> {
        let rows = sqlx::query_as::<_, PipelineFollowUpSummaryRecord>(
            r#"
            SELECT lead_contact_id,
                   MIN(due_at) AS next_due_at,
                   COUNT(*) AS open_count
            FROM follow_ups
            WHERE status = 'OPEN'
            GROUP BY lead_contact_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| (row.lead_contact_id.clone(), row))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::PipelineFollowUpRepository;
    use crate::db::Database;

    #[tokio::test]
    async fn open_summary_returns_earliest_due_and_count() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, status, created_at, updated_at, submission_count) VALUES ('summary-contact', 'Summary', 'NEW', ?, ?, 0)",
        )
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("seed contact");

        for (id, due, status) in [
            ("fu-late", "2026-08-24T09:00:00.000Z", "OPEN"),
            ("fu-early", "2026-08-23T08:00:00.000Z", "OPEN"),
            ("fu-done", "2026-08-22T08:00:00.000Z", "COMPLETED"),
        ] {
            sqlx::query(
                "INSERT INTO follow_ups (id, lead_contact_id, due_at, status, created_at) VALUES (?, 'summary-contact', ?, ?, ?)",
            )
            .bind(id)
            .bind(due)
            .bind(status)
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("seed follow-up");
        }

        let summaries = PipelineFollowUpRepository::new(database.pool().clone())
            .open_summaries()
            .await
            .expect("load summaries");
        let summary = summaries.get("summary-contact").expect("contact summary");
        assert_eq!(summary.next_due_at, "2026-08-23T08:00:00.000Z");
        assert_eq!(summary.open_count, 2);
    }
}
