use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct FollowUpRecord {
    pub id: String,
    pub lead_contact_id: String,
    pub due_at: String,
    pub status: String,
    pub note: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone)]
pub struct FollowUpRepository {
    pool: SqlitePool,
}

impl FollowUpRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_for_contact(&self, contact_id: &str) -> Result<Vec<FollowUpRecord>, AppError> {
        let rows = sqlx::query_as::<_, FollowUpRecord>(
            r#"
            SELECT id, lead_contact_id, due_at, status, note, created_at, completed_at
            FROM follow_ups
            WHERE lead_contact_id = ?
            ORDER BY status = 'OPEN' DESC,
                     CASE WHEN status = 'OPEN' THEN due_at END ASC,
                     created_at DESC,
                     id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn create(
        &self,
        contact_id: &str,
        due_at: &str,
        note: Option<&str>,
        occurred_at: &str,
    ) -> Result<String, AppError> {
        let mut transaction = self.pool.begin().await?;
        ensure_contact_exists(&mut transaction, contact_id).await?;

        let follow_up_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO follow_ups (
                id, lead_contact_id, due_at, status, note, created_at, completed_at
            ) VALUES (?, ?, ?, 'OPEN', ?, ?, NULL)
            "#,
        )
        .bind(&follow_up_id)
        .bind(contact_id)
        .bind(due_at)
        .bind(note)
        .bind(occurred_at)
        .execute(&mut *transaction)
        .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            "FOLLOW_UP_CREATED",
            occurred_at,
            json!({
                "followUpId": follow_up_id.clone(),
                "dueAt": due_at,
            }),
        )
        .await?;

        transaction.commit().await?;
        Ok(follow_up_id)
    }

    pub async fn reschedule(
        &self,
        contact_id: &str,
        follow_up_id: &str,
        due_at: &str,
        note: Option<&str>,
        occurred_at: &str,
    ) -> Result<bool, AppError> {
        let mut transaction = self.pool.begin().await?;
        let current = sqlx::query_as::<_, FollowUpRecord>(
            r#"
            SELECT id, lead_contact_id, due_at, status, note, created_at, completed_at
            FROM follow_ups
            WHERE id = ? AND lead_contact_id = ?
            "#,
        )
        .bind(follow_up_id)
        .bind(contact_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("follow-up {follow_up_id}")))?;

        if current.status != "OPEN" {
            return Err(AppError::Validation(format!(
                "only OPEN follow-ups can be rescheduled; current status is {}",
                current.status
            )));
        }

        if current.due_at == due_at && current.note.as_deref() == note {
            transaction.commit().await?;
            return Ok(false);
        }

        sqlx::query(
            "UPDATE follow_ups SET due_at = ?, note = ? WHERE id = ? AND lead_contact_id = ?",
        )
        .bind(due_at)
        .bind(note)
        .bind(follow_up_id)
        .bind(contact_id)
        .execute(&mut *transaction)
        .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            "FOLLOW_UP_RESCHEDULED",
            occurred_at,
            json!({
                "followUpId": follow_up_id,
                "fromDueAt": current.due_at,
                "toDueAt": due_at,
            }),
        )
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    pub async fn complete(
        &self,
        contact_id: &str,
        follow_up_id: &str,
        occurred_at: &str,
    ) -> Result<bool, AppError> {
        self.set_terminal_status(
            contact_id,
            follow_up_id,
            "COMPLETED",
            "FOLLOW_UP_COMPLETED",
            Some(occurred_at),
            occurred_at,
        )
        .await
    }

    pub async fn cancel(
        &self,
        contact_id: &str,
        follow_up_id: &str,
        occurred_at: &str,
    ) -> Result<bool, AppError> {
        self.set_terminal_status(
            contact_id,
            follow_up_id,
            "CANCELLED",
            "FOLLOW_UP_CANCELLED",
            None,
            occurred_at,
        )
        .await
    }

    async fn set_terminal_status(
        &self,
        contact_id: &str,
        follow_up_id: &str,
        target_status: &str,
        activity_type: &str,
        completed_at: Option<&str>,
        occurred_at: &str,
    ) -> Result<bool, AppError> {
        let mut transaction = self.pool.begin().await?;
        let current_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM follow_ups WHERE id = ? AND lead_contact_id = ?",
        )
        .bind(follow_up_id)
        .bind(contact_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("follow-up {follow_up_id}")))?;

        if current_status == target_status {
            transaction.commit().await?;
            return Ok(false);
        }

        if current_status != "OPEN" {
            return Err(AppError::Validation(format!(
                "only OPEN follow-ups can change state; current status is {current_status}"
            )));
        }

        sqlx::query(
            "UPDATE follow_ups SET status = ?, completed_at = ? WHERE id = ? AND lead_contact_id = ?",
        )
        .bind(target_status)
        .bind(completed_at)
        .bind(follow_up_id)
        .bind(contact_id)
        .execute(&mut *transaction)
        .await?;

        insert_activity(
            &mut transaction,
            contact_id,
            activity_type,
            occurred_at,
            json!({
                "followUpId": follow_up_id,
                "status": target_status,
            }),
        )
        .await?;

        transaction.commit().await?;
        Ok(true)
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

    use super::FollowUpRepository;
    use crate::db::Database;

    async fn seed_contact(database: &Database) -> String {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let contact_id = "follow-up-contact".to_string();
        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, status, created_at, updated_at, submission_count) VALUES (?, 'Follow-up Test', 'NEW', ?, ?, 0)",
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
    async fn follow_up_create_reschedule_complete_round_trip_is_audited() {
        let database = Database::connect_memory().await.expect("open database");
        let contact_id = seed_contact(&database).await;
        let repository = FollowUpRepository::new(database.pool().clone());
        let now = "2026-08-22T09:00:00.000Z";
        let first_due = "2026-08-23T09:00:00.000Z";
        let second_due = "2026-08-24T11:30:00.000Z";

        let id = repository
            .create(&contact_id, first_due, Some("İlk arama"), now)
            .await
            .expect("create follow-up");
        assert!(repository
            .reschedule(&contact_id, &id, second_due, Some("Öğleden sonra ara"), now)
            .await
            .expect("reschedule follow-up"));
        assert!(repository
            .complete(&contact_id, &id, now)
            .await
            .expect("complete follow-up"));

        let rows = repository
            .list_for_contact(&contact_id)
            .await
            .expect("list follow-ups");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].due_at, second_due);
        assert_eq!(rows[0].status, "COMPLETED");
        assert_eq!(rows[0].completed_at.as_deref(), Some(now));

        let activity_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lead_activities WHERE lead_contact_id = ? AND activity_type LIKE 'FOLLOW_UP_%'",
        )
        .bind(&contact_id)
        .fetch_one(database.pool())
        .await
        .expect("count follow-up activities");
        assert_eq!(activity_count, 3);
    }

    #[tokio::test]
    async fn cancelled_follow_up_cannot_be_rescheduled() {
        let database = Database::connect_memory().await.expect("open database");
        let contact_id = seed_contact(&database).await;
        let repository = FollowUpRepository::new(database.pool().clone());
        let now = "2026-08-22T09:00:00.000Z";
        let due = "2026-08-23T09:00:00.000Z";

        let id = repository
            .create(&contact_id, due, None, now)
            .await
            .expect("create follow-up");
        assert!(repository
            .cancel(&contact_id, &id, now)
            .await
            .expect("cancel follow-up"));

        let result = repository
            .reschedule(&contact_id, &id, "2026-08-24T09:00:00.000Z", None, now)
            .await;
        assert!(matches!(result, Err(crate::error::AppError::Validation(_))));
    }
}
