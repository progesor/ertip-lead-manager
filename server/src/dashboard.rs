use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::{AUTHORIZATION, COOKIE}},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
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
const DEFAULT_GROUP_LIMIT: u32 = 6;
const MAX_GROUP_LIMIT: u32 = 20;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionRequest {
    pub now_utc: String,
    pub today_start_utc: String,
    pub tomorrow_start_utc: String,
    pub recent_repeat_since_utc: String,
    pub analytics_since_utc: String,
    pub group_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardKpis {
    pub total_contacts: i64,
    pub new_contacts: i64,
    pub qualified_contacts: i64,
    pub quote_sent_contacts: i64,
    pub won_contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAnalyticsSummary {
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
    pub won_contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionLead {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub latest_submission_at: Option<String>,
    pub due_at: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionGroup {
    pub total: i64,
    pub items: Vec<DashboardAttentionLead>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardAttentionResponse {
    pub kpis: DashboardKpis,
    pub analytics_30d: DashboardAnalyticsSummary,
    pub new_uncontacted: DashboardAttentionGroup,
    pub due_today: DashboardAttentionGroup,
    pub overdue: DashboardAttentionGroup,
    pub recent_repeats: DashboardAttentionGroup,
    pub open_quality_issues: DashboardAttentionGroup,
}

#[derive(Debug, Clone)]
struct NormalizedRequest {
    now: DateTime<Utc>,
    today_start: DateTime<Utc>,
    tomorrow_start: DateTime<Utc>,
    recent_since: DateTime<Utc>,
    analytics_since: DateTime<Utc>,
    limit: i64,
}

#[derive(Clone)]
pub struct DashboardService {
    pool: PgPool,
}

impl DashboardService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn attention(
        &self,
        actor: &Actor,
        request: DashboardAttentionRequest,
    ) -> Result<DashboardAttentionResponse, CrmError> {
        actor.require(Action::LeadRead)?;
        let request = normalize_request(request)?;
        let sales_id = sales_scope(actor);

        Ok(DashboardAttentionResponse {
            kpis: self.kpis(sales_id).await?,
            analytics_30d: self
                .analytics_summary(request.analytics_since, request.now, sales_id)
                .await?,
            new_uncontacted: self.new_uncontacted(request.limit, sales_id).await?,
            due_today: self
                .follow_up_group(
                    Some(request.today_start),
                    Some(request.tomorrow_start),
                    None,
                    request.limit,
                    sales_id,
                )
                .await?,
            overdue: self
                .follow_up_group(None, None, Some(request.now), request.limit, sales_id)
                .await?,
            recent_repeats: self
                .recent_repeats(request.recent_since, request.limit, sales_id)
                .await?,
            open_quality_issues: self.open_quality_issues(request.limit, sales_id).await?,
        })
    }

    async fn kpis(&self, sales_id: Option<&str>) -> Result<DashboardKpis, CrmError> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint AS total_contacts,
                COUNT(*) FILTER (WHERE status = 'NEW')::bigint AS new_contacts,
                COUNT(*) FILTER (WHERE status = 'QUALIFIED')::bigint AS qualified_contacts,
                COUNT(*) FILTER (WHERE status = 'QUOTE_SENT')::bigint AS quote_sent_contacts,
                COUNT(*) FILTER (WHERE status = 'WON')::bigint AS won_contacts
            FROM lead_contacts
            WHERE ($1::text IS NULL OR assigned_user_id = $1)
            "#,
        )
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(DashboardKpis {
            total_contacts: row.try_get("total_contacts")?,
            new_contacts: row.try_get("new_contacts")?,
            qualified_contacts: row.try_get("qualified_contacts")?,
            quote_sent_contacts: row.try_get("quote_sent_contacts")?,
            won_contacts: row.try_get("won_contacts")?,
        })
    }

    async fn analytics_summary(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        sales_id: Option<&str>,
    ) -> Result<DashboardAnalyticsSummary, CrmError> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint AS submissions,
                COUNT(DISTINCT s.lead_contact_id)::bigint AS unique_contacts,
                COALESCE(SUM(CASE WHEN EXISTS (
                    SELECT 1 FROM lead_submissions earlier
                    WHERE earlier.lead_contact_id = s.lead_contact_id
                      AND (
                        COALESCE(earlier.source_created_at_utc, earlier.created_at) < COALESCE(s.source_created_at_utc, s.created_at)
                        OR (
                            COALESCE(earlier.source_created_at_utc, earlier.created_at) = COALESCE(s.source_created_at_utc, s.created_at)
                            AND earlier.id < s.id
                        )
                      )
                ) THEN 1 ELSE 0 END), 0)::bigint AS repeat_submissions
            FROM lead_submissions s
            JOIN lead_contacts c ON c.id = s.lead_contact_id
            WHERE COALESCE(s.source_created_at_utc, s.created_at) >= $1
              AND COALESCE(s.source_created_at_utc, s.created_at) < $2
              AND ($3::text IS NULL OR c.assigned_user_id = $3)
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;

        let won_contacts = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::bigint
            FROM lead_contacts c
            WHERE c.status = 'WON'
              AND ($3::text IS NULL OR c.assigned_user_id = $3)
              AND EXISTS (
                SELECT 1 FROM lead_submissions s
                WHERE s.lead_contact_id = c.id
                  AND COALESCE(s.source_created_at_utc, s.created_at) >= $1
                  AND COALESCE(s.source_created_at_utc, s.created_at) < $2
              )
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(DashboardAnalyticsSummary {
            submissions: row.try_get("submissions")?,
            unique_contacts: row.try_get("unique_contacts")?,
            repeat_submissions: row.try_get("repeat_submissions")?,
            won_contacts,
        })
    }

    async fn new_uncontacted(
        &self,
        limit: i64,
        sales_id: Option<&str>,
    ) -> Result<DashboardAttentionGroup, CrmError> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::bigint FROM lead_contacts WHERE status = 'NEW' AND ($1::text IS NULL OR assigned_user_id = $1)",
        )
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            r#"
            SELECT id, display_name, status, primary_phone, country_code, latest_submission_at,
                   NULL::timestamptz AS due_at, 1::bigint AS count
            FROM lead_contacts
            WHERE status = 'NEW' AND ($1::text IS NULL OR assigned_user_id = $1)
            ORDER BY latest_submission_at DESC NULLS LAST, id ASC
            LIMIT $2
            "#,
        )
        .bind(sales_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        group(total, rows)
    }

    async fn follow_up_group(
        &self,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        overdue_before: Option<DateTime<Utc>>,
        limit: i64,
        sales_id: Option<&str>,
    ) -> Result<DashboardAttentionGroup, CrmError> {
        let (total, rows) = if let Some(before) = overdue_before {
            let total = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)::bigint
                FROM follow_ups f
                JOIN lead_contacts c ON c.id = f.lead_contact_id
                WHERE f.status = 'OPEN' AND f.due_at < $1
                  AND ($2::text IS NULL OR c.assigned_user_id = $2)
                "#,
            )
            .bind(before)
            .bind(sales_id)
            .fetch_one(&self.pool)
            .await?;
            let rows = sqlx::query(
                r#"
                SELECT c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                       c.latest_submission_at, MIN(f.due_at) AS due_at, COUNT(*)::bigint AS count
                FROM follow_ups f
                JOIN lead_contacts c ON c.id = f.lead_contact_id
                WHERE f.status = 'OPEN' AND f.due_at < $1
                  AND ($2::text IS NULL OR c.assigned_user_id = $2)
                GROUP BY c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                         c.latest_submission_at
                ORDER BY MIN(f.due_at) ASC, c.id ASC
                LIMIT $3
                "#,
            )
            .bind(before)
            .bind(sales_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        } else {
            let start = start.expect("today start is supplied by caller");
            let end = end.expect("tomorrow start is supplied by caller");
            let total = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)::bigint
                FROM follow_ups f
                JOIN lead_contacts c ON c.id = f.lead_contact_id
                WHERE f.status = 'OPEN' AND f.due_at >= $1 AND f.due_at < $2
                  AND ($3::text IS NULL OR c.assigned_user_id = $3)
                "#,
            )
            .bind(start)
            .bind(end)
            .bind(sales_id)
            .fetch_one(&self.pool)
            .await?;
            let rows = sqlx::query(
                r#"
                SELECT c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                       c.latest_submission_at, MIN(f.due_at) AS due_at, COUNT(*)::bigint AS count
                FROM follow_ups f
                JOIN lead_contacts c ON c.id = f.lead_contact_id
                WHERE f.status = 'OPEN' AND f.due_at >= $1 AND f.due_at < $2
                  AND ($3::text IS NULL OR c.assigned_user_id = $3)
                GROUP BY c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                         c.latest_submission_at
                ORDER BY MIN(f.due_at) ASC, c.id ASC
                LIMIT $4
                "#,
            )
            .bind(start)
            .bind(end)
            .bind(sales_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        };
        group(total, rows)
    }

    async fn recent_repeats(
        &self,
        recent_since: DateTime<Utc>,
        limit: i64,
        sales_id: Option<&str>,
    ) -> Result<DashboardAttentionGroup, CrmError> {
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::bigint FROM lead_contacts
            WHERE submission_count > 1 AND latest_submission_at >= $1
              AND ($2::text IS NULL OR assigned_user_id = $2)
            "#,
        )
        .bind(recent_since)
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            r#"
            SELECT id, display_name, status, primary_phone, country_code, latest_submission_at,
                   NULL::timestamptz AS due_at, submission_count::bigint AS count
            FROM lead_contacts
            WHERE submission_count > 1 AND latest_submission_at >= $1
              AND ($2::text IS NULL OR assigned_user_id = $2)
            ORDER BY latest_submission_at DESC, id ASC
            LIMIT $3
            "#,
        )
        .bind(recent_since)
        .bind(sales_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        group(total, rows)
    }

    async fn open_quality_issues(
        &self,
        limit: i64,
        sales_id: Option<&str>,
    ) -> Result<DashboardAttentionGroup, CrmError> {
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::bigint
            FROM lead_data_quality_issues q
            LEFT JOIN lead_submissions s ON s.id = q.lead_submission_id
            JOIN lead_contacts c ON c.id = COALESCE(q.lead_contact_id, s.lead_contact_id)
            WHERE q.status = 'OPEN' AND ($1::text IS NULL OR c.assigned_user_id = $1)
            "#,
        )
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                   c.latest_submission_at, NULL::timestamptz AS due_at, COUNT(*)::bigint AS count
            FROM lead_data_quality_issues q
            LEFT JOIN lead_submissions s ON s.id = q.lead_submission_id
            JOIN lead_contacts c ON c.id = COALESCE(q.lead_contact_id, s.lead_contact_id)
            WHERE q.status = 'OPEN' AND ($1::text IS NULL OR c.assigned_user_id = $1)
            GROUP BY c.id, c.display_name, c.status, c.primary_phone, c.country_code,
                     c.latest_submission_at
            ORDER BY COUNT(*) DESC, c.latest_submission_at DESC NULLS LAST, c.id ASC
            LIMIT $2
            "#,
        )
        .bind(sales_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        group(total, rows)
    }
}

