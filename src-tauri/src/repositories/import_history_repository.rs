use sqlx::{FromRow, SqlitePool};

use crate::error::AppError;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ImportHistoryRecord {
    pub id: String,
    pub file_name: String,
    pub file_format: String,
    pub sheet_name: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub total_rows: i64,
    pub new_submissions: i64,
    pub exact_duplicates: i64,
    pub repeat_candidates: i64,
    pub warning_count: i64,
    pub error_count: i64,
    pub app_version: String,
}

#[derive(Clone)]
pub struct ImportHistoryRepository {
    pool: SqlitePool,
}

impl ImportHistoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<ImportHistoryRecord>, AppError> {
        let rows = sqlx::query_as::<_, ImportHistoryRecord>(
            r#"
            SELECT
                id,
                file_name,
                file_format,
                sheet_name,
                completed_at,
                status,
                total_rows,
                new_submissions,
                exact_duplicates,
                repeat_candidates,
                warning_count,
                error_count,
                app_version
            FROM import_batches
            ORDER BY COALESCE(completed_at, started_at) DESC, started_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::ImportHistoryRepository;
    use crate::db::Database;

    #[tokio::test]
    async fn returns_recent_batches_newest_first_with_persisted_format() {
        let database = Database::connect_memory().await.expect("open database");
        let older = "2026-08-20T10:00:00.000Z";
        let newer = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        for (id, file, timestamp) in [
            ("batch-old", "old.csv", older.to_string()),
            ("batch-new", "new.xlsx", newer),
        ] {
            sqlx::query(
                "INSERT INTO import_batches (id, file_name, sheet_name, started_at, completed_at, status, total_rows, new_submissions, exact_duplicates, repeat_candidates, warning_count, error_count, app_version) VALUES (?, ?, 'Sheet1', ?, ?, 'COMMITTED', 10, 8, 2, 1, 0, 0, '0.1.0')",
            )
            .bind(id)
            .bind(file)
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(database.pool())
            .await
            .expect("insert batch");
        }

        let rows = ImportHistoryRepository::new(database.pool().clone())
            .list_recent(10)
            .await
            .expect("list history");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "batch-new");
        assert_eq!(rows[0].file_format, "XLSX");
        assert_eq!(rows[1].id, "batch-old");
        assert_eq!(rows[1].file_format, "CSV");
    }
}
