//! Application services orchestrate domain rules and repositories.

pub mod analytics_service;
pub mod dashboard_service;
pub mod follow_up_service;
pub mod import_commit_service;
pub mod import_history_service;
pub mod import_preview_service;
pub mod lead_crm_service;
pub mod lead_detail_service;
pub mod lead_workspace_service;
pub mod pipeline_service;
pub mod team_service;

#[cfg(test)]
mod analytics_performance_tests;
#[cfg(test)]
mod lead_product_override_integration_tests;
#[cfg(test)]
mod lead_workspace_performance_tests;
