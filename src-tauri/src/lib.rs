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
            commands::analytics::get_analytics_report,
            commands::dashboard::get_dashboard_attention,
            commands::diagnostics::get_app_diagnostics,
            commands::imports::preview_import,
            commands::imports::commit_import,
            commands::imports::list_import_history,
            commands::leads::list_leads,
            commands::leads::get_lead_filter_options,
            commands::leads::get_lead_detail,
            commands::leads::change_lead_status,
            commands::leads::create_lead_note,
            commands::leads::update_lead_note,
            commands::leads::delete_lead_note,
            commands::leads::set_lead_product_interest,
            commands::pipeline::get_pipeline_board,
            commands::follow_ups::list_lead_follow_ups,
            commands::follow_ups::create_lead_follow_up,
            commands::follow_ups::reschedule_lead_follow_up,
            commands::follow_ups::complete_lead_follow_up,
            commands::follow_ups::cancel_lead_follow_up,
            commands::team::list_staff_members,
            commands::team::create_staff_member,
            commands::team::update_staff_member,
            commands::team::set_staff_member_active,
            commands::team::assign_lead,
        ])
        .run(tauri::generate_context!())
        .expect("Ertip Lead Manager çalıştırılamadı");
}
