mod commands;
mod db;
mod domain;
mod error;
mod importer;
mod repositories;
mod services;
mod state;

use std::error::Error;

use db::Database;
use error::AppError;
use state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::new(AppError::AppData(error.to_string())) as Box<dyn Error>)?;

            let database_path = data_dir.join("ertip-lead-manager.sqlite3");
            let database = tauri::async_runtime::block_on(Database::connect(database_path))
                .map_err(|error| Box::new(error) as Box<dyn Error>)?;

            app.manage(AppState::new(database));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::get_app_diagnostics,
            commands::imports::preview_import,
            commands::imports::commit_import,
            commands::imports::list_import_history,
            commands::leads::list_leads,
        ])
        .run(tauri::generate_context!())
        .expect("Ertip Lead Manager çalıştırılamadı");
}
