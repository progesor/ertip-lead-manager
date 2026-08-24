use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::lead_workspace_repository::{
    LeadListFilters, LeadListQuery, LeadListSort, LeadWorkspaceRepository,
};
use crate::repositories::pipeline_follow_up_repository::PipelineFollowUpRepository;

const ACTIVE_STATUSES: [&str; 5] = ["NEW", "CONTACTED", "REPLIED", "QUALIFIED", "QUOTE_SENT"];
const TERMINAL_STATUSES: [&str; 3] = ["WON", "LOST", "INVALID"];
const DEFAULT_COLUMN_LIMIT: u32 = 100;
const MAX_COLUMN_LIMIT: u32 = 250;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipelineBoardRequest {
    pub search: Option<String>,
    pub country_code: Option<String>,
    pub product_code: Option<String>,
    pub repeat_only: Option<bool>,
    pub warning_only: Option<bool>,
    pub include_terminal: Option<bool>,
    pub follow_up_mode: Option<String>,
    pub now_utc: Option<String>,
    pub tomorrow_start_utc: Option<String>,
    pub per_column_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineCard {
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
    pub platforms: Vec<String>,
    pub warning_count: i64,
    pub next_follow_up_at: Option<String>,
    pub open_follow_up_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineColumn {
    pub status: String,
    pub total: i64,
    pub cards: Vec<PipelineCard>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineBoardResponse {
    pub columns: Vec<PipelineColumn>,
    pub visible_total: i64,
    pub per_column_limit: u32,
}

#[derive(Clone)]
pub struct PipelineService {
    repository: LeadWorkspaceRepository,
    follow_up_repository: PipelineFollowUpRepository,
}

impl PipelineService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: LeadWorkspaceRepository::new(pool.clone()),
            follow_up_repository: PipelineFollowUpRepository::new(pool),
        }
    }

    pub async fn board(&self, request: PipelineBoardRequest) -> Result<PipelineBoardResponse, AppError> {
        let column_limit = request
            .per_column_limit
            .unwrap_or(DEFAULT_COLUMN_LIMIT)
            .clamp(1, MAX_COLUMN_LIMIT);

        let (follow_up_due_from, follow_up_due_to, follow_up_due_before) =
            follow_up_window(&request)?;

        let base_filters = LeadListFilters {
            search: clean_optional(request.search),
            status: None,
            country_code: clean_optional(request.country_code)
                .map(|value| value.to_ascii_uppercase()),
            product_code: clean_optional(request.product_code),
            repeat_only: request.repeat_only.unwrap_or(false),
            warning_only: request.warning_only.unwrap_or(false),
            follow_up_due_from,
            follow_up_due_to,
            follow_up_due_before,
        };

        let follow_up_summaries = self.follow_up_repository.open_summaries().await?;

        let mut statuses = ACTIVE_STATUSES.to_vec();
        if request.include_terminal.unwrap_or(false) {
            statuses.extend(TERMINAL_STATUSES);
        }

        let mut columns = Vec::with_capacity(statuses.len());
        let mut visible_total = 0_i64;

        for status in statuses {
            let mut filters = base_filters.clone();
            filters.status = Some(status.to_string());

            let query = LeadListQuery {
                filters,
                sort: LeadListSort::LatestDesc,
                limit: column_limit as i64,
                offset: 0,
            };

            let (records, total) = self.repository.list(&query).await?;
            visible_total += total;

            let cards = records
                .into_iter()
                .map(|record| {
                    let follow_up = follow_up_summaries.get(&record.id);
                    PipelineCard {
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
                        platforms: record.platforms,
                        warning_count: record.warning_count,
                        next_follow_up_at: follow_up.map(|item| item.next_due_at.clone()),
                        open_follow_up_count: follow_up.map(|item| item.open_count).unwrap_or(0),
                    }
                })
                .collect::<Vec<_>>();

            columns.push(PipelineColumn {
                status: status.to_string(),
                total,
                truncated: total > cards.len() as i64,
                cards,
            });
        }

        Ok(PipelineBoardResponse {
            columns,
            visible_total,
            per_column_limit: column_limit,
        })
    }
}

