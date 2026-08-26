use chrono::{DateTime, Utc};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::{AUTHORIZATION, COOKIE}},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::error;

use crate::{
    app::AppState,
    auth::{AuthError, AuthService},
    authz::{Action, Actor, AuthorizationError, Role},
    crm::CrmError,
};

const SESSION_COOKIE_NAME: &str = "elm_session";
const ACTIVE_STATUSES: [&str; 5] = ["NEW", "CONTACTED", "REPLIED", "QUALIFIED", "QUOTE_SENT"];
const TERMINAL_STATUSES: [&str; 3] = ["WON", "LOST", "INVALID"];
const DEFAULT_COLUMN_LIMIT: u32 = 100;
const MAX_COLUMN_LIMIT: u32 = 250;

const EFFECTIVE_PRODUCTS_CTE: &str = r#"
WITH automatic_products AS (
    SELECT DISTINCT s.lead_contact_id, spi.product_code
    FROM lead_submissions s
    JOIN submission_product_interests spi ON spi.lead_submission_id = s.id
),
latest_overrides AS (
    SELECT DISTINCT ON (o.lead_contact_id, o.product_code)
           o.lead_contact_id, o.product_code, o.action
    FROM contact_product_interest_overrides o
    ORDER BY o.lead_contact_id, o.product_code, o.created_at DESC, o.id DESC
),
effective_products AS (
    SELECT a.lead_contact_id, a.product_code
    FROM automatic_products a
    LEFT JOIN latest_overrides o
      ON o.lead_contact_id = a.lead_contact_id
     AND o.product_code = a.product_code
    WHERE o.action IS NULL OR o.action = 'ADD'

    UNION

    SELECT o.lead_contact_id, o.product_code
    FROM latest_overrides o
    WHERE o.action = 'ADD'
)
"#;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipelineBoardRequest {
    pub search: Option<String>,
    pub country_code: Option<String>,
    pub product_code: Option<String>,
    pub assigned_user_id: Option<String>,
    pub unassigned_only: Option<bool>,
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
    pub assigned_user_id: Option<String>,
    pub assigned_user_name: Option<String>,
    pub assigned_user_active: Option<bool>,
    pub latest_submission_at: Option<DateTime<Utc>>,
    pub submission_count: i64,
    pub is_repeat: bool,
    pub product_interests: Vec<String>,
    pub platforms: Vec<String>,
    pub warning_count: i64,
    pub next_follow_up_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Clone)]
struct NormalizedRequest {
    search: Option<String>,
    country_code: Option<String>,
    product_code: Option<String>,
    assigned_user_id: Option<String>,
    unassigned_only: bool,
    repeat_only: bool,
    warning_only: bool,
    include_terminal: bool,
    follow_up_window: FollowUpWindow,
    column_limit: u32,
}

#[derive(Debug, Clone)]
enum FollowUpWindow {
    Any,
    Overdue(DateTime<Utc>),
    Today {
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    },
}

#[derive(Clone)]
pub struct PipelineService {
    pool: PgPool,
}

