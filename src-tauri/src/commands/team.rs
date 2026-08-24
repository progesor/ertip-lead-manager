use tauri::State;

use crate::error::CommandError;
use crate::services::team_service::{AssignmentResult, StaffMember, StaffMemberInput, TeamService};
use crate::state::AppState;

#[tauri::command]
pub async fn list_staff_members(
    include_inactive: bool,
    state: State<'_, AppState>,
) -> Result<Vec<StaffMember>, CommandError> {
    TeamService::new(state.database.pool().clone())
        .list_staff(include_inactive)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn create_staff_member(
    input: StaffMemberInput,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    TeamService::new(state.database.pool().clone())
        .create_staff(input)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn update_staff_member(
    user_id: String,
    input: StaffMemberInput,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    TeamService::new(state.database.pool().clone())
        .update_staff(user_id, input)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn set_staff_member_active(
    user_id: String,
    is_active: bool,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    TeamService::new(state.database.pool().clone())
        .set_staff_active(user_id, is_active)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn assign_lead(
    contact_id: String,
    user_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AssignmentResult, CommandError> {
    TeamService::new(state.database.pool().clone())
        .assign_lead(contact_id, user_id)
        .await
        .map_err(CommandError::from)
}
