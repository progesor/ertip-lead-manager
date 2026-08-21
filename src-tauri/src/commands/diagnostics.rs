use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::CommandError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDiagnostics {
    app_version: String,
    database_path: String,
    schema_version: i64,
}

#[tauri::command]
pub async fn get_app_diagnostics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppDiagnostics, CommandError> {
    let schema_version = state
        .database
        .schema_version()
        .await
        .map_err(CommandError::from)?;

    Ok(AppDiagnostics {
        app_version: app.package_info().version.to_string(),
        database_path: state.database.path().display().to_string(),
        schema_version,
    })
}
