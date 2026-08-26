use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, Row};
use uuid::Uuid;

use crate::{
    authz::{Action, Actor, LeadScope},
    crm::CrmError,
};

const MAX_FOLLOW_UP_NOTE_CHARS: usize = 1_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpItem {
    pub id: String,
    pub lead_contact_id: String,
    pub due_at: DateTime<Utc>,
    pub status: String,
    pub note: Option<String>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFollowUpRequest {
    pub due_at: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RescheduleFollowUpRequest {
    pub due_at: String,
    pub note: Option<String>,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpTransitionRequest {
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpMutationResult {
    pub changed: bool,
    pub follow_up: FollowUpItem,
}

#[derive(Clone)]
pub struct FollowUpService {
    pool: PgPool,
}

impl FollowUpService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_for_contact(
        &self,
        actor: &Actor,
        contact_id: &str,
    ) -> Result<Vec<FollowUpItem>, CrmError> {
        actor.require(Action::LeadRead)?;
        let contact_id = required_id(contact_id, "contactId")?;
        ensure_read_scope(&self.pool, actor, contact_id).await?;

        let rows = sqlx::query(
            r#"
            SELECT id, lead_contact_id, due_at, status, note, revision,
                   created_at, updated_at, completed_at
            FROM follow_ups
            WHERE lead_contact_id = $1
            ORDER BY (status = 'OPEN') DESC,
                     CASE WHEN status = 'OPEN' THEN due_at END ASC NULLS LAST,
                     created_at DESC,
                     id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_follow_up).collect()
    }

    pub async fn create(
        &self,
        actor: &Actor,
        contact_id: &str,
        request: CreateFollowUpRequest,
    ) -> Result<FollowUpItem, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        let due_at = canonical_utc(&request.due_at)?;
        let note = clean_note(request.note)?;
        let mut tx = self.pool.begin().await?;
        ensure_write_scope(&mut tx, actor, contact_id).await?;

        let follow_up_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO follow_ups (
                id, lead_contact_id, due_at, status, note, revision,
                created_at, updated_at, completed_at
            ) VALUES ($1, $2, $3, 'OPEN', $4, 0, $5, $5, NULL)
            "#,
        )
        .bind(&follow_up_id)
        .bind(contact_id)
        .bind(due_at)
        .bind(note.as_deref())
        .bind(now)
        .execute(&mut *tx)
        .await?;

        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "FOLLOW_UP_CREATED",
            json!({
                "followUpId": follow_up_id.clone(),
                "dueAt": due_at.to_rfc3339(),
            })
            .to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(FollowUpItem {
            id: follow_up_id,
            lead_contact_id: contact_id.to_string(),
            due_at,
            status: "OPEN".to_string(),
            note,
            revision: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
        })
    }

    pub async fn reschedule(
        &self,
        actor: &Actor,
        contact_id: &str,
        follow_up_id: &str,
        request: RescheduleFollowUpRequest,
    ) -> Result<FollowUpMutationResult, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        let follow_up_id = required_id(follow_up_id, "followUpId")?;
        validate_expected_revision(request.expected_revision)?;
        let due_at = canonical_utc(&request.due_at)?;
        let note = clean_note(request.note)?;
        let mut tx = self.pool.begin().await?;
        ensure_write_scope(&mut tx, actor, contact_id).await?;
        let current = lock_follow_up(&mut tx, contact_id, follow_up_id).await?;

        if current.revision != request.expected_revision {
            return Err(CrmError::Conflict {
                resource: "follow-up".to_string(),
                current_revision: current.revision,
            });
        }
        if current.status != "OPEN" {
            return Err(CrmError::Validation(format!(
                "only OPEN follow-ups can be rescheduled; current status is {}",
                current.status
            )));
        }
        if current.due_at == due_at && current.note == note {
            tx.commit().await?;
            return Ok(FollowUpMutationResult {
                changed: false,
                follow_up: current,
            });
        }

        let next_revision = current.revision + 1;
        let updated_at = Utc::now();
        sqlx::query(
            r#"
            UPDATE follow_ups
            SET due_at = $1, note = $2, revision = $3, updated_at = $4
            WHERE id = $5 AND lead_contact_id = $6
            "#,
        )
        .bind(due_at)
        .bind(note.as_deref())
        .bind(next_revision)
        .bind(updated_at)
        .bind(follow_up_id)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;

        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "FOLLOW_UP_RESCHEDULED",
            json!({
                "followUpId": follow_up_id,
                "fromDueAt": current.due_at.to_rfc3339(),
                "toDueAt": due_at.to_rfc3339(),
            })
            .to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(FollowUpMutationResult {
            changed: true,
            follow_up: FollowUpItem {
                id: current.id,
                lead_contact_id: current.lead_contact_id,
                due_at,
                status: current.status,
                note,
                revision: next_revision,
                created_at: current.created_at,
                updated_at,
                completed_at: current.completed_at,
            },
        })
    }

    pub async fn complete(
        &self,
        actor: &Actor,
        contact_id: &str,
        follow_up_id: &str,
        request: FollowUpTransitionRequest,
    ) -> Result<FollowUpMutationResult, CrmError> {
        self.set_terminal_status(
            actor,
            contact_id,
            follow_up_id,
            request.expected_revision,
            "COMPLETED",
            "FOLLOW_UP_COMPLETED",
            true,
        )
        .await
    }

    pub async fn cancel(
        &self,
        actor: &Actor,
        contact_id: &str,
        follow_up_id: &str,
        request: FollowUpTransitionRequest,
    ) -> Result<FollowUpMutationResult, CrmError> {
        self.set_terminal_status(
            actor,
            contact_id,
            follow_up_id,
            request.expected_revision,
            "CANCELLED",
            "FOLLOW_UP_CANCELLED",
            false,
        )
        .await
    }

    async fn set_terminal_status(
        &self,
        actor: &Actor,
        contact_id: &str,
        follow_up_id: &str,
        expected_revision: i64,
        target_status: &str,
        activity_type: &str,
        mark_completed_at: bool,
    ) -> Result<FollowUpMutationResult, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        let follow_up_id = required_id(follow_up_id, "followUpId")?;
        validate_expected_revision(expected_revision)?;
        let mut tx = self.pool.begin().await?;
        ensure_write_scope(&mut tx, actor, contact_id).await?;
        let current = lock_follow_up(&mut tx, contact_id, follow_up_id).await?;

        if current.revision != expected_revision {
            return Err(CrmError::Conflict {
                resource: "follow-up".to_string(),
                current_revision: current.revision,
            });
        }
        if current.status == target_status {
            tx.commit().await?;
            return Ok(FollowUpMutationResult {
                changed: false,
                follow_up: current,
            });
        }
        if current.status != "OPEN" {
            return Err(CrmError::Validation(format!(
                "only OPEN follow-ups can change state; current status is {}",
                current.status
            )));
        }

        let now = Utc::now();
        let next_revision = current.revision + 1;
        let completed_at = mark_completed_at.then_some(now);
        sqlx::query(
            r#"
            UPDATE follow_ups
            SET status = $1, completed_at = $2, revision = $3, updated_at = $4
            WHERE id = $5 AND lead_contact_id = $6
            "#,
        )
        .bind(target_status)
        .bind(completed_at)
        .bind(next_revision)
        .bind(now)
        .bind(follow_up_id)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;

        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            activity_type,
            json!({
                "followUpId": follow_up_id,
                "status": target_status,
            })
            .to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(FollowUpMutationResult {
            changed: true,
            follow_up: FollowUpItem {
                id: current.id,
                lead_contact_id: current.lead_contact_id,
                due_at: current.due_at,
                status: target_status.to_string(),
                note: current.note,
                revision: next_revision,
                created_at: current.created_at,
                updated_at: now,
                completed_at,
            },
        })
    }
}

