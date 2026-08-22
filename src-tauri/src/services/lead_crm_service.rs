use chrono::{SecondsFormat, Utc};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::lead_crm_repository::LeadCrmRepository;

const ALLOWED_STATUSES: [&str; 8] = [
    "NEW",
    "CONTACTED",
    "REPLIED",
    "QUALIFIED",
    "QUOTE_SENT",
    "WON",
    "LOST",
    "INVALID",
];

const MAX_NOTE_CHARS: usize = 5_000;

#[derive(Clone)]
pub struct LeadCrmService {
    repository: LeadCrmRepository,
}

impl LeadCrmService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: LeadCrmRepository::new(pool),
        }
    }

    pub async fn change_status(&self, contact_id: &str, new_status: &str) -> Result<bool, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let new_status = new_status.trim().to_ascii_uppercase();
        if !ALLOWED_STATUSES.contains(&new_status.as_str()) {
            return Err(AppError::Validation(format!(
                "unsupported lead status {new_status}"
            )));
        }

        self.repository
            .change_status(contact_id, &new_status, &now_utc())
            .await
    }

    pub async fn create_note(&self, contact_id: &str, body: &str) -> Result<String, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let body = clean_note(body)?;
        self.repository
            .create_note(contact_id, body, &now_utc())
            .await
    }

    pub async fn update_note(
        &self,
        contact_id: &str,
        note_id: &str,
        body: &str,
    ) -> Result<bool, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let note_id = clean_required(note_id, "note id")?;
        let body = clean_note(body)?;
        self.repository
            .update_note(contact_id, note_id, body, &now_utc())
            .await
    }

    pub async fn delete_note(&self, contact_id: &str, note_id: &str) -> Result<(), AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let note_id = clean_required(note_id, "note id")?;
        self.repository
            .delete_note(contact_id, note_id, &now_utc())
            .await
    }
}

fn clean_required<'a>(value: &'a str, label: &str) -> Result<&'a str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{label} is required")));
    }
    Ok(value)
}

fn clean_note(value: &str) -> Result<&str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation("note body is required".to_string()));
    }
    if value.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::Validation(format!(
            "note exceeds {MAX_NOTE_CHARS} characters"
        )));
    }
    Ok(value)
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::{clean_note, LeadCrmService};
    use crate::db::Database;
    use crate::error::AppError;

    #[test]
    fn note_validation_rejects_blank_body() {
        assert!(matches!(clean_note("   "), Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn unsupported_status_is_rejected_before_database_mutation() {
        let database = Database::connect_memory().await.expect("open database");
        let service = LeadCrmService::new(database.pool().clone());
        let result = service.change_status("any", "ARCHIVED").await;
        assert!(matches!(result, Err(AppError::Validation(_))));
    }
}
