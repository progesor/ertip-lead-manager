use sqlx::{Row, SqlitePool};

use crate::error::AppError;

const TS: &str = "COALESCE(s.source_created_at_utc, s.created_at)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsRangeRecord {
    pub earliest_submission_at: Option<String>,
    pub latest_submission_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsSummaryRecord {
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsTrendRecord {
    pub day: String,
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsStatusRecord {
    pub status: String,
    pub contacts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsBreakdownRecord {
    pub key: String,
    pub submissions: i64,
    pub unique_contacts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsNamedBreakdownRecord {
    pub key: String,
    pub name: String,
    pub submissions: i64,
    pub unique_contacts: i64,
}

#[derive(Clone)]
pub struct AnalyticsRepository {
    pool: SqlitePool,
}

impl AnalyticsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn data_range(&self) -> Result<AnalyticsRangeRecord, AppError> {
        let sql = format!(
            "SELECT MIN({TS}) AS earliest_submission_at, MAX({TS}) AS latest_submission_at FROM lead_submissions s"
        );
        let row = sqlx::query(&sql).fetch_one(&self.pool).await?;
        Ok(AnalyticsRangeRecord {
            earliest_submission_at: row.try_get("earliest_submission_at")?,
            latest_submission_at: row.try_get("latest_submission_at")?,
        })
    }

    pub async fn summary(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<AnalyticsSummaryRecord, AppError> {
        let sql = format!(
            r#"
            SELECT
                COUNT(*) AS submissions,
                COUNT(DISTINCT s.lead_contact_id) AS unique_contacts,
                COALESCE(SUM(CASE WHEN EXISTS (
                    SELECT 1
                    FROM lead_submissions earlier
                    WHERE earlier.lead_contact_id = s.lead_contact_id
                      AND (
                        COALESCE(earlier.source_created_at_utc, earlier.created_at) < {TS}
                        OR (
                          COALESCE(earlier.source_created_at_utc, earlier.created_at) = {TS}
                          AND earlier.id < s.id
                        )
                      )
                ) THEN 1 ELSE 0 END), 0) AS repeat_submissions
            FROM lead_submissions s
            WHERE (? IS NULL OR {TS} >= ?)
              AND (? IS NULL OR {TS} < ?)
            "#
        );
        let row = bind_window(sqlx::query(&sql), from_utc, to_utc)
            .fetch_one(&self.pool)
            .await?;
        Ok(AnalyticsSummaryRecord {
            submissions: row.get("submissions"),
            unique_contacts: row.get("unique_contacts"),
            repeat_submissions: row.get("repeat_submissions"),
        })
    }

    pub async fn trend(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsTrendRecord>, AppError> {
        let sql = format!(
            r#"
            SELECT
                substr({TS}, 1, 10) AS day,
                COUNT(*) AS submissions,
                COUNT(DISTINCT s.lead_contact_id) AS unique_contacts,
                COALESCE(SUM(CASE WHEN EXISTS (
                    SELECT 1
                    FROM lead_submissions earlier
                    WHERE earlier.lead_contact_id = s.lead_contact_id
                      AND (
                        COALESCE(earlier.source_created_at_utc, earlier.created_at) < {TS}
                        OR (
                          COALESCE(earlier.source_created_at_utc, earlier.created_at) = {TS}
                          AND earlier.id < s.id
                        )
                      )
                ) THEN 1 ELSE 0 END), 0) AS repeat_submissions
            FROM lead_submissions s
            WHERE (? IS NULL OR {TS} >= ?)
              AND (? IS NULL OR {TS} < ?)
            GROUP BY substr({TS}, 1, 10)
            ORDER BY day ASC
            "#
        );
        let rows = bind_window(sqlx::query(&sql), from_utc, to_utc)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsTrendRecord {
                    day: row.try_get("day")?,
                    submissions: row.get("submissions"),
                    unique_contacts: row.get("unique_contacts"),
                    repeat_submissions: row.get("repeat_submissions"),
                })
            })
            .collect()
    }

    pub async fn current_statuses(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsStatusRecord>, AppError> {
        let sql = format!(
            r#"
            SELECT c.status, COUNT(*) AS contacts
            FROM lead_contacts c
            WHERE EXISTS (
                SELECT 1
                FROM lead_submissions s
                WHERE s.lead_contact_id = c.id
                  AND (? IS NULL OR {TS} >= ?)
                  AND (? IS NULL OR {TS} < ?)
            )
            GROUP BY c.status
            ORDER BY c.status ASC
            "#
        );
        let rows = bind_window(sqlx::query(&sql), from_utc, to_utc)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsStatusRecord {
                    status: row.try_get("status")?,
                    contacts: row.get("contacts"),
                })
            })
            .collect()
    }

    pub async fn country_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownRecord>, AppError> {
        let sql = format!(
            r#"
            SELECT
                COALESCE(NULLIF(UPPER(TRIM(c.country_code)), ''), 'UNKNOWN') AS key,
                COUNT(*) AS submissions,
                COUNT(DISTINCT s.lead_contact_id) AS unique_contacts
            FROM lead_submissions s
            JOIN lead_contacts c ON c.id = s.lead_contact_id
            WHERE (? IS NULL OR {TS} >= ?)
              AND (? IS NULL OR {TS} < ?)
            GROUP BY COALESCE(NULLIF(UPPER(TRIM(c.country_code)), ''), 'UNKNOWN')
            ORDER BY submissions DESC, key ASC
            "#
        );
        self.breakdown_rows(&sql, from_utc, to_utc).await
    }

    pub async fn platform_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownRecord>, AppError> {
        let sql = format!(
            r#"
            SELECT
                COALESCE(NULLIF(LOWER(TRIM(s.platform)), ''), 'unknown') AS key,
                COUNT(*) AS submissions,
                COUNT(DISTINCT s.lead_contact_id) AS unique_contacts
            FROM lead_submissions s
            WHERE (? IS NULL OR {TS} >= ?)
              AND (? IS NULL OR {TS} < ?)
            GROUP BY COALESCE(NULLIF(LOWER(TRIM(s.platform)), ''), 'unknown')
            ORDER BY submissions DESC, key ASC
            "#
        );
        self.breakdown_rows(&sql, from_utc, to_utc).await
    }

    pub async fn product_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownRecord>, AppError> {
        let sql = format!(
            r#"
            SELECT
                COALESCE(spi.product_code, 'NO_PRODUCT') AS key,
                COUNT(*) AS submissions,
                COUNT(DISTINCT s.lead_contact_id) AS unique_contacts
            FROM lead_submissions s
            LEFT JOIN submission_product_interests spi ON spi.lead_submission_id = s.id
            WHERE (? IS NULL OR {TS} >= ?)
              AND (? IS NULL OR {TS} < ?)
            GROUP BY COALESCE(spi.product_code, 'NO_PRODUCT')
            ORDER BY submissions DESC, key ASC
            "#
        );
        self.breakdown_rows(&sql, from_utc, to_utc).await
    }

    pub async fn campaign_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsNamedBreakdownRecord>, AppError> {
        self.named_breakdown("campaign_id", "campaign_name", from_utc, to_utc)
            .await
    }

    pub async fn form_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsNamedBreakdownRecord>, AppError> {
        self.named_breakdown("form_id", "form_name", from_utc, to_utc)
            .await
    }

    pub async fn adset_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsNamedBreakdownRecord>, AppError> {
        self.named_breakdown("adset_id", "adset_name", from_utc, to_utc)
            .await
    }

    pub async fn ad_breakdown(
        &self,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsNamedBreakdownRecord>, AppError> {
        self.named_breakdown("ad_id", "ad_name", from_utc, to_utc)
            .await
    }

    async fn breakdown_rows(
        &self,
        sql: &str,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownRecord>, AppError> {
        let rows = bind_window(sqlx::query(sql), from_utc, to_utc)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsBreakdownRecord {
                    key: row.try_get("key")?,
                    submissions: row.get("submissions"),
                    unique_contacts: row.get("unique_contacts"),
                })
            })
            .collect()
    }

    async fn named_breakdown(
        &self,
        id_column: &str,
        name_column: &str,
        from_utc: Option<&str>,
        to_utc: Option<&str>,
    ) -> Result<Vec<AnalyticsNamedBreakdownRecord>, AppError> {
        let key_expr = format!(
            "COALESCE(NULLIF(TRIM(s.{id_column}), ''), NULLIF(TRIM(s.{name_column}), ''), 'UNKNOWN')"
        );
        let name_expr = format!(
            "COALESCE(NULLIF(TRIM(s.{name_column}), ''), NULLIF(TRIM(s.{id_column}), ''), 'Bilinmiyor')"
        );
        let sql = format!(
            r#"
            SELECT
                {key_expr} AS key,
                {name_expr} AS name,
                COUNT(*) AS submissions,
                COUNT(DISTINCT s.lead_contact_id) AS unique_contacts
            FROM lead_submissions s
            WHERE (? IS NULL OR {TS} >= ?)
              AND (? IS NULL OR {TS} < ?)
            GROUP BY {key_expr}, {name_expr}
            ORDER BY submissions DESC, name COLLATE NOCASE ASC, key ASC
            "#
        );
        let rows = bind_window(sqlx::query(&sql), from_utc, to_utc)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsNamedBreakdownRecord {
                    key: row.try_get("key")?,
                    name: row.try_get("name")?,
                    submissions: row.get("submissions"),
                    unique_contacts: row.get("unique_contacts"),
                })
            })
            .collect()
    }
}

fn bind_window<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    from_utc: Option<&'q str>,
    to_utc: Option<&'q str>,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    query.bind(from_utc).bind(from_utc).bind(to_utc).bind(to_utc)
}
