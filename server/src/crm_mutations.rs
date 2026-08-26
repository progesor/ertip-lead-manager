use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, Row};
use uuid::Uuid;

use crate::{
    authz::{Action, Actor, LeadScope},
    crm::CrmError,
};

const MAX_NOTE_CHARS: usize = 5_000;
const PRODUCT_CODES: [&str; 7] = [
    "FUE_MICROMOTOR_SYSTEMS",
    "LONG_HAIR_FUE_SOLUTIONS",
    "FUE_PUNCHES",
    "IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS",
    "MEDICAL_CHAIRS_CLINIC_FURNITURE",
    "OTHER_GENERAL_INFORMATION",
    "UNKNOWN",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadNote {
    pub id: String,
    pub body: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLeadNoteRequest {
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLeadNoteRequest {
    pub body: String,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLeadNoteQuery {
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NoteMutationResult {
    pub changed: bool,
    pub note: LeadNote,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNoteResult {
    pub deleted: bool,
    pub note_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductInterestRequest {
    pub included: bool,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductInterestResult {
    pub changed: bool,
    pub product_code: String,
    pub included: bool,
    pub revision: i64,
}

#[derive(Clone)]
pub struct CrmMutationService {
    pool: PgPool,
}

impl CrmMutationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_note(
        &self,
        actor: &Actor,
        contact_id: &str,
        request: CreateLeadNoteRequest,
    ) -> Result<LeadNote, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        let body = validate_note_body(&request.body)?;
        let mut tx = self.pool.begin().await?;
        ensure_lead_scope(&mut tx, actor, contact_id, false).await?;

        let note_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO lead_notes (
                id, lead_contact_id, body, revision, created_at, updated_at
            ) VALUES ($1, $2, $3, 0, $4, $4)
            "#,
        )
        .bind(&note_id)
        .bind(contact_id)
        .bind(&body)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "NOTE_CREATED",
            json!({ "noteId": note_id.clone() }).to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(LeadNote {
            id: note_id,
            body,
            revision: 0,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn update_note(
        &self,
        actor: &Actor,
        contact_id: &str,
        note_id: &str,
        request: UpdateLeadNoteRequest,
    ) -> Result<NoteMutationResult, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        let note_id = required_id(note_id, "noteId")?;
        validate_expected_revision(request.expected_revision)?;
        let body = validate_note_body(&request.body)?;
        let mut tx = self.pool.begin().await?;
        ensure_lead_scope(&mut tx, actor, contact_id, false).await?;

        let row = sqlx::query(
            r#"
            SELECT body, revision, created_at, updated_at
            FROM lead_notes
            WHERE id = $1 AND lead_contact_id = $2
            FOR UPDATE
            "#,
        )
        .bind(note_id)
        .bind(contact_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CrmError::NotFound("lead note".to_string()))?;

        let current_revision: i64 = row.try_get("revision")?;
        if current_revision != request.expected_revision {
            return Err(CrmError::Conflict {
                resource: "lead note".to_string(),
                current_revision,
            });
        }
        let current_body: String = row.try_get("body")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let current_updated_at: DateTime<Utc> = row.try_get("updated_at")?;
        if current_body == body {
            tx.commit().await?;
            return Ok(NoteMutationResult {
                changed: false,
                note: LeadNote {
                    id: note_id.to_string(),
                    body,
                    revision: current_revision,
                    created_at,
                    updated_at: current_updated_at,
                },
            });
        }

        let next_revision = current_revision + 1;
        let updated_at = Utc::now();
        sqlx::query(
            r#"
            UPDATE lead_notes
            SET body = $1, revision = $2, updated_at = $3
            WHERE id = $4 AND lead_contact_id = $5
            "#,
        )
        .bind(&body)
        .bind(next_revision)
        .bind(updated_at)
        .bind(note_id)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;
        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "NOTE_UPDATED",
            json!({ "noteId": note_id }).to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(NoteMutationResult {
            changed: true,
            note: LeadNote {
                id: note_id.to_string(),
                body,
                revision: next_revision,
                created_at,
                updated_at,
            },
        })
    }

    pub async fn delete_note(
        &self,
        actor: &Actor,
        contact_id: &str,
        note_id: &str,
        expected_revision: i64,
    ) -> Result<DeleteNoteResult, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        let note_id = required_id(note_id, "noteId")?;
        validate_expected_revision(expected_revision)?;
        let mut tx = self.pool.begin().await?;
        ensure_lead_scope(&mut tx, actor, contact_id, false).await?;

        let current_revision = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT revision
            FROM lead_notes
            WHERE id = $1 AND lead_contact_id = $2
            FOR UPDATE
            "#,
        )
        .bind(note_id)
        .bind(contact_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CrmError::NotFound("lead note".to_string()))?;
        if current_revision != expected_revision {
            return Err(CrmError::Conflict {
                resource: "lead note".to_string(),
                current_revision,
            });
        }

        sqlx::query("DELETE FROM lead_notes WHERE id = $1 AND lead_contact_id = $2")
            .bind(note_id)
            .bind(contact_id)
            .execute(&mut *tx)
            .await?;
        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "NOTE_DELETED",
            json!({ "noteId": note_id }).to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(DeleteNoteResult {
            deleted: true,
            note_id: note_id.to_string(),
        })
    }

    pub async fn set_product_interest(
        &self,
        actor: &Actor,
        contact_id: &str,
        product_code: &str,
        request: ProductInterestRequest,
    ) -> Result<ProductInterestResult, CrmError> {
        actor.require(Action::LeadContentUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        validate_expected_revision(request.expected_revision)?;
        let product_code = product_code.trim().to_ascii_uppercase();
        if !PRODUCT_CODES.contains(&product_code.as_str()) {
            return Err(CrmError::Validation(format!(
                "unsupported product code {product_code}"
            )));
        }

        let mut tx = self.pool.begin().await?;
        let current_revision = ensure_lead_scope(&mut tx, actor, contact_id, true)
            .await?
            .expect("locked lead scope returns revision");
        if current_revision != request.expected_revision {
            return Err(CrmError::Conflict {
                resource: "lead contact".to_string(),
                current_revision,
            });
        }

        let automatic_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM lead_submissions s
            JOIN submission_product_interests spi ON spi.lead_submission_id = s.id
            WHERE s.lead_contact_id = $1 AND spi.product_code = $2
            "#,
        )
        .bind(contact_id)
        .bind(&product_code)
        .fetch_one(&mut *tx)
        .await?;
        let latest_action = sqlx::query_scalar::<_, String>(
            r#"
            SELECT action
            FROM contact_product_interest_overrides
            WHERE lead_contact_id = $1 AND product_code = $2
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(contact_id)
        .bind(&product_code)
        .fetch_optional(&mut *tx)
        .await?;
        let currently_included = match latest_action.as_deref() {
            Some("ADD") => true,
            Some("REMOVE") => false,
            _ => automatic_count > 0,
        };

        if currently_included == request.included {
            tx.commit().await?;
            return Ok(ProductInterestResult {
                changed: false,
                product_code,
                included: request.included,
                revision: current_revision,
            });
        }

        let action = if request.included { "ADD" } else { "REMOVE" };
        let override_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO contact_product_interest_overrides (
                id, lead_contact_id, product_code, action, created_at
            ) VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&override_id)
        .bind(contact_id)
        .bind(&product_code)
        .bind(action)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let next_revision = current_revision + 1;
        sqlx::query(
            "UPDATE lead_contacts SET revision = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(next_revision)
        .bind(now)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;
        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "PRODUCT_INTEREST_CHANGED",
            json!({
                "productCode": product_code.clone(),
                "included": request.included,
                "previousIncluded": currently_included,
                "overrideId": override_id,
            })
            .to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(ProductInterestResult {
            changed: true,
            product_code,
            included: request.included,
            revision: next_revision,
        })
    }
}

async fn ensure_lead_scope(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    actor: &Actor,
    contact_id: &str,
    lock_for_update: bool,
) -> Result<Option<i64>, CrmError> {
    let sql = if lock_for_update {
        "SELECT assigned_user_id, revision FROM lead_contacts WHERE id = $1 FOR UPDATE"
    } else {
        "SELECT assigned_user_id, revision FROM lead_contacts WHERE id = $1 FOR SHARE"
    };
    let row = sqlx::query(sql)
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
    if lock_for_update {
        Ok(Some(row.try_get("revision")?))
    } else {
        Ok(None)
    }
}

fn validate_note_body(value: &str) -> Result<String, CrmError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CrmError::Validation("note body is required".to_string()));
    }
    if value.chars().count() > MAX_NOTE_CHARS {
        return Err(CrmError::Validation(format!(
            "note exceeds {MAX_NOTE_CHARS} characters"
        )));
    }
    Ok(value.to_string())
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
    use sqlx::postgres::PgPoolOptions;

    use super::{
        CreateLeadNoteRequest, CrmMutationService, ProductInterestRequest,
        UpdateLeadNoteRequest,
    };
    use crate::{
        authz::{Actor, Role},
        crm::CrmError,
        db::run_migrations,
    };

    #[tokio::test]
    async fn note_and_product_mutations_are_scoped_audited_and_revision_safe() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping CRM mutation integration test");
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect PostgreSQL test database");
        run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM lead_activities WHERE lead_contact_id = 'crm-mut-test-lead'")
            .execute(&pool)
            .await
            .expect("cleanup activities");
        sqlx::query("DELETE FROM lead_contacts WHERE id = 'crm-mut-test-lead'")
            .execute(&pool)
            .await
            .expect("cleanup lead");
        sqlx::query("DELETE FROM app_users WHERE id LIKE 'crm-mut-test-%'")
            .execute(&pool)
            .await
            .expect("cleanup users");

        for (id, name, role) in [
            ("crm-mut-test-manager", "Mutation Manager", "MANAGER"),
            ("crm-mut-test-sales-a", "Mutation Sales A", "SALES"),
            ("crm-mut-test-sales-b", "Mutation Sales B", "SALES"),
        ] {
            sqlx::query(
                "INSERT INTO app_users (id, display_name, role, is_active, created_at, updated_at) VALUES ($1, $2, $3, TRUE, now(), now())",
            )
            .bind(id)
            .bind(name)
            .bind(role)
            .execute(&pool)
            .await
            .expect("seed user");
        }
        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, status, assigned_user_id, created_at, updated_at, submission_count) VALUES ('crm-mut-test-lead', 'Mutation Lead', 'NEW', 'crm-mut-test-sales-a', now(), now(), 0)",
        )
        .execute(&pool)
        .await
        .expect("seed lead");

        let service = CrmMutationService::new(pool.clone());
        let sales_a = Actor {
            user_id: "crm-mut-test-sales-a".to_string(),
            role: Role::Sales,
        };
        let sales_b = Actor {
            user_id: "crm-mut-test-sales-b".to_string(),
            role: Role::Sales,
        };
        let manager = Actor {
            user_id: "crm-mut-test-manager".to_string(),
            role: Role::Manager,
        };

        let note = service
            .create_note(
                &sales_a,
                "crm-mut-test-lead",
                CreateLeadNoteRequest {
                    body: "İlk not".to_string(),
                },
            )
            .await
            .expect("create note");
        assert_eq!(note.revision, 0);
        let updated = service
            .update_note(
                &sales_a,
                "crm-mut-test-lead",
                &note.id,
                UpdateLeadNoteRequest {
                    body: "Güncel not".to_string(),
                    expected_revision: 0,
                },
            )
            .await
            .expect("update note");
        assert!(updated.changed);
        assert_eq!(updated.note.revision, 1);
        assert!(matches!(
            service
                .update_note(
                    &sales_a,
                    "crm-mut-test-lead",
                    &note.id,
                    UpdateLeadNoteRequest {
                        body: "Stale not".to_string(),
                        expected_revision: 0,
                    },
                )
                .await,
            Err(CrmError::Conflict {
                current_revision: 1,
                ..
            })
        ));
        service
            .delete_note(&sales_a, "crm-mut-test-lead", &note.id, 1)
            .await
            .expect("delete note");

        let product = service
            .set_product_interest(
                &sales_a,
                "crm-mut-test-lead",
                "FUE_PUNCHES",
                ProductInterestRequest {
                    included: true,
                    expected_revision: 0,
                },
            )
            .await
            .expect("add product");
        assert!(product.changed);
        assert_eq!(product.revision, 1);
        assert!(matches!(
            service
                .set_product_interest(
                    &sales_a,
                    "crm-mut-test-lead",
                    "FUE_PUNCHES",
                    ProductInterestRequest {
                        included: false,
                        expected_revision: 0,
                    },
                )
                .await,
            Err(CrmError::Conflict {
                current_revision: 1,
                ..
            })
        ));
        let removed = service
            .set_product_interest(
                &manager,
                "crm-mut-test-lead",
                "FUE_PUNCHES",
                ProductInterestRequest {
                    included: false,
                    expected_revision: 1,
                },
            )
            .await
            .expect("manager removes product");
        assert_eq!(removed.revision, 2);
        assert!(matches!(
            service
                .set_product_interest(
                    &sales_b,
                    "crm-mut-test-lead",
                    "LONG_HAIR_FUE_SOLUTIONS",
                    ProductInterestRequest {
                        included: true,
                        expected_revision: 2,
                    },
                )
                .await,
            Err(CrmError::NotFound(_))
        ));

        let sales_note_activities = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM lead_activities WHERE lead_contact_id = 'crm-mut-test-lead' AND actor_user_id = 'crm-mut-test-sales-a' AND activity_type IN ('NOTE_CREATED', 'NOTE_UPDATED', 'NOTE_DELETED')",
        )
        .fetch_one(&pool)
        .await
        .expect("note activity count");
        assert_eq!(sales_note_activities, 3);
        let manager_product_activities = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM lead_activities WHERE lead_contact_id = 'crm-mut-test-lead' AND actor_user_id = 'crm-mut-test-manager' AND activity_type = 'PRODUCT_INTEREST_CHANGED'",
        )
        .fetch_one(&pool)
        .await
        .expect("product activity count");
        assert_eq!(manager_product_activities, 1);

        pool.close().await;
    }
}