impl PipelineService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn board(
        &self,
        actor: &Actor,
        request: PipelineBoardRequest,
    ) -> Result<PipelineBoardResponse, CrmError> {
        actor.require(Action::LeadRead)?;
        validate_sales_filters(actor, &request)?;
        let request = normalize_request(request)?;

        let mut statuses = ACTIVE_STATUSES.to_vec();
        if request.include_terminal {
            statuses.extend(TERMINAL_STATUSES);
        }

        let mut columns = Vec::with_capacity(statuses.len());
        let mut visible_total = 0_i64;
        for status in statuses {
            let total = self.count_column(actor, &request, status).await?;
            let cards = self.load_cards(actor, &request, status).await?;
            visible_total += total;
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
            per_column_limit: request.column_limit,
        })
    }

    async fn count_column(
        &self,
        actor: &Actor,
        request: &NormalizedRequest,
        status: &str,
    ) -> Result<i64, CrmError> {
        let mut builder = QueryBuilder::<Postgres>::new(EFFECTIVE_PRODUCTS_CTE);
        builder.push(" SELECT COUNT(*)::bigint AS total FROM lead_contacts c WHERE c.status = ");
        builder.push_bind(status.to_string());
        push_filters(&mut builder, actor, request);
        let row = builder.build().fetch_one(&self.pool).await?;
        Ok(row.try_get("total")?)
    }

    async fn load_cards(
        &self,
        actor: &Actor,
        request: &NormalizedRequest,
        status: &str,
    ) -> Result<Vec<PipelineCard>, CrmError> {
        let mut builder = QueryBuilder::<Postgres>::new(EFFECTIVE_PRODUCTS_CTE);
        builder.push(
            r#"
            SELECT
                c.id,
                COALESCE(NULLIF(trim(c.display_name), ''), 'İsimsiz lead') AS display_name,
                c.primary_email,
                c.primary_phone,
                c.country_code,
                c.status,
                c.assigned_user_id,
                u.display_name AS assigned_user_name,
                u.is_active AS assigned_user_active,
                c.latest_submission_at,
                c.submission_count::bigint AS submission_count,
                ARRAY(
                    SELECT ep.product_code
                    FROM effective_products ep
                    WHERE ep.lead_contact_id = c.id
                    ORDER BY ep.product_code
                ) AS product_interests,
                ARRAY(
                    SELECT p.platform
                    FROM (
                        SELECT DISTINCT lower(trim(s.platform)) AS platform
                        FROM lead_submissions s
                        WHERE s.lead_contact_id = c.id
                          AND NULLIF(trim(s.platform), '') IS NOT NULL
                    ) p
                    ORDER BY p.platform
                ) AS platforms,
                (
                    SELECT COUNT(*)::bigint
                    FROM lead_data_quality_issues q
                    WHERE q.lead_contact_id = c.id AND q.status = 'OPEN'
                ) AS warning_count,
                (
                    SELECT MIN(f.due_at)
                    FROM follow_ups f
                    WHERE f.lead_contact_id = c.id AND f.status = 'OPEN'
                ) AS next_follow_up_at,
                (
                    SELECT COUNT(*)::bigint
                    FROM follow_ups f
                    WHERE f.lead_contact_id = c.id AND f.status = 'OPEN'
                ) AS open_follow_up_count
            FROM lead_contacts c
            LEFT JOIN app_users u ON u.id = c.assigned_user_id
            WHERE c.status =
            "#,
        );
        builder.push_bind(status.to_string());
        push_filters(&mut builder, actor, request);
        builder.push(" ORDER BY c.latest_submission_at DESC NULLS LAST, c.id ASC LIMIT ");
        builder.push_bind(request.column_limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter().map(map_card).collect()
    }
}

fn normalize_request(request: PipelineBoardRequest) -> Result<NormalizedRequest, CrmError> {
    let follow_up_window = match clean_optional(request.follow_up_mode).map(|v| v.to_ascii_uppercase()) {
        None => FollowUpWindow::Any,
        Some(mode) if mode == "OVERDUE" => {
            let now = parse_required_utc(request.now_utc.as_deref(), "nowUtc", "OVERDUE")?;
            FollowUpWindow::Overdue(now)
        }
        Some(mode) if mode == "TODAY" => {
            let from = parse_required_utc(request.now_utc.as_deref(), "nowUtc", "TODAY")?;
            let to = parse_required_utc(
                request.tomorrow_start_utc.as_deref(),
                "tomorrowStartUtc",
                "TODAY",
            )?;
            if from >= to {
                return Err(CrmError::Validation(
                    "nowUtc must be before tomorrowStartUtc".to_string(),
                ));
            }
            FollowUpWindow::Today { from, to }
        }
        Some(mode) => {
            return Err(CrmError::Validation(format!(
                "unsupported pipeline follow-up mode: {mode}"
            )));
        }
    };

    Ok(NormalizedRequest {
        search: clean_optional(request.search).map(|value| value.to_lowercase()),
        country_code: clean_optional(request.country_code).map(|value| value.to_ascii_uppercase()),
        product_code: clean_optional(request.product_code).map(|value| value.to_ascii_uppercase()),
        assigned_user_id: clean_optional(request.assigned_user_id),
        unassigned_only: request.unassigned_only.unwrap_or(false),
        repeat_only: request.repeat_only.unwrap_or(false),
        warning_only: request.warning_only.unwrap_or(false),
        include_terminal: request.include_terminal.unwrap_or(false),
        follow_up_window,
        column_limit: request
            .per_column_limit
            .unwrap_or(DEFAULT_COLUMN_LIMIT)
            .clamp(1, MAX_COLUMN_LIMIT),
    })
}

