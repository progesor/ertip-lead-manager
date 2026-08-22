use tauri::State;

use crate::error::CommandError;
use crate::services::lead_detail_service::{LeadDetailResponse, LeadDetailService};
use crate::services::lead_workspace_service::{
    LeadFilterOptions, LeadListRequest, LeadListResponse, LeadWorkspaceService,
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

#[tauri::command]
pub async fn get_lead_filter_options(
    state: State<'_, AppState>,
) -> Result<LeadFilterOptions, CommandError> {
    LeadWorkspaceService::new(state.database.pool().clone())
        .filter_options()
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn get_lead_detail(
    contact_id: String,
    state: State<'_, AppState>,
) -> Result<Option<LeadDetailResponse>, CommandError> {
    LeadDetailService::new(state.database.pool().clone())
        .get(&contact_id)
        .await
        .map_err(CommandError::from)
}