async fn ensure_read_scope(
    pool: &PgPool,
    actor: &Actor,
    contact_id: &str,
) -> Result<(), CrmError> {
    let row = sqlx::query("SELECT assigned_user_id FROM lead_contacts WHERE id = $1")
        .bind(contact_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| CrmError::NotFound("lead contact".to_string()))?;
    let assigned_user_id: Option<String> = row.try_get("assigned_user_id")?;
    if let LeadScope::AssignedTo(user_id) = actor.lead_scope() {
        if assigned_user_id.as_deref() != Some(user_id) {
            return Err(CrmError::NotFound("lead contact".to_string()));
        }
    }
    Ok(())
}

async fn ensure_write_scope(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    actor: &Actor,
    contact_id: &str,
) -> Result<(), CrmError> {
    let row = sqlx::query(
        "SELECT assigned_user_id FROM lead_contacts WHERE id = $1 FOR SHARE",
    )
    .bind(contact_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| CrmError::NotFound("lead contact".to_string()))?;
    let assigned_user_id: Option<String> = row.try_get("assigned_user_id")?;
    if let LeadScope::AssignedTo(user_id) = actor.lead_scope() {
        if assigned_user_id.as_deref() != Some(user_id) {
            return Err(CrmError::NotFound("lead contact".to_string()));
        }
    }
    Ok(())
}