fn validate_sales_filters(actor: &Actor, request: &PipelineBoardRequest) -> Result<(), CrmError> {
    if actor.role != Role::Sales {
        return Ok(());
    }
    if request.unassigned_only.unwrap_or(false) {
        return Err(CrmError::Authorization(AuthorizationError::Forbidden));
    }
    if let Some(requested) = request
        .assigned_user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if requested != actor.user_id {
            return Err(CrmError::Authorization(AuthorizationError::Forbidden));
        }
    }
    Ok(())
}

fn push_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    actor: &Actor,
    request: &NormalizedRequest,
) {
    if actor.role == Role::Sales {
        builder.push(" AND c.assigned_user_id = ");
        builder.push_bind(actor.user_id.clone());
    } else if request.unassigned_only {
        builder.push(" AND c.assigned_user_id IS NULL");
    } else if let Some(user_id) = &request.assigned_user_id {
        builder.push(" AND c.assigned_user_id = ");
        builder.push_bind(user_id.clone());
    }

    if let Some(search) = &request.search {
        let pattern = format!("%{search}%");
        builder.push(" AND (lower(COALESCE(c.display_name, '')) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR lower(COALESCE(c.primary_email, '')) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR lower(COALESCE(c.primary_phone, '')) LIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    if let Some(country_code) = &request.country_code {
        builder.push(" AND upper(COALESCE(c.country_code, '')) = ");
        builder.push_bind(country_code.clone());
    }
    if let Some(product_code) = &request.product_code {
        builder.push(
            " AND EXISTS (SELECT 1 FROM effective_products ep WHERE ep.lead_contact_id = c.id AND ep.product_code = ",
        );
        builder.push_bind(product_code.clone());
        builder.push(")");
    }
    if request.repeat_only {
        builder.push(" AND c.submission_count > 1");
    }
    if request.warning_only {
        builder.push(
            " AND EXISTS (SELECT 1 FROM lead_data_quality_issues q WHERE q.lead_contact_id = c.id AND q.status = 'OPEN')",
        );
    }

    match &request.follow_up_window {
        FollowUpWindow::Any => {}
        FollowUpWindow::Overdue(before) => {
            builder.push(
                " AND EXISTS (SELECT 1 FROM follow_ups ff WHERE ff.lead_contact_id = c.id AND ff.status = 'OPEN' AND ff.due_at < ",
            );
            builder.push_bind(*before);
            builder.push(")");
        }
        FollowUpWindow::Today { from, to } => {
            builder.push(
                " AND EXISTS (SELECT 1 FROM follow_ups ff WHERE ff.lead_contact_id = c.id AND ff.status = 'OPEN' AND ff.due_at >= ",
            );
            builder.push_bind(*from);
            builder.push(" AND ff.due_at < ");
            builder.push_bind(*to);
            builder.push(")");
        }
    }
}

fn map_card(row: sqlx::postgres::PgRow) -> Result<PipelineCard, CrmError> {
    let submission_count: i64 = row.try_get("submission_count")?;
    Ok(PipelineCard {
        id: row.try_get("id")?,
        display_name: row.try_get("display_name")?,
        primary_email: row.try_get("primary_email")?,
        primary_phone: row.try_get("primary_phone")?,
        country_code: row.try_get("country_code")?,
        status: row.try_get("status")?,
        assigned_user_id: row.try_get("assigned_user_id")?,
        assigned_user_name: row.try_get("assigned_user_name")?,
        assigned_user_active: row.try_get("assigned_user_active")?,
        latest_submission_at: row.try_get("latest_submission_at")?,
        submission_count,
        is_repeat: submission_count > 1,
        product_interests: row.try_get("product_interests")?,
        platforms: row.try_get("platforms")?,
        warning_count: row.try_get("warning_count")?,
        next_follow_up_at: row.try_get("next_follow_up_at")?,
        open_follow_up_count: row.try_get("open_follow_up_count")?,
    })
}

fn parse_required_utc(
    value: Option<&str>,
    field: &str,
    mode: &str,
) -> Result<DateTime<Utc>, CrmError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CrmError::Validation(format!("{field} is required for {mode}")))?;
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| CrmError::Validation(format!("{field} must be RFC3339")))?;
    Ok(parsed.with_timezone(&Utc))
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: ApiErrorBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug)]
struct ApiHttpError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiHttpError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }
}

impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorResponse {
                error: ApiErrorBody { code: self.code, message: self.message },
            }),
        )
            .into_response()
    }
}

impl From<AuthError> for ApiHttpError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unauthorized => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Oturum gerekli veya oturum süresi dolmuş.",
            ),
            AuthError::InvalidCredentials => Self::new(
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "E-posta veya parola geçersiz.",
            ),
            AuthError::TemporarilyLocked => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "LOGIN_TEMPORARILY_LOCKED",
                "Çok sayıda başarısız deneme nedeniyle giriş geçici olarak kilitlendi.",
            ),
            other => {
                error!(error = %other, "pipeline authentication operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTH_INTERNAL_ERROR",
                    "Kimlik doğrulama işlemi tamamlanamadı.",
                )
            }
        }
    }
}

impl From<CrmError> for ApiHttpError {
    fn from(error: CrmError) -> Self {
        match error {
            CrmError::Authorization(AuthorizationError::Forbidden) => Self::new(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Bu işlem için yetkiniz yok.",
            ),
            CrmError::Authorization(AuthorizationError::InvalidRole(role)) => {
                error!(persisted_role = %role, "unsupported persisted authorization role");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTHORIZATION_INTERNAL_ERROR",
                    "Yetkilendirme işlemi tamamlanamadı.",
                )
            }
            CrmError::Validation(message) => Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message),
            CrmError::NotFound(message) => Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message),
            CrmError::Conflict { resource, current_revision } => Self::new(
                StatusCode::CONFLICT,
                "STALE_REVISION",
                format!("{resource} başka bir kullanıcı tarafından değiştirildi. Güncel revision: {current_revision}."),
            ),
            CrmError::Database(database_error) => {
                error!(error = %database_error, "pipeline CRM operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CRM_INTERNAL_ERROR",
                    "CRM işlemi tamamlanamadı.",
                )
            }
        }
    }
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");
    Router::new()
        .route("/api/v1/pipeline", get(get_pipeline))
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn get_pipeline(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(request): Query<PipelineBoardRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let board = PipelineService::new(state.pool.clone())
        .board(&actor, request)
        .await?;
    Ok(Json(board).into_response())
}

