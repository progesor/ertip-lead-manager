use tauri::State;

use crate::error::CommandError;
use crate::services::lead_workspace_service::{
    LeadListRequest, LeadListResponse, LeadWorkspaceService,
};
use crate::state::AppState;

#[tauri::command]
pub async fn list_leads(
    request: LeadListRequest,
    state: State<'_, AppState>,
) -> Result<LeadListResponse, CommandError> {
    LeadWorkspaceService::new(state.database.pool().clone())
        .list(request)
        .await
        .map_err(CommandError::from)
}
