use std::path::PathBuf;

use tauri::State;

use crate::error::CommandError;
use crate::services::import_preview_service::{ImportPreview, ImportPreviewService};
use crate::state::AppState;

#[tauri::command]
pub async fn preview_import(
    path: String,
    state: State<'_, AppState>,
) -> Result<ImportPreview, CommandError> {
    let service = ImportPreviewService::new(state.database.pool().clone());
    service
        .preview(&PathBuf::from(path))
        .await
        .map_err(CommandError::from)
}
