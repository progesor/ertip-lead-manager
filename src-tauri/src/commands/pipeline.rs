use tauri::State;

use crate::error::CommandError;
use crate::services::pipeline_service::{
    PipelineBoardRequest, PipelineBoardResponse, PipelineService,
};
use crate::state::AppState;

#[tauri::command]
pub async fn get_pipeline_board(
    request: PipelineBoardRequest,
    state: State<'_, AppState>,
) -> Result<PipelineBoardResponse, CommandError> {
    PipelineService::new(state.database.pool().clone())
        .board(request)
        .await
        .map_err(CommandError::from)
}