async fn lock_follow_up(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    contact_id: &str,
    follow_up_id: &str,
) -> Result<FollowUpItem, CrmError> {
    let row = sqlx::query(
        r#"
        SELECT id, lead_contact_id, due_at, status, note, revision,
               created_at, updated_at, completed_at
        FROM follow_ups
        WHERE id = $1 AND lead_contact_id = $2
        FOR UPDATE
        "#,
    )
    .bind(follow_up_id)
    .bind(contact_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| CrmError::NotFound("follow-up".to_string()))?;
    map_follow_up(row)
}

fn map_follow_up(row: sqlx::postgres::PgRow) -> Result<FollowUpItem, CrmError> {
    Ok(FollowUpItem {
        id: row.try_get("id")?,
        lead_contact_id: row.try_get("lead_contact_id")?,
        due_at: row.try_get("due_at")?,
        status: row.try_get("status")?,
        note: row.try_get("note")?,
        revision: row.try_get("revision")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        completed_at: row.try_get("completed_at")?,
    })
}

fn canonical_utc(value: &str) -> Result<DateTime<Utc>, CrmError> {
    let value = required_id(value, "dueAt")?;
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| CrmError::Validation("follow-up dueAt must be RFC3339".to_string()))?;
    Ok(parsed.with_timezone(&Utc))
}

fn clean_note(value: Option<String>) -> Result<Option<String>, CrmError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_FOLLOW_UP_NOTE_CHARS {
        return Err(CrmError::Validation(format!(
            "follow-up note exceeds {MAX_FOLLOW_UP_NOTE_CHARS} characters"
        )));
    }
    Ok(Some(value.to_string()))
}

fn validate_expected_revision(revision: i64) -> Result<(), CrmError> {
    if revision < 0 {
        return Err(CrmError::Validation(
            "expectedRevision must be zero or greater".to_string(),
        ));
    }
    Ok(())
}

fn required_id<'a>(value: &'a str, field: &str) -> Result<&'a str, CrmError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CrmError::Validation(format!("{field} is required")));
    }
    Ok(value)
}

