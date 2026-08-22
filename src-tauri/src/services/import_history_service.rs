use serde::Serialize;

use crate::error::AppError;
use crate::repositories::import_history_repository::{
    ImportHistoryRecord, ImportHistoryRepository,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportHistoryItem {
    pub batch_id: String,
    pub file_name: String,
    pub format: String,
    pub sheet_name: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub total_rows: i64,
    pub imported_submissions: i64,
    pub exact_duplicates: i64,
    pub repeat_submissions: i64,
    pub warning_count: i64,
    pub error_count: i64,
    pub app_version: String,
}

#[derive(Clone)]
pub struct ImportHistoryService {
    repository: ImportHistoryRepository,
}

impl ImportHistoryService {
    pub fn new(repository: ImportHistoryRepository) -> Self {
        Self { repository }
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<ImportHistoryItem>, AppError> {
        let limit = limit.clamp(1, 100);
        let records = self.repository.list_recent(limit).await?;
        Ok(records.into_iter().map(map_record).collect())
    }
}

fn map_record(record: ImportHistoryRecord) -> ImportHistoryItem {
    ImportHistoryItem {
        batch_id: record.id,
        file_name: record.file_name,
        format: record.file_format,
        sheet_name: record.sheet_name,
        completed_at: record.completed_at,
        status: record.status,
        total_rows: record.total_rows,
        imported_submissions: record.new_submissions,
        exact_duplicates: record.exact_duplicates,
        repeat_submissions: record.repeat_candidates,
        warning_count: record.warning_count,
        error_count: record.error_count,
        app_version: record.app_version,
    }
}
