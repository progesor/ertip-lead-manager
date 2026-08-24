use tauri::State;

use crate::error::CommandError;
use crate::services::dashboard_service::{
    DashboardAttentionRequest, DashboardAttentionResponse, DashboardService,
};
use crate::state::AppState;

#[tauri::command]
pub async fn get_dashboard_attention(
    request: DashboardAttentionRequest,
    state: State<'_, AppState>,
) -> Result<DashboardAttentionResponse, CommandError> {
    DashboardService::new(state.database.pool().clone())
        .attention(request)
        .await
        .map_err(CommandError::from)
}
