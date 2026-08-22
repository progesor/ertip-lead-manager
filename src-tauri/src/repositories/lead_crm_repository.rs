use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct LeadNoteRecord {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct LeadCrmRepository {
    pool: SqlitePool,
}

impl LeadCrmRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn notes(&self, contact_id: &str) -> Result<Vec<LeadNoteRecord>, AppError> {
        let notes = sqlx::query_as::<_, LeadNoteRecord>(
            r#"
            SELECT id, body, created_at, updated_at
            FROM lead_notes
            WHERE lead_contact_id = ?
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(notes)
    }

    pub async fn change_status(
        &self,
        contact_id: &str,
        new_status: &str,
        occurred_at: &str,
    ) -> Result<bool, AppError> {
        let mut transaction = self.pool.begin().await?;
        let old_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM lead_contacts WHERE id = ?",
        )
        .bind(contact_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("lead contact {contact_id}")))?;

        if old_status == new_status {
            transaction.commit().await?;
            return Ok(false);
        }

        sqlx::query("UPDATE lead_contacts SET status = ?, updated_at = ? WHERE id = ?")
            .bind(new_status)
            .bind(occurred_at)
            .bind(contact_id)
            .execute(&mut *transaction)
            .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            "STATUS_CHANGED",
            occurred_at,
            json!({
                "fromStatus": old_status,
                "toStatus": new_status,
            }),
        )
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    pub async fn create_note(
        &self,
        contact_id: &str,
        body: &str,
        occurred_at: &str,
    ) -> Result<String, AppError> {
        let mut transaction = self.pool.begin().await?;
        ensure_contact_exists(&mut transaction, contact_id).await?;

        let note_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO lead_notes (id, lead_contact_id, body, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&note_id)
        .bind(contact_id)
        .bind(body)
        .bind(occurred_at)
        .bind(occurred_at)
        .execute(&mut *transaction)
        .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            "NOTE_CREATED",
            occurred_at,
            json!({ "noteId": note_id.clone() }),
        )
        .await?;

        transaction.commit().await?;
        Ok(note_id)
    }

    pub async fn update_note(
        &self,
        contact_id: &str,
        note_id: &str,
        body: &str,
        occurred_at: &str,
    ) -> Result<bool, AppError> {
        let mut transaction = self.pool.begin().await?;
        let old_body = sqlx::query_scalar::<_, String>(
            "SELECT body FROM lead_notes WHERE id = ? AND lead_contact_id = ?",
        )
        .bind(note_id)
        .bind(contact_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("lead note {note_id}")))?;

        if old_body == body {
            transaction.commit().await?;
            return Ok(false);
        }

        sqlx::query(
            "UPDATE lead_notes SET body = ?, updated_at = ? WHERE id = ? AND lead_contact_id = ?",
        )
        .bind(body)
        .bind(occurred_at)
        .bind(note_id)
        .bind(contact_id)
        .execute(&mut *transaction)
        .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            "NOTE_UPDATED",
            occurred_at,
            json!({ "noteId": note_id }),
        )
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    pub async fn delete_note(
        &self,
        contact_id: &str,
        note_id: &str,
        occurred_at: &str,
    ) -> Result<(), AppError> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM lead_notes WHERE id = ? AND lead_contact_id = ?",
        )
        .bind(note_id)
        .bind(contact_id)
        .fetch_one(&mut *transaction)
        .await?;

        if exists == 0 {
            return Err(AppError::NotFound(format!("lead note {note_id}")));
        }

        sqlx::query("DELETE FROM lead_notes WHERE id = ? AND lead_contact_id = ?")
            .bind(note_id)
            .bind(contact_id)
            .execute(&mut *transaction)
            .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            "NOTE_DELETED",
            occurred_at,
            json!({ "noteId": note_id }),
        )
        .await?;

        transaction.commit().await?;
        Ok(())
    }
}

async fn ensure_contact_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    contact_id: &str,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM lead_contacts WHERE id = ?")
        .bind(contact_id)
        .fetch_one(&mut **transaction)
        .await?;

    if exists == 0 {
        return Err(AppError::NotFound(format!("lead contact {contact_id}")));
    }

    Ok(())
}

async fn insert_activity(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    contact_id: &str,
    activity_type: &str,
    occurred_at: &str,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO lead_activities (id, lead_contact_id, activity_type, occurred_at, payload_json) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(contact_id)
    .bind(activity_type)
    .bind(occurred_at)
    .bind(payload.to_string())
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::LeadCrmRepository;
    use crate::db::Database;

    async fn seed_contact(database: &Database) -> String {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let contact_id = "crm-contact".to_string();
        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, status, created_at, updated_at, submission_count) VALUES (?, 'CRM Test', 'NEW', ?, ?, 0)",
        )
        .bind(&contact_id)
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("seed contact");
        contact_id
    }

    #[tokio::test]
    async fn status_change_persists_and_creates_activity() {
        let database = Database::connect_memory().await.expect("open database");
        let contact_id = seed_contact(&database).await;
        let repository = LeadCrmRepository::new(database.pool().clone());
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        assert!(repository
            .change_status(&contact_id, "QUALIFIED", &now)
            .await
            .expect("change status"));

        let status: String = sqlx::query_scalar("SELECT status FROM lead_contacts WHERE id = ?")
            .bind(&contact_id)
            .fetch_one(database.pool())
            .await
            .expect("read status");
        let activity: String = sqlx::query_scalar(
            "SELECT activity_type FROM lead_activities WHERE lead_contact_id = ? ORDER BY occurred_at DESC LIMIT 1",
        )
        .bind(&contact_id)
        .fetch_one(database.pool())
        .await
        .expect("read activity");

        assert_eq!(status, "QUALIFIED");
        assert_eq!(activity, "STATUS_CHANGED");
    }

    #[tokio::test]
    async fn note_create_update_delete_round_trip_creates_audit_events() {
        let database = Database::connect_memory().await.expect("open database");
        let contact_id = seed_contact(&database).await;
        let repository = LeadCrmRepository::new(database.pool().clone());
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        let note_id = repository
            .create_note(&contact_id, "İlk not", &now)
            .await
            .expect("create note");
        assert!(repository
            .update_note(&contact_id, &note_id, "Güncel not", &now)
            .await
            .expect("update note"));

        let notes = repository.notes(&contact_id).await.expect("list notes");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].body, "Güncel not");

        repository
            .delete_note(&contact_id, &note_id, &now)
            .await
            .expect("delete note");

        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lead_notes WHERE id = ?")
            .bind(&note_id)
            .fetch_one(database.pool())
            .await
            .expect("count notes");
        let activity_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lead_activities WHERE lead_contact_id = ? AND activity_type IN ('NOTE_CREATED', 'NOTE_UPDATED', 'NOTE_DELETED')",
        )
        .bind(&contact_id)
        .fetch_one(database.pool())
        .await
        .expect("count activities");

        assert_eq!(remaining, 0);
        assert_eq!(activity_count, 3);
    }
}
