use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::lead_workspace_repository::{
    LeadListFilters, LeadListQuery, LeadListSort, LeadWorkspaceRepository,
};

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LeadListRequest {
    pub search: Option<String>,
    pub status: Option<String>,
    pub country_code: Option<String>,
    pub product_code: Option<String>,
    pub repeat_only: Option<bool>,
    pub warning_only: Option<bool>,
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadListItem {
    pub id: String,
    pub display_name: String,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub latest_submission_at: Option<String>,
    pub submission_count: i64,
    pub is_repeat: bool,
    pub product_interests: Vec<String>,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadListResponse {
    pub items: Vec<LeadListItem>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[derive(Clone)]
pub struct LeadWorkspaceService {
    repository: LeadWorkspaceRepository,
}

impl LeadWorkspaceService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: LeadWorkspaceRepository::new(pool),
        }
    }

    pub async fn list(&self, request: LeadListRequest) -> Result<LeadListResponse, AppError> {
        let page = request.page.unwrap_or(0);
        let page_size = request
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);

        let filters = LeadListFilters {
            search: clean_optional(request.search),
            status: clean_optional(request.status),
            country_code: clean_optional(request.country_code).map(|value| value.to_ascii_uppercase()),
            product_code: clean_optional(request.product_code),
            repeat_only: request.repeat_only.unwrap_or(false),
            warning_only: request.warning_only.unwrap_or(false),
        };

        let query = LeadListQuery {
            filters,
            sort: parse_sort(request.sort.as_deref()),
            limit: page_size as i64,
            offset: page as i64 * page_size as i64,
        };

        let (records, total) = self.repository.list(&query).await?;
        let total_pages = if total <= 0 {
            0
        } else {
            ((total as u64 + page_size as u64 - 1) / page_size as u64) as u32
        };

        let items = records
            .into_iter()
            .map(|record| LeadListItem {
                id: record.id,
                display_name: record
                    .display_name
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "İsimsiz lead".to_string()),
                primary_email: record.primary_email,
                primary_phone: record.primary_phone,
                country_code: record.country_code,
                status: record.status,
                latest_submission_at: record.latest_submission_at,
                submission_count: record.submission_count,
                is_repeat: record.submission_count > 1,
                product_interests: record.product_codes,
                warning_count: record.warning_count,
            })
            .collect();

        Ok(LeadListResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_sort(value: Option<&str>) -> LeadListSort {
    match value.unwrap_or_default() {
        "LATEST_ASC" => LeadListSort::LatestAsc,
        "NAME_ASC" => LeadListSort::NameAsc,
        "NAME_DESC" => LeadListSort::NameDesc,
        _ => LeadListSort::LatestDesc,
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_optional, parse_sort};
    use crate::repositories::lead_workspace_repository::LeadListSort;

    #[test]
    fn request_helpers_are_deterministic() {
        assert_eq!(clean_optional(Some("  TR ".into())), Some("TR".into()));
        assert_eq!(clean_optional(Some("   ".into())), None);
        assert_eq!(parse_sort(Some("NAME_ASC")), LeadListSort::NameAsc);
        assert_eq!(parse_sort(Some("unknown")), LeadListSort::LatestDesc);
    }
}
