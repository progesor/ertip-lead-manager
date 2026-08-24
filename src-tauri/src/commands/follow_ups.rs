use tauri::State;

use crate::error::CommandError;
use crate::services::follow_up_service::{FollowUpItem, FollowUpService};
use crate::state::AppState;

#[tauri::command]
pub async fn list_lead_follow_ups(
    contact_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<FollowUpItem>, CommandError> {
    FollowUpService::new(state.database.pool().clone())
        .list_for_contact(&contact_id)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn create_lead_follow_up(
    contact_id: String,
    due_at: String,
    note: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    FollowUpService::new(state.database.pool().clone())
        .create(&contact_id, &due_at, note.as_deref())
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn reschedule_lead_follow_up(
    contact_id: String,
    follow_up_id: String,
    due_at: String,
    note: Option<String>,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    FollowUpService::new(state.database.pool().clone())
        .reschedule(&contact_id, &follow_up_id, &due_at, note.as_deref())
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn complete_lead_follow_up(
    contact_id: String,
    follow_up_id: String,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    FollowUpService::new(state.database.pool().clone())
        .complete(&contact_id, &follow_up_id)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn cancel_lead_follow_up(
    contact_id: String,
    follow_up_id: String,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    FollowUpService::new(state.database.pool().clone())
        .cancel(&contact_id, &follow_up_id)
        .await
        .map_err(CommandError::from)
}
