use tauri::State;

use crate::error::CommandError;
use crate::services::lead_crm_service::LeadCrmService;
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

#[tauri::command]
pub async fn change_lead_status(
    contact_id: String,
    new_status: String,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    LeadCrmService::new(state.database.pool().clone())
        .change_status(&contact_id, &new_status)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn create_lead_note(
    contact_id: String,
    body: String,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    LeadCrmService::new(state.database.pool().clone())
        .create_note(&contact_id, &body)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn update_lead_note(
    contact_id: String,
    note_id: String,
    body: String,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    LeadCrmService::new(state.database.pool().clone())
        .update_note(&contact_id, &note_id, &body)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn delete_lead_note(
    contact_id: String,
    note_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    LeadCrmService::new(state.database.pool().clone())
        .delete_note(&contact_id, &note_id)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn set_lead_product_interest(
    contact_id: String,
    product_code: String,
    included: bool,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    LeadCrmService::new(state.database.pool().clone())
        .set_product_interest(&contact_id, &product_code, included)
        .await
        .map_err(CommandError::from)
}
