use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::follow_up_repository::{FollowUpRecord, FollowUpRepository};

const MAX_FOLLOW_UP_NOTE_CHARS: usize = 1_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpItem {
    pub id: String,
    pub lead_contact_id: String,
    pub due_at: String,
    pub status: String,
    pub note: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone)]
pub struct FollowUpService {
    repository: FollowUpRepository,
}

impl FollowUpService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: FollowUpRepository::new(pool),
        }
    }

    pub async fn list_for_contact(&self, contact_id: &str) -> Result<Vec<FollowUpItem>, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        Ok(self
            .repository
            .list_for_contact(contact_id)
            .await?
            .into_iter()
            .map(map_record)
            .collect())
    }

    pub async fn create(
        &self,
        contact_id: &str,
        due_at: &str,
        note: Option<&str>,
    ) -> Result<String, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let due_at = canonical_utc(due_at)?;
        let note = clean_note(note)?;
        self.repository
            .create(contact_id, &due_at, note.as_deref(), &now_utc())
            .await
    }

    pub async fn reschedule(
        &self,
        contact_id: &str,
        follow_up_id: &str,
        due_at: &str,
        note: Option<&str>,
    ) -> Result<bool, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let follow_up_id = clean_required(follow_up_id, "follow-up id")?;
        let due_at = canonical_utc(due_at)?;
        let note = clean_note(note)?;
        self.repository
            .reschedule(
                contact_id,
                follow_up_id,
                &due_at,
                note.as_deref(),
                &now_utc(),
            )
            .await
    }

    pub async fn complete(
        &self,
        contact_id: &str,
        follow_up_id: &str,
    ) -> Result<bool, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let follow_up_id = clean_required(follow_up_id, "follow-up id")?;
        self.repository
            .complete(contact_id, follow_up_id, &now_utc())
            .await
    }

    pub async fn cancel(
        &self,
        contact_id: &str,
        follow_up_id: &str,
    ) -> Result<bool, AppError> {
        let contact_id = clean_required(contact_id, "contact id")?;
        let follow_up_id = clean_required(follow_up_id, "follow-up id")?;
        self.repository
            .cancel(contact_id, follow_up_id, &now_utc())
            .await
    }
}

fn map_record(record: FollowUpRecord) -> FollowUpItem {
    FollowUpItem {
        id: record.id,
        lead_contact_id: record.lead_contact_id,
        due_at: record.due_at,
        status: record.status,
        note: record.note,
        created_at: record.created_at,
        completed_at: record.completed_at,
    }
}

fn clean_required<'a>(value: &'a str, label: &str) -> Result<&'a str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{label} is required")));
    }
    Ok(value)
}

fn clean_note(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_FOLLOW_UP_NOTE_CHARS {
        return Err(AppError::Validation(format!(
            "follow-up note exceeds {MAX_FOLLOW_UP_NOTE_CHARS} characters"
        )));
    }
    Ok(Some(value.to_string()))
}

fn canonical_utc(value: &str) -> Result<String, AppError> {
    let value = clean_required(value, "due_at")?;
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| AppError::Validation("follow-up due_at must be RFC3339".to_string()))?;
    Ok(parsed
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::{canonical_utc, clean_note};
    use crate::error::AppError;

    #[test]
    fn due_time_is_canonicalized_to_utc() {
        assert_eq!(
            canonical_utc("2026-08-22T15:30:00+03:00").expect("canonical due time"),
            "2026-08-22T12:30:00.000Z"
        );
    }

    #[test]
    fn blank_note_becomes_none_and_invalid_timestamp_is_rejected() {
        assert_eq!(clean_note(Some("   ")).expect("clean note"), None);
        assert!(matches!(canonical_utc("tomorrow"), Err(AppError::Validation(_))));
    }
}
