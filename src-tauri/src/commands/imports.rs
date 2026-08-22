use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::error::CommandError;
use crate::repositories::import_history_repository::ImportHistoryRepository;
use crate::services::import_commit_service::{CommitImportResult, ImportCommitService};
use crate::services::import_history_service::{ImportHistoryItem, ImportHistoryService};
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

#[tauri::command]
pub async fn list_import_history(
    state: State<'_, AppState>,
) -> Result<Vec<ImportHistoryItem>, CommandError> {
    let repository = ImportHistoryRepository::new(state.database.pool().clone());
    let service = ImportHistoryService::new(repository);
    service.list_recent(50).await.map_err(CommandError::from)
}