async fn authenticated_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Actor, ApiHttpError> {
    let token = session_token_from_headers(headers).ok_or(AuthError::Unauthorized)?;
    let session = AuthService::new(state.pool.clone(), state.session_ttl_hours)
        .resolve(&token)
        .await?;
    Actor::from_auth_user(&session.user)
        .map_err(|error| ApiHttpError::from(CrmError::Authorization(error)))
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(raw) = headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        if let Some(token) = raw.strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE_NAME && !value.is_empty()).then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::{PipelineBoardRequest, PipelineService};
    use crate::authz::{Actor, Role};

    #[tokio::test]
    async fn pipeline_is_scoped_and_preserves_active_terminal_and_followup_behavior() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping pipeline integration test");
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect test postgres");

        sqlx::query("DELETE FROM follow_ups WHERE lead_contact_id LIKE 'm6-pipe-%'")
            .execute(&pool).await.expect("clean followups");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'm6-pipe-%'")
            .execute(&pool).await.expect("clean contacts");
        sqlx::query("DELETE FROM app_users WHERE id IN ('m6-pipe-sales-a', 'm6-pipe-sales-b')")
            .execute(&pool).await.expect("clean users");

        sqlx::query(
            "INSERT INTO app_users (id, display_name, role, is_active, revision, created_at, updated_at) VALUES ('m6-pipe-sales-a', 'Pipeline Sales A', 'SALES', TRUE, 0, now(), now()), ('m6-pipe-sales-b', 'Pipeline Sales B', 'SALES', TRUE, 0, now(), now())",
        )
        .execute(&pool).await.expect("seed users");

        for (id, name, status, assignee) in [
            ("m6-pipe-new", "M6 Pipeline Isolated New", "NEW", Some("m6-pipe-sales-a")),
            ("m6-pipe-contacted", "M6 Pipeline Isolated Contacted", "CONTACTED", Some("m6-pipe-sales-b")),
            ("m6-pipe-won", "M6 Pipeline Isolated Won", "WON", Some("m6-pipe-sales-a")),
        ] {
            sqlx::query(
                "INSERT INTO lead_contacts (id, display_name, status, assigned_user_id, revision, created_at, updated_at, latest_submission_at, submission_count) VALUES ($1, $2, $3, $4, 0, now(), now(), now(), 1)",
            )
            .bind(id).bind(name).bind(status).bind(assignee)
            .execute(&pool).await.expect("seed lead");
        }

        sqlx::query(
            "INSERT INTO follow_ups (id, lead_contact_id, due_at, status, revision, created_at, updated_at) VALUES ('m6-pipe-follow', 'm6-pipe-new', '2026-08-25T06:00:00Z', 'OPEN', 0, now(), now())",
        )
        .execute(&pool).await.expect("seed followup");

        let manager = Actor { user_id: "manager".to_string(), role: Role::Manager };
        let sales_a = Actor { user_id: "m6-pipe-sales-a".to_string(), role: Role::Sales };
        let service = PipelineService::new(pool.clone());

        let active = service.board(&manager, PipelineBoardRequest {
            search: Some("m6 pipeline isolated".to_string()),
            ..PipelineBoardRequest::default()
        }).await.expect("manager active board");
        assert_eq!(active.columns.len(), 5);
        assert_eq!(active.visible_total, 2);
        assert_eq!(active.columns[0].cards[0].open_follow_up_count, 1);

        let full = service.board(&manager, PipelineBoardRequest {
            search: Some("m6 pipeline isolated".to_string()),
            include_terminal: Some(true),
            ..PipelineBoardRequest::default()
        }).await.expect("manager full board");
        assert_eq!(full.columns.len(), 8);
        assert_eq!(full.visible_total, 3);

        let sales = service.board(&sales_a, PipelineBoardRequest {
            search: Some("m6 pipeline isolated".to_string()),
            include_terminal: Some(true),
            ..PipelineBoardRequest::default()
        }).await.expect("sales scoped board");
        assert_eq!(sales.visible_total, 2);
        assert!(sales.columns.iter().flat_map(|column| &column.cards).all(|card| card.assigned_user_id.as_deref() == Some("m6-pipe-sales-a")));

        let overdue = service.board(&manager, PipelineBoardRequest {
            search: Some("m6 pipeline isolated".to_string()),
            follow_up_mode: Some("OVERDUE".to_string()),
            now_utc: Some("2026-08-25T07:00:00Z".to_string()),
            ..PipelineBoardRequest::default()
        }).await.expect("overdue board");
        assert_eq!(overdue.visible_total, 1);
        assert_eq!(overdue.columns[0].cards[0].id, "m6-pipe-new");

        let forbidden = service.board(&sales_a, PipelineBoardRequest {
            search: Some("m6 pipeline isolated".to_string()),
            assigned_user_id: Some("m6-pipe-sales-b".to_string()),
            ..PipelineBoardRequest::default()
        }).await;
        assert!(forbidden.is_err());

        sqlx::query("DELETE FROM follow_ups WHERE lead_contact_id LIKE 'm6-pipe-%'")
            .execute(&pool).await.expect("cleanup followups");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'm6-pipe-%'")
            .execute(&pool).await.expect("cleanup contacts");
        sqlx::query("DELETE FROM app_users WHERE id IN ('m6-pipe-sales-a', 'm6-pipe-sales-b')")
            .execute(&pool).await.expect("cleanup users");
    }
}