fn normalize_request(request: DashboardAttentionRequest) -> Result<NormalizedRequest, CrmError> {
    let now = parse_utc(&request.now_utc, "nowUtc")?;
    let today_start = parse_utc(&request.today_start_utc, "todayStartUtc")?;
    let tomorrow_start = parse_utc(&request.tomorrow_start_utc, "tomorrowStartUtc")?;
    let recent_since = parse_utc(&request.recent_repeat_since_utc, "recentRepeatSinceUtc")?;
    let analytics_since = parse_utc(&request.analytics_since_utc, "analyticsSinceUtc")?;
    if today_start >= tomorrow_start {
        return Err(CrmError::Validation("todayStartUtc must be before tomorrowStartUtc".to_string()));
    }
    if analytics_since >= now {
        return Err(CrmError::Validation("analyticsSinceUtc must be before nowUtc".to_string()));
    }
    Ok(NormalizedRequest {
        now,
        today_start,
        tomorrow_start,
        recent_since,
        analytics_since,
        limit: request.group_limit.unwrap_or(DEFAULT_GROUP_LIMIT).clamp(1, MAX_GROUP_LIMIT) as i64,
    })
}

fn parse_utc(value: &str, field: &str) -> Result<DateTime<Utc>, CrmError> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| CrmError::Validation(format!("{field} must be RFC3339")))
}

