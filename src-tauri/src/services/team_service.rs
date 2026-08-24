use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::team_repository::{AssigneeRecord, StaffMemberRecord, TeamRepository};

const ROLES: [&str; 3] = ["ADMIN", "MANAGER", "SALES"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StaffMember {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub auth_subject: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadAssignee {
    pub id: String,
    pub display_name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentResult {
    pub changed: bool,
    pub assignee: Option<LeadAssignee>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaffMemberInput {
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Clone)]
pub struct TeamService {
    repository: TeamRepository,
}

impl TeamService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: TeamRepository::new(pool),
        }
    }

    pub async fn list_staff(&self, include_inactive: bool) -> Result<Vec<StaffMember>, AppError> {
        Ok(self
            .repository
            .list_staff(include_inactive)
            .await?
            .into_iter()
            .map(staff_member)
            .collect())
    }

    pub async fn create_staff(&self, input: StaffMemberInput) -> Result<String, AppError> {
        let (display_name, email, role) = validate_input(input)?;
        self.repository
            .create_staff(&display_name, email.as_deref(), &role, &now())
            .await
    }

    pub async fn update_staff(&self, user_id: String, input: StaffMemberInput) -> Result<(), AppError> {
        let user_id = required_id(user_id, "userId")?;
        let (display_name, email, role) = validate_input(input)?;
        self.repository
            .update_staff(&user_id, &display_name, email.as_deref(), &role, &now())
            .await
    }

    pub async fn set_staff_active(&self, user_id: String, is_active: bool) -> Result<(), AppError> {
        let user_id = required_id(user_id, "userId")?;
        self.repository
            .set_staff_active(&user_id, is_active, &now())
            .await
    }

    pub async fn assign_lead(
        &self,
        contact_id: String,
        user_id: Option<String>,
    ) -> Result<AssignmentResult, AppError> {
        let contact_id = required_id(contact_id, "contactId")?;
        let user_id = user_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        // Local mode intentionally has no authenticated actor yet. Online mode will derive this
        // from the server-side session, never from a client-supplied actor id.
        let result = self
            .repository
            .assign_lead(&contact_id, user_id.as_deref(), None, &now())
            .await?;

        Ok(AssignmentResult {
            changed: result.changed,
            assignee: result.assignee.map(lead_assignee),
        })
    }

    pub async fn assignee(&self, contact_id: &str) -> Result<Option<LeadAssignee>, AppError> {
        Ok(self
            .repository
            .contact_assignee(contact_id)
            .await?
            .map(lead_assignee))
    }
}

fn validate_input(input: StaffMemberInput) -> Result<(String, Option<String>, String), AppError> {
    let display_name = input.display_name.trim().to_string();
    if display_name.len() < 2 || display_name.len() > 100 {
        return Err(AppError::Validation(
            "staff display name must be between 2 and 100 characters".to_string(),
        ));
    }

    let email = input
        .email
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(email) = &email {
        let valid = email.len() <= 254
            && email.contains('@')
            && !email.starts_with('@')
            && !email.ends_with('@')
            && !email.chars().any(char::is_whitespace);
        if !valid {
            return Err(AppError::Validation("invalid staff email".to_string()));
        }
    }

    let role = input.role.trim().to_ascii_uppercase();
    if !ROLES.contains(&role.as_str()) {
        return Err(AppError::Validation("unsupported staff role".to_string()));
    }

    Ok((display_name, email, role))
}

fn required_id(value: String, field: &str) -> Result<String, AppError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    Ok(value)
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn staff_member(record: StaffMemberRecord) -> StaffMember {
    StaffMember {
        id: record.id,
        display_name: record.display_name,
        email: record.email,
        role: record.role,
        is_active: record.is_active,
        auth_subject: record.auth_subject,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn lead_assignee(record: AssigneeRecord) -> LeadAssignee {
    LeadAssignee {
        id: record.id,
        display_name: record.display_name,
        is_active: record.is_active,
    }
}

#[cfg(test)]
mod tests {
    use super::{StaffMemberInput, TeamService};
    use crate::db::Database;

    #[tokio::test]
    async fn inactive_staff_cannot_receive_new_assignment() {
        let database = Database::connect_memory().await.expect("open database");
        sqlx::query("INSERT INTO lead_contacts (id, display_name, status, created_at, updated_at, submission_count) VALUES ('team-contact', 'Lead', 'NEW', '2026-08-24T00:00:00.000Z', '2026-08-24T00:00:00.000Z', 0)")
            .execute(database.pool())
            .await
            .expect("seed contact");
        let service = TeamService::new(database.pool().clone());
        let user_id = service
            .create_staff(StaffMemberInput {
                display_name: "Test Sales".to_string(),
                email: None,
                role: "sales".to_string(),
            })
            .await
            .expect("create staff");
        service
            .set_staff_active(user_id.clone(), false)
            .await
            .expect("deactivate");

        let error = service
            .assign_lead("team-contact".to_string(), Some(user_id))
            .await
            .expect_err("inactive assignment must fail");
        assert!(matches!(error, crate::error::AppError::Validation(_)));
    }
}
