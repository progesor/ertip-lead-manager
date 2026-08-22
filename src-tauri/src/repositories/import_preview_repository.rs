use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::importer::identity::ContactIdentity;

#[derive(Debug, Clone)]
pub struct ImportIdentitySnapshot {
    pub external_lead_ids: Vec<String>,
    pub contacts: Vec<ContactIdentity>,
}

#[derive(Clone)]
pub struct ImportPreviewRepository {
    pool: SqlitePool,
}

impl ImportPreviewRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn load_identity_snapshot(&self) -> Result<ImportIdentitySnapshot, AppError> {
        let external_lead_ids = sqlx::query_scalar::<_, String>(
            "SELECT external_lead_id FROM lead_submissions",
        )
        .fetch_all(&self.pool)
        .await?;

        let rows = sqlx::query(
            "SELECT id, normalized_email, normalized_phone FROM lead_contacts",
        )
        .fetch_all(&self.pool)
        .await?;

        let contacts = rows
            .into_iter()
            .map(|row| ContactIdentity {
                contact_id: row.get("id"),
                normalized_email: row.get("normalized_email"),
                normalized_phone: row.get("normalized_phone"),
            })
            .collect();

        Ok(ImportIdentitySnapshot {
            external_lead_ids,
            contacts,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::ImportPreviewRepository;
    use crate::db::Database;

    #[tokio::test]
    async fn loads_existing_external_ids_and_contact_identity_values() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, normalized_email, normalized_phone, status, created_at, updated_at, submission_count) VALUES (?, ?, ?, ?, 'NEW', ?, ?, 1)",
        )
        .bind("contact-a")
        .bind("Demo")
        .bind("demo@example.test")
        .bind("+905551234567")
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert contact");

        sqlx::query(
            "INSERT INTO import_batches (id, file_name, sheet_name, started_at, status, total_rows, app_version) VALUES (?, ?, ?, ?, 'COMMITTED', 1, ?)",
        )
        .bind("batch-a")
        .bind("fixture.csv")
        .bind("CSV")
        .bind(&now)
        .bind("0.1.0")
        .execute(database.pool())
        .await
        .expect("insert batch");

        sqlx::query(
            "INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_raw, raw_payload_json, created_at) VALUES (?, ?, ?, ?, ?, '{}', ?)",
        )
        .bind("submission-a")
        .bind("contact-a")
        .bind("batch-a")
        .bind("l:known")
        .bind("2026-08-21T10:00:00+03:00")
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert submission");

        let snapshot = ImportPreviewRepository::new(database.pool().clone())
            .load_identity_snapshot()
            .await
            .expect("load snapshot");

        assert_eq!(snapshot.external_lead_ids, vec!["l:known"]);
        assert_eq!(snapshot.contacts.len(), 1);
        assert_eq!(snapshot.contacts[0].contact_id, "contact-a");
        assert_eq!(
            snapshot.contacts[0].normalized_email.as_deref(),
            Some("demo@example.test")
        );
    }
}