fn group(total: i64, rows: Vec<sqlx::postgres::PgRow>) -> Result<DashboardAttentionGroup, CrmError> {
    let items = rows
        .into_iter()
        .map(|row| {
            let display_name: Option<String> = row.try_get("display_name")?;
            let latest_submission_at: Option<DateTime<Utc>> = row.try_get("latest_submission_at")?;
            let due_at: Option<DateTime<Utc>> = row.try_get("due_at")?;
            Ok(DashboardAttentionLead {
                id: row.try_get("id")?,
                display_name: display_name
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "İsimsiz lead".to_string()),
                status: row.try_get("status")?,
                primary_phone: row.try_get("primary_phone")?,
                country_code: row.try_get("country_code")?,
                latest_submission_at: latest_submission_at.map(format_utc),
                due_at: due_at.map(format_utc),
                count: row.try_get("count")?,
            })
        })
        .collect::<Result<Vec<_>, CrmError>>()?;
    Ok(DashboardAttentionGroup { total, items })
}

fn format_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn sales_scope(actor: &Actor) -> Option<&str> {
    (actor.role == Role::Sales).then_some(actor.user_id.as_str())
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse { error: ApiErrorBody }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody { code: &'static str, message: String }
#[derive(Debug)]
struct ApiHttpError { status: StatusCode, code: &'static str, message: String }
impl ApiHttpError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }
}
impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(ApiErrorResponse { error: ApiErrorBody { code: self.code, message: self.message } })).into_response()
    }
}
impl From<AuthError> for ApiHttpError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unauthorized => Self::new(StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED", "Oturum gerekli veya oturum süresi dolmuş."),
            AuthError::InvalidCredentials => Self::new(StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS", "E-posta veya parola geçersiz."),
            AuthError::TemporarilyLocked => Self::new(StatusCode::TOO_MANY_REQUESTS, "LOGIN_TEMPORARILY_LOCKED", "Çok sayıda başarısız deneme nedeniyle giriş geçici olarak kilitlendi."),
            other => { error!(error = %other, "dashboard authentication operation failed"); Self::new(StatusCode::INTERNAL_SERVER_ERROR, "AUTH_INTERNAL_ERROR", "Kimlik doğrulama işlemi tamamlanamadı.") }
        }
    }
}
impl From<CrmError> for ApiHttpError {
    fn from(error: CrmError) -> Self {
        match error {
            CrmError::Authorization(AuthorizationError::Forbidden) => Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", "Bu işlem için yetkiniz yok."),
            CrmError::Authorization(AuthorizationError::InvalidRole(role)) => { error!(persisted_role = %role, "unsupported persisted authorization role"); Self::new(StatusCode::INTERNAL_SERVER_ERROR, "AUTHORIZATION_INTERNAL_ERROR", "Yetkilendirme işlemi tamamlanamadı.") }
            CrmError::Validation(message) => Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message),
            CrmError::NotFound(message) => Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message),
            CrmError::Conflict { resource, current_revision } => Self::new(StatusCode::CONFLICT, "STALE_REVISION", format!("{resource} başka bir kullanıcı tarafından değiştirildi. Güncel revision: {current_revision}.")),
            CrmError::Database(database_error) => { error!(error = %database_error, "dashboard CRM operation failed"); Self::new(StatusCode::INTERNAL_SERVER_ERROR, "CRM_INTERNAL_ERROR", "CRM işlemi tamamlanamadı.") }
        }
    }
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");
    Router::new()
        .route("/api/v1/dashboard/attention", get(get_attention))
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn get_attention(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(request): Query<DashboardAttentionRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let response = DashboardService::new(state.pool.clone()).attention(&actor, request).await?;
    Ok(Json(response).into_response())
}

