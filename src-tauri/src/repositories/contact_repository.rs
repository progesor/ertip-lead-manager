use chrono::{SecondsFormat, Utc};
use sqlx::{Row, SqlitePool};

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadContactRecord {
    pub id: String,
    pub display_name: Option<String>,
    pub status: String,
}

#[derive(Clone)]
pub struct ContactRepository {
    pool: SqlitePool,
}

impl ContactRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_minimal(&self, id: &str, display_name: &str) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query(
            r#"
            INSERT INTO lead_contacts (
                id, display_name, status, created_at, updated_at, submission_count
            ) VALUES (?, ?, 'NEW', ?, ?, 0)
            "#,
        )
        .bind(id)
        .bind(display_name)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<LeadContactRecord>, AppError> {
        let row = sqlx::query(
            "SELECT id, display_name, status FROM lead_contacts WHERE id = ? LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| LeadContactRecord {
            id: row.get("id"),
            display_name: row.get("display_name"),
            status: row.get("status"),
        }))
    }
}
