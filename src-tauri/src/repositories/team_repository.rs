use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct StaffMemberRecord {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub auth_subject: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct AssigneeRecord {
    pub id: String,
    pub display_name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentChange {
    pub changed: bool,
    pub assignee: Option<AssigneeRecord>,
}

#[derive(Clone)]
pub struct TeamRepository {
    pool: SqlitePool,
}

impl TeamRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_staff(&self, include_inactive: bool) -> Result<Vec<StaffMemberRecord>, AppError> {
        let sql = if include_inactive {
            r#"
            SELECT id, display_name, email, role, is_active, auth_subject, created_at, updated_at
            FROM app_users
            ORDER BY is_active DESC, display_name COLLATE NOCASE ASC, id ASC
            "#
        } else {
            r#"
            SELECT id, display_name, email, role, is_active, auth_subject, created_at, updated_at
            FROM app_users
            WHERE is_active = 1
            ORDER BY display_name COLLATE NOCASE ASC, id ASC
            "#
        };
        Ok(sqlx::query_as::<_, StaffMemberRecord>(sql)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_staff(&self, user_id: &str) -> Result<Option<StaffMemberRecord>, AppError> {
        Ok(sqlx::query_as::<_, StaffMemberRecord>(
            r#"
            SELECT id, display_name, email, role, is_active, auth_subject, created_at, updated_at
            FROM app_users
            WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_staff(
        &self,
        display_name: &str,
        email: Option<&str>,
        role: &str,
        occurred_at: &str,
    ) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO app_users (id, display_name, email, role, is_active, created_at, updated_at)
            VALUES (?, ?, ?, ?, 1, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(display_name)
        .bind(email)
        .bind(role)
        .bind(occurred_at)
        .bind(occurred_at)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn update_staff(
        &self,
        user_id: &str,
        display_name: &str,
        email: Option<&str>,
        role: &str,
        occurred_at: &str,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE app_users SET display_name = ?, email = ?, role = ?, updated_at = ? WHERE id = ?",
        )
        .bind(display_name)
        .bind(email)
        .bind(role)
        .bind(occurred_at)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("staff member".to_string()));
        }
        Ok(())
    }

    pub async fn set_staff_active(
        &self,
        user_id: &str,
        is_active: bool,
        occurred_at: &str,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE app_users SET is_active = ?, updated_at = ? WHERE id = ?",
        )
        .bind(is_active)
        .bind(occurred_at)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("staff member".to_string()));
        }
        Ok(())
    }

    pub async fn contact_assignee(&self, contact_id: &str) -> Result<Option<AssigneeRecord>, AppError> {
        Ok(sqlx::query_as::<_, AssigneeRecord>(
            r#"
            SELECT u.id, u.display_name, u.is_active
            FROM lead_contacts c
            JOIN app_users u ON u.id = c.assigned_user_id
            WHERE c.id = ?
            "#,
        )
        .bind(contact_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn assign_lead(
        &self,
        contact_id: &str,
        new_user_id: Option<&str>,
        actor_user_id: Option<&str>,
        occurred_at: &str,
    ) -> Result<AssignmentChange, AppError> {
        let mut tx = self.pool.begin().await?;

        let contact_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM lead_contacts WHERE id = ?",
        )
        .bind(contact_id)
        .fetch_one(&mut *tx)
        .await?;
        if contact_exists == 0 {
            return Err(AppError::NotFound("lead contact".to_string()));
        }

        let old_assignee = sqlx::query_as::<_, AssigneeRecord>(
            r#"
            SELECT u.id, u.display_name, u.is_active
            FROM lead_contacts c
            JOIN app_users u ON u.id = c.assigned_user_id
            WHERE c.id = ?
            "#,
        )
        .bind(contact_id)
        .fetch_optional(&mut *tx)
        .await?;

        let new_assignee = match new_user_id {
            Some(user_id) => {
                let user = sqlx::query_as::<_, AssigneeRecord>(
                    "SELECT id, display_name, is_active FROM app_users WHERE id = ?",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound("staff member".to_string()))?;
                if !user.is_active {
                    return Err(AppError::Validation(
                        "inactive staff member cannot receive new assignments".to_string(),
                    ));
                }
                Some(user)
            }
            None => None,
        };

        if old_assignee.as_ref().map(|item| item.id.as_str())
            == new_assignee.as_ref().map(|item| item.id.as_str())
        {
            tx.commit().await?;
            return Ok(AssignmentChange {
                changed: false,
                assignee: new_assignee,
            });
        }

        sqlx::query(
            "UPDATE lead_contacts SET assigned_user_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(new_assignee.as_ref().map(|item| item.id.as_str()))
        .bind(occurred_at)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;

        let activity_id = Uuid::new_v4().to_string();
        let payload = json!({
            "fromUserId": old_assignee.as_ref().map(|item| item.id.as_str()),
            "fromDisplayName": old_assignee.as_ref().map(|item| item.display_name.as_str()),
            "toUserId": new_assignee.as_ref().map(|item| item.id.as_str()),
            "toDisplayName": new_assignee.as_ref().map(|item| item.display_name.as_str()),
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO lead_activities (
                id, lead_contact_id, activity_type, occurred_at, payload_json, actor_user_id
            ) VALUES (?, ?, 'ASSIGNEE_CHANGED', ?, ?, ?)
            "#,
        )
        .bind(activity_id)
        .bind(contact_id)
        .bind(occurred_at)
        .bind(payload)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(AssignmentChange {
            changed: true,
            assignee: new_assignee,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::TeamRepository;
    use crate::db::Database;

    #[tokio::test]
    async fn assignment_persists_and_creates_audited_activity_without_hard_delete() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        sqlx::query("INSERT INTO lead_contacts (id, display_name, status, created_at, updated_at, submission_count) VALUES ('lead-team', 'Team Lead', 'NEW', ?, ?, 0)")
            .bind(&now)
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("seed contact");

        let repository = TeamRepository::new(database.pool().clone());
        let user_id = repository
            .create_staff("Ayşe Test", Some("ayse@example.test"), "SALES", &now)
            .await
            .expect("create staff");

        let changed = repository
            .assign_lead("lead-team", Some(&user_id), None, &now)
            .await
            .expect("assign lead");
        assert!(changed.changed);
        assert_eq!(changed.assignee.as_ref().map(|item| item.display_name.as_str()), Some("Ayşe Test"));

        let activity_type = sqlx::query_scalar::<_, String>(
            "SELECT activity_type FROM lead_activities WHERE lead_contact_id = 'lead-team'",
        )
        .fetch_one(database.pool())
        .await
        .expect("read activity");
        assert_eq!(activity_type, "ASSIGNEE_CHANGED");

        repository
            .set_staff_active(&user_id, false, &now)
            .await
            .expect("deactivate staff");
        let assignee = repository
            .contact_assignee("lead-team")
            .await
            .expect("read assignee")
            .expect("assigned staff remains linked");
        assert!(!assignee.is_active);
    }
}