async fn authenticated_actor(state: &AppState, headers: &HeaderMap) -> Result<Actor, ApiHttpError> {
    let token = session_token_from_headers(headers).ok_or(AuthError::Unauthorized)?;
    let session = AuthService::new(state.pool.clone(), state.session_ttl_hours).resolve(&token).await?;
    Actor::from_auth_user(&session.user).map_err(|error| ApiHttpError::from(CrmError::Authorization(error)))
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(raw) = headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        if let Some(token) = raw.strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() { return Some(token.to_string()); }
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

    use super::{DashboardAttentionRequest, DashboardService};
    use crate::{authz::{Actor, Role}, db::run_migrations};

    #[tokio::test]
    async fn dashboard_attention_preserves_groups_and_sales_scope() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping dashboard integration test");
            return;
        };
        let pool = PgPoolOptions::new().max_connections(4).connect(&database_url).await.expect("connect test postgres");
        run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM lead_data_quality_issues WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("clean quality");
        sqlx::query("DELETE FROM follow_ups WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("clean followups");
        sqlx::query("DELETE FROM lead_submissions WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("clean submissions");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("clean contacts");
        sqlx::query("DELETE FROM import_batches WHERE id = 'm6-dash-batch'").execute(&pool).await.expect("clean batch");
        sqlx::query("DELETE FROM app_users WHERE id IN ('m6-dash-sales-a', 'm6-dash-sales-b')").execute(&pool).await.expect("clean users");

        sqlx::query("INSERT INTO app_users (id, display_name, role, is_active, revision, created_at, updated_at) VALUES ('m6-dash-sales-a', 'Dashboard Sales A', 'SALES', TRUE, 0, now(), now()), ('m6-dash-sales-b', 'Dashboard Sales B', 'SALES', TRUE, 0, now(), now())")
            .execute(&pool).await.expect("seed users");
        sqlx::query("INSERT INTO lead_contacts (id, display_name, primary_phone, country_code, status, assigned_user_id, revision, created_at, updated_at, latest_submission_at, submission_count) VALUES ('m6-dash-new', 'Dashboard New', '+905551111111', 'TR', 'NEW', 'm6-dash-sales-a', 0, now(), now(), '2098-08-22T08:00:00Z', 1), ('m6-dash-repeat', 'Dashboard Repeat', '+905552222222', 'TR', 'CONTACTED', 'm6-dash-sales-a', 0, now(), now(), '2098-08-22T09:00:00Z', 2), ('m6-dash-won', 'Dashboard Won', NULL, 'GB', 'WON', 'm6-dash-sales-b', 0, now(), now(), '2098-08-22T10:00:00Z', 1)")
            .execute(&pool).await.expect("seed contacts");
        sqlx::query("INSERT INTO import_batches (id, file_name, file_format, sheet_name, started_at, completed_at, status, total_rows, app_version) VALUES ('m6-dash-batch', 'fixture.csv', 'CSV', 'CSV', now(), now(), 'COMMITTED', 3, '0.1.0')")
            .execute(&pool).await.expect("seed batch");
        for (id, contact, external, ts) in [
            ("m6-dash-s1", "m6-dash-new", "m6:dash:s1", "2098-08-22T08:00:00Z"),
            ("m6-dash-s2", "m6-dash-repeat", "m6:dash:s2", "2098-08-21T09:00:00Z"),
            ("m6-dash-s3", "m6-dash-repeat", "m6:dash:s3", "2098-08-22T09:00:00Z"),
            ("m6-dash-s4", "m6-dash-won", "m6:dash:s4", "2098-08-22T10:00:00Z"),
        ] {
            sqlx::query("INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_utc, source_created_at_raw, raw_payload_json, created_at) VALUES ($1, $2, 'm6-dash-batch', $3, $4::timestamptz, $4, '{}', $4::timestamptz)")
                .bind(id).bind(contact).bind(external).bind(ts).execute(&pool).await.expect("seed submission");
        }
        sqlx::query("INSERT INTO follow_ups (id, lead_contact_id, due_at, status, revision, created_at, updated_at) VALUES ('m6-dash-overdue', 'm6-dash-new', '2098-08-22T08:30:00Z', 'OPEN', 0, now(), now()), ('m6-dash-today', 'm6-dash-repeat', '2098-08-22T12:00:00Z', 'OPEN', 0, now(), now())")
            .execute(&pool).await.expect("seed followups");
        sqlx::query("INSERT INTO lead_data_quality_issues (id, lead_contact_id, issue_type, severity, status, created_at) VALUES ('m6-dash-quality', 'm6-dash-repeat', 'MISSING_FIELD', 'WARNING', 'OPEN', now())")
            .execute(&pool).await.expect("seed quality");

        let request = || DashboardAttentionRequest {
            now_utc: "2098-08-22T10:00:00Z".to_string(),
            today_start_utc: "2098-08-22T09:00:00Z".to_string(),
            tomorrow_start_utc: "2098-08-23T09:00:00Z".to_string(),
            recent_repeat_since_utc: "2098-08-22T00:00:00Z".to_string(),
            analytics_since_utc: "2098-08-20T00:00:00Z".to_string(),
            group_limit: Some(10),
        };
        let manager = Actor { user_id: "manager".to_string(), role: Role::Manager };
        let sales = Actor { user_id: "m6-dash-sales-a".to_string(), role: Role::Sales };
        let service = DashboardService::new(pool.clone());

        let manager_view = service.attention(&manager, request()).await.expect("manager dashboard");
        assert!(manager_view.kpis.total_contacts >= 3);
        assert_eq!(manager_view.analytics_30d.submissions, 4);
        assert_eq!(manager_view.analytics_30d.won_contacts, 1);
        assert_eq!(manager_view.due_today.total, 1);
        assert_eq!(manager_view.overdue.total, 1);

        let sales_view = service.attention(&sales, request()).await.expect("sales dashboard");
        assert_eq!(sales_view.kpis.total_contacts, 2);
        assert_eq!(sales_view.analytics_30d.submissions, 3);
        assert_eq!(sales_view.analytics_30d.won_contacts, 0);
        assert_eq!(sales_view.new_uncontacted.total, 1);
        assert_eq!(sales_view.due_today.total, 1);
        assert_eq!(sales_view.overdue.total, 1);
        assert_eq!(sales_view.recent_repeats.total, 1);
        assert_eq!(sales_view.open_quality_issues.total, 1);
        assert_eq!(sales_view.new_uncontacted.items[0].primary_phone.as_deref(), Some("+905551111111"));

        sqlx::query("DELETE FROM lead_data_quality_issues WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("cleanup quality");
        sqlx::query("DELETE FROM follow_ups WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("cleanup followups");
        sqlx::query("DELETE FROM lead_submissions WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("cleanup submissions");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'm6-dash-%'").execute(&pool).await.expect("cleanup contacts");
        sqlx::query("DELETE FROM import_batches WHERE id = 'm6-dash-batch'").execute(&pool).await.expect("cleanup batch");
        sqlx::query("DELETE FROM app_users WHERE id IN ('m6-dash-sales-a', 'm6-dash-sales-b')").execute(&pool).await.expect("cleanup users");
    }
}
