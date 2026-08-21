use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::error::CommandError;
use crate::services::import_commit_service::{CommitImportResult, ImportCommitService};
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

#[tauri::command]
pub async fn commit_import(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CommitImportResult, CommandError> {
    let service = ImportCommitService::new(state.database.pool().clone());
    service
        .commit(&PathBuf::from(path), &app.package_info().version.to_string())
        .await
        .map_err(CommandError::from)
}