fn follow_up_window(
    request: &PipelineBoardRequest,
) -> Result<(Option<String>, Option<String>, Option<String>), AppError> {
    let mode = clean_optional(request.follow_up_mode.clone())
        .map(|value| value.to_ascii_uppercase());

    match mode.as_deref() {
        None => Ok((None, None, None)),
        Some("OVERDUE") => {
            let now = canonical_utc(
                request
                    .now_utc
                    .as_deref()
                    .ok_or_else(|| AppError::Validation("nowUtc is required for OVERDUE".to_string()))?,
                "nowUtc",
            )?;
            Ok((None, None, Some(now)))
        }
        Some("TODAY") => {
            let now = canonical_utc(
                request
                    .now_utc
                    .as_deref()
                    .ok_or_else(|| AppError::Validation("nowUtc is required for TODAY".to_string()))?,
                "nowUtc",
            )?;
            let tomorrow = canonical_utc(
                request
                    .tomorrow_start_utc
                    .as_deref()
                    .ok_or_else(|| {
                        AppError::Validation("tomorrowStartUtc is required for TODAY".to_string())
                    })?,
                "tomorrowStartUtc",
            )?;
            if now >= tomorrow {
                return Err(AppError::Validation(
                    "nowUtc must be before tomorrowStartUtc".to_string(),
                ));
            }
            Ok((Some(now), Some(tomorrow), None))
        }
        Some(value) => Err(AppError::Validation(format!(
            "unsupported pipeline follow-up mode: {value}"
        ))),
    }
}

fn canonical_utc(value: &str, field: &str) -> Result<String, AppError> {
    let parsed = DateTime::parse_from_rfc3339(value.trim())
        .map_err(|_| AppError::Validation(format!("{field} must be RFC3339")))?;
    Ok(parsed
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::{PipelineBoardRequest, PipelineService};
    use crate::db::Database;

    #[tokio::test]
    async fn pipeline_groups_contacts_and_filters_follow_up_attention_windows() {
        let database = Database::connect_memory().await.expect("open database");
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        for (id, name, status) in [
            ("pipeline-new", "New Lead", "NEW"),
            ("pipeline-contacted", "Contacted Lead", "CONTACTED"),
            ("pipeline-won", "Won Lead", "WON"),
        ] {
            sqlx::query(
                "INSERT INTO lead_contacts (id, display_name, status, created_at, updated_at, latest_submission_at, submission_count) VALUES (?, ?, ?, ?, ?, ?, 1)",
            )
            .bind(id)
            .bind(name)
            .bind(status)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("seed pipeline contact");
        }

        sqlx::query(
            "INSERT INTO follow_ups (id, lead_contact_id, due_at, status, note, created_at) VALUES ('pipeline-overdue', 'pipeline-new', '2026-08-24T06:00:00.000Z', 'OPEN', 'Ara', ?), ('pipeline-today', 'pipeline-contacted', '2026-08-24T12:00:00.000Z', 'OPEN', 'Teklif sor', ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("seed pipeline follow-ups");

        let service = PipelineService::new(database.pool().clone());
        let active = service
            .board(PipelineBoardRequest::default())
            .await
            .expect("load active board");

        assert_eq!(active.columns.len(), 5);
        assert_eq!(active.visible_total, 2);
        assert_eq!(active.columns[0].status, "NEW");
        assert_eq!(active.columns[0].cards[0].id, "pipeline-new");
        assert_eq!(active.columns[0].cards[0].open_follow_up_count, 1);
        assert!(!active.columns.iter().any(|column| column.status == "WON"));

        let overdue = service
            .board(PipelineBoardRequest {
                follow_up_mode: Some("OVERDUE".to_string()),
                now_utc: Some("2026-08-24T07:00:00.000Z".to_string()),
                ..PipelineBoardRequest::default()
            })
            .await
            .expect("load overdue board");
        assert_eq!(overdue.visible_total, 1);
        assert_eq!(overdue.columns[0].cards[0].id, "pipeline-new");

        let due_today = service
            .board(PipelineBoardRequest {
                follow_up_mode: Some("TODAY".to_string()),
                now_utc: Some("2026-08-24T07:00:00.000Z".to_string()),
                tomorrow_start_utc: Some("2026-08-24T21:00:00.000Z".to_string()),
                ..PipelineBoardRequest::default()
            })
            .await
            .expect("load today board");
        assert_eq!(due_today.visible_total, 1);
        assert_eq!(
            due_today
                .columns
                .iter()
                .find(|column| column.status == "CONTACTED")
                .expect("contacted column")
                .cards[0]
                .id,
            "pipeline-contacted"
        );

        let with_terminal = service
            .board(PipelineBoardRequest {
                include_terminal: Some(true),
                ..PipelineBoardRequest::default()
            })
            .await
            .expect("load full board");

        assert_eq!(with_terminal.columns.len(), 8);
        assert_eq!(with_terminal.visible_total, 3);
        assert_eq!(
            with_terminal
                .columns
                .iter()
                .find(|column| column.status == "WON")
                .expect("won column")
                .cards[0]
                .id,
            "pipeline-won"
        );
    }
}
