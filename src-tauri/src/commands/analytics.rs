use tauri::State;

use crate::error::CommandError;
use crate::services::analytics_service::{AnalyticsRequest, AnalyticsResponse, AnalyticsService};
use crate::state::AppState;

#[tauri::command]
pub async fn get_analytics_report(
    request: AnalyticsRequest,
    state: State<'_, AppState>,
) -> Result<AnalyticsResponse, CommandError> {
    AnalyticsService::new(state.database.pool().clone())
        .report(request)
        .await
        .map_err(CommandError::from)
}