async fn insert_activity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    contact_id: &str,
    actor_user_id: &str,
    activity_type: &str,
    payload_json: String,
) -> Result<(), CrmError> {
    sqlx::query(
        r#"
        INSERT INTO lead_activities (
            id, lead_contact_id, actor_user_id, activity_type, occurred_at, payload_json
        ) VALUES ($1, $2, $3, $4, now(), $5)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(contact_id)
    .bind(actor_user_id)
    .bind(activity_type)
    .bind(payload_json)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use sqlx::postgres::PgPoolOptions;

    use super::{
        CreateFollowUpRequest, FollowUpService, FollowUpTransitionRequest,
        RescheduleFollowUpRequest, canonical_utc, clean_note,
    };
    use crate::{
        authz::{Actor, Role},
        crm::CrmError,
        db::run_migrations,
    };

    #[test]
    fn due_time_is_canonicalized_to_utc_and_blank_note_is_none() {
        assert_eq!(
            canonical_utc("2026-08-22T15:30:00+03:00").expect("canonical due time"),
            Utc.with_ymd_and_hms(2026, 8, 22, 12, 30, 0)
                .single()
                .expect("UTC timestamp")
        );
        assert_eq!(clean_note(Some("   ".to_string())).expect("clean note"), None);
        assert!(matches!(
            canonical_utc("tomorrow"),
            Err(CrmError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn follow_up_lifecycle_is_scoped_audited_and_revision_safe() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping follow-up integration test");
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect PostgreSQL test database");
        run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM lead_activities WHERE lead_contact_id = 'follow-api-test-lead'")
            .execute(&pool)
            .await
            .expect("cleanup activities");
        sqlx::query("DELETE FROM lead_contacts WHERE id = 'follow-api-test-lead'")
            .execute(&pool)
            .await
            .expect("cleanup lead");
        sqlx::query("DELETE FROM app_users WHERE id LIKE 'follow-api-test-%'")
            .execute(&pool)
            .await
            .expect("cleanup users");

        for (id, name) in [
            ("follow-api-test-sales-a", "Follow Sales A"),
            ("follow-api-test-sales-b", "Follow Sales B"),
        ] {
            sqlx::query(
                "INSERT INTO app_users (id, display_name, role, is_active, created_at, updated_at) VALUES ($1, $2, 'SALES', TRUE, now(), now())",
            )
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .expect("seed user");
        }
        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, status, assigned_user_id, created_at, updated_at, submission_count) VALUES ('follow-api-test-lead', 'Follow Lead', 'NEW', 'follow-api-test-sales-a', now(), now(), 0)",
        )
        .execute(&pool)
        .await
        .expect("seed lead");

        let service = FollowUpService::new(pool.clone());
        let sales_a = Actor {
            user_id: "follow-api-test-sales-a".to_string(),
            role: Role::Sales,
        };
        let sales_b = Actor {
            user_id: "follow-api-test-sales-b".to_string(),
            role: Role::Sales,
        };

        let created = service
            .create(
                &sales_a,
                "follow-api-test-lead",
                CreateFollowUpRequest {
                    due_at: "2026-08-23T12:00:00+03:00".to_string(),
                    note: Some("İlk arama".to_string()),
                },
            )
            .await
            .expect("create follow-up");
        assert_eq!(created.revision, 0);
        assert_eq!(created.status, "OPEN");

        let rescheduled = service
            .reschedule(
                &sales_a,
                "follow-api-test-lead",
                &created.id,
                RescheduleFollowUpRequest {
                    due_at: "2026-08-24T14:30:00+03:00".to_string(),
                    note: Some("Öğleden sonra ara".to_string()),
                    expected_revision: 0,
                },
            )
            .await
            .expect("reschedule follow-up");
        assert!(rescheduled.changed);
        assert_eq!(rescheduled.follow_up.revision, 1);

        assert!(matches!(
            service
                .complete(
                    &sales_a,
                    "follow-api-test-lead",
                    &created.id,
                    FollowUpTransitionRequest {
                        expected_revision: 0,
                    },
                )
                .await,
            Err(CrmError::Conflict {
                current_revision: 1,
                ..
            })
        ));

        let completed = service
            .complete(
                &sales_a,
                "follow-api-test-lead",
                &created.id,
                FollowUpTransitionRequest {
                    expected_revision: 1,
                },
            )
            .await
            .expect("complete follow-up");
        assert!(completed.changed);
        assert_eq!(completed.follow_up.status, "COMPLETED");
        assert_eq!(completed.follow_up.revision, 2);
        assert!(completed.follow_up.completed_at.is_some());

        let no_op = service
            .complete(
                &sales_a,
                "follow-api-test-lead",
                &created.id,
                FollowUpTransitionRequest {
                    expected_revision: 2,
                },
            )
            .await
            .expect("idempotent complete");
        assert!(!no_op.changed);

        assert!(matches!(
            service
                .cancel(
                    &sales_a,
                    "follow-api-test-lead",
                    &created.id,
                    FollowUpTransitionRequest {
                        expected_revision: 2,
                    },
                )
                .await,
            Err(CrmError::Validation(_))
        ));
        assert!(matches!(
            service
                .list_for_contact(&sales_b, "follow-api-test-lead")
                .await,
            Err(CrmError::NotFound(_))
        ));

        let items = service
            .list_for_contact(&sales_a, "follow-api-test-lead")
            .await
            .expect("list follow-ups");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "COMPLETED");

        let activity_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM lead_activities WHERE lead_contact_id = 'follow-api-test-lead' AND actor_user_id = 'follow-api-test-sales-a' AND activity_type LIKE 'FOLLOW_UP_%'",
        )
        .fetch_one(&pool)
        .await
        .expect("follow-up activity count");
        assert_eq!(activity_count, 3);

        pool.close().await;
    }
}
