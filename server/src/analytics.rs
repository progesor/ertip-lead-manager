use std::collections::BTreeMap;

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
const STATUS_ORDER: [&str; 8] = [
    "NEW", "CONTACTED", "REPLIED", "QUALIFIED", "QUOTE_SENT", "WON", "LOST", "INVALID",
];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRequest {
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsRange {
    pub earliest_submission_at: Option<String>,
    pub latest_submission_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTrendPoint {
    pub day: String,
    pub submissions: i64,
    pub unique_contacts: i64,
    pub repeat_submissions: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsStatusPoint {
    pub status: String,
    pub contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsBreakdownPoint {
    pub key: String,
    pub submissions: i64,
    pub unique_contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsNamedBreakdownPoint {
    pub key: String,
    pub name: String,
    pub submissions: i64,
    pub unique_contacts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsResponse {
    pub range: AnalyticsRange,
    pub summary: AnalyticsSummary,
    pub trend: Vec<AnalyticsTrendPoint>,
    pub current_status_funnel: Vec<AnalyticsStatusPoint>,
    pub country_breakdown: Vec<AnalyticsBreakdownPoint>,
    pub platform_breakdown: Vec<AnalyticsBreakdownPoint>,
    pub product_breakdown: Vec<AnalyticsBreakdownPoint>,
    pub campaign_breakdown: Vec<AnalyticsNamedBreakdownPoint>,
    pub form_breakdown: Vec<AnalyticsNamedBreakdownPoint>,
    pub adset_breakdown: Vec<AnalyticsNamedBreakdownPoint>,
    pub ad_breakdown: Vec<AnalyticsNamedBreakdownPoint>,
}

#[derive(Debug, Clone)]
struct Window {
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct AnalyticsService {
    pool: PgPool,
}

impl AnalyticsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn report(
        &self,
        actor: &Actor,
        request: AnalyticsRequest,
    ) -> Result<AnalyticsResponse, CrmError> {
        actor.require(Action::LeadRead)?;
        let window = normalize_window(request)?;
        let sales_id = sales_scope(actor);

        Ok(AnalyticsResponse {
            range: self.data_range(sales_id).await?,
            summary: self.summary(&window, sales_id).await?,
            trend: self.trend(&window, sales_id).await?,
            current_status_funnel: self.current_statuses(&window, sales_id).await?,
            country_breakdown: self.country_breakdown(&window, sales_id).await?,
            platform_breakdown: self.platform_breakdown(&window, sales_id).await?,
            product_breakdown: self.product_breakdown(&window, sales_id).await?,
            campaign_breakdown: self.named_breakdown(&window, sales_id, NamedDimension::Campaign).await?,
            form_breakdown: self.named_breakdown(&window, sales_id, NamedDimension::Form).await?,
            adset_breakdown: self.named_breakdown(&window, sales_id, NamedDimension::Adset).await?,
            ad_breakdown: self.named_breakdown(&window, sales_id, NamedDimension::Ad).await?,
        })
    }

    async fn data_range(&self, sales_id: Option<&str>) -> Result<AnalyticsRange, CrmError> {
        let row = sqlx::query(
            r#"
            SELECT
                MIN(COALESCE(s.source_created_at_utc, s.created_at)) AS earliest_submission_at,
                MAX(COALESCE(s.source_created_at_utc, s.created_at)) AS latest_submission_at
            FROM lead_submissions s
            JOIN lead_contacts c ON c.id = s.lead_contact_id
            WHERE ($1::text IS NULL OR c.assigned_user_id = $1)
            "#,
        )
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(AnalyticsRange {
            earliest_submission_at: format_optional_utc(row.try_get("earliest_submission_at")?),
            latest_submission_at: format_optional_utc(row.try_get("latest_submission_at")?),
        })
    }

    async fn summary(
        &self,
        window: &Window,
        sales_id: Option<&str>,
    ) -> Result<AnalyticsSummary, CrmError> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint AS submissions,
                COUNT(DISTINCT s.lead_contact_id)::bigint AS unique_contacts,
                COALESCE(SUM(CASE WHEN EXISTS (
                    SELECT 1
                    FROM lead_submissions earlier
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
            WHERE ($1::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) >= $1)
              AND ($2::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) < $2)
              AND ($3::text IS NULL OR c.assigned_user_id = $3)
            "#,
        )
        .bind(window.from)
        .bind(window.to)
        .bind(sales_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(AnalyticsSummary {
            submissions: row.try_get("submissions")?,
            unique_contacts: row.try_get("unique_contacts")?,
            repeat_submissions: row.try_get("repeat_submissions")?,
        })
    }

    async fn trend(
        &self,
        window: &Window,
        sales_id: Option<&str>,
    ) -> Result<Vec<AnalyticsTrendPoint>, CrmError> {
        let rows = sqlx::query(
            r#"
            SELECT
                (COALESCE(s.source_created_at_utc, s.created_at) AT TIME ZONE 'UTC')::date::text AS day,
                COUNT(*)::bigint AS submissions,
                COUNT(DISTINCT s.lead_contact_id)::bigint AS unique_contacts,
                COALESCE(SUM(CASE WHEN EXISTS (
                    SELECT 1
                    FROM lead_submissions earlier
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
            WHERE ($1::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) >= $1)
              AND ($2::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) < $2)
              AND ($3::text IS NULL OR c.assigned_user_id = $3)
            GROUP BY (COALESCE(s.source_created_at_utc, s.created_at) AT TIME ZONE 'UTC')::date
            ORDER BY day ASC
            "#,
        )
        .bind(window.from)
        .bind(window.to)
        .bind(sales_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsTrendPoint {
                    day: row.try_get("day")?,
                    submissions: row.try_get("submissions")?,
                    unique_contacts: row.try_get("unique_contacts")?,
                    repeat_submissions: row.try_get("repeat_submissions")?,
                })
            })
            .collect()
    }

    async fn current_statuses(
        &self,
        window: &Window,
        sales_id: Option<&str>,
    ) -> Result<Vec<AnalyticsStatusPoint>, CrmError> {
        let rows = sqlx::query(
            r#"
            SELECT c.status, COUNT(*)::bigint AS contacts
            FROM lead_contacts c
            WHERE ($3::text IS NULL OR c.assigned_user_id = $3)
              AND EXISTS (
                SELECT 1
                FROM lead_submissions s
                WHERE s.lead_contact_id = c.id
                  AND ($1::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) >= $1)
                  AND ($2::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) < $2)
              )
            GROUP BY c.status
            "#,
        )
        .bind(window.from)
        .bind(window.to)
        .bind(sales_id)
        .fetch_all(&self.pool)
        .await?;

        let mut counts = BTreeMap::new();
        for row in rows {
            counts.insert(row.try_get::<String, _>("status")?, row.try_get::<i64, _>("contacts")?);
        }
        Ok(STATUS_ORDER
            .iter()
            .map(|status| AnalyticsStatusPoint {
                status: (*status).to_string(),
                contacts: counts.remove(*status).unwrap_or(0),
            })
            .collect())
    }

    async fn country_breakdown(
        &self,
        window: &Window,
        sales_id: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownPoint>, CrmError> {
        self.breakdown(
            window,
            sales_id,
            "COALESCE(NULLIF(UPPER(TRIM(c.country_code)), ''), 'UNKNOWN')",
            "",
        )
        .await
    }

    async fn platform_breakdown(
        &self,
        window: &Window,
        sales_id: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownPoint>, CrmError> {
        self.breakdown(
            window,
            sales_id,
            "COALESCE(NULLIF(LOWER(TRIM(s.platform)), ''), 'unknown')",
            "",
        )
        .await
    }

    async fn product_breakdown(
        &self,
        window: &Window,
        sales_id: Option<&str>,
    ) -> Result<Vec<AnalyticsBreakdownPoint>, CrmError> {
        self.breakdown(
            window,
            sales_id,
            "COALESCE(spi.product_code, 'NO_PRODUCT')",
            "LEFT JOIN submission_product_interests spi ON spi.lead_submission_id = s.id",
        )
        .await
    }

    async fn breakdown(
        &self,
        window: &Window,
        sales_id: Option<&str>,
        key_expr: &str,
        extra_join: &str,
    ) -> Result<Vec<AnalyticsBreakdownPoint>, CrmError> {
        let sql = format!(
            r#"
            SELECT
                {key_expr} AS key,
                COUNT(*)::bigint AS submissions,
                COUNT(DISTINCT s.lead_contact_id)::bigint AS unique_contacts
            FROM lead_submissions s
            JOIN lead_contacts c ON c.id = s.lead_contact_id
            {extra_join}
            WHERE ($1::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) >= $1)
              AND ($2::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) < $2)
              AND ($3::text IS NULL OR c.assigned_user_id = $3)
            GROUP BY {key_expr}
            ORDER BY submissions DESC, key ASC
            "#
        );
        let rows = sqlx::query(&sql)
            .bind(window.from)
            .bind(window.to)
            .bind(sales_id)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsBreakdownPoint {
                    key: row.try_get("key")?,
                    submissions: row.try_get("submissions")?,
                    unique_contacts: row.try_get("unique_contacts")?,
                })
            })
            .collect()
    }

    async fn named_breakdown(
        &self,
        window: &Window,
        sales_id: Option<&str>,
        dimension: NamedDimension,
    ) -> Result<Vec<AnalyticsNamedBreakdownPoint>, CrmError> {
        let (id_column, name_column) = dimension.columns();
        let key_expr = format!(
            "COALESCE(NULLIF(TRIM(s.{id_column}), ''), NULLIF(TRIM(s.{name_column}), ''), 'UNKNOWN')"
        );
        let name_expr = format!(
            "COALESCE(NULLIF(TRIM(s.{name_column}), ''), NULLIF(TRIM(s.{id_column}), ''), 'Bilinmiyor')"
        );
        let sql = format!(
            r#"
            SELECT
                {key_expr} AS key,
                {name_expr} AS name,
                COUNT(*)::bigint AS submissions,
                COUNT(DISTINCT s.lead_contact_id)::bigint AS unique_contacts
            FROM lead_submissions s
            JOIN lead_contacts c ON c.id = s.lead_contact_id
            WHERE ($1::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) >= $1)
              AND ($2::timestamptz IS NULL OR COALESCE(s.source_created_at_utc, s.created_at) < $2)
              AND ($3::text IS NULL OR c.assigned_user_id = $3)
            GROUP BY {key_expr}, {name_expr}
            ORDER BY submissions DESC, lower({name_expr}) ASC, key ASC
            "#
        );
        let rows = sqlx::query(&sql)
            .bind(window.from)
            .bind(window.to)
            .bind(sales_id)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AnalyticsNamedBreakdownPoint {
                    key: row.try_get("key")?,
                    name: row.try_get("name")?,
                    submissions: row.try_get("submissions")?,
                    unique_contacts: row.try_get("unique_contacts")?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
enum NamedDimension {
    Campaign,
    Form,
    Adset,
    Ad,
}

impl NamedDimension {
    fn columns(self) -> (&'static str, &'static str) {
        match self {
            Self::Campaign => ("campaign_id", "campaign_name"),
            Self::Form => ("form_id", "form_name"),
            Self::Adset => ("adset_id", "adset_name"),
            Self::Ad => ("ad_id", "ad_name"),
        }
    }
}

fn normalize_window(request: AnalyticsRequest) -> Result<Window, CrmError> {
    let from = parse_optional_utc(request.from_utc, "fromUtc")?;
    let to = parse_optional_utc(request.to_utc, "toUtc")?;
    if let (Some(from), Some(to)) = (from, to) {
        if from >= to {
            return Err(CrmError::Validation("fromUtc must be before toUtc".to_string()));
        }
    }
    Ok(Window { from, to })
}

fn parse_optional_utc(
    value: Option<String>,
    field: &str,
) -> Result<Option<DateTime<Utc>>, CrmError> {
    let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(&value)
        .map_err(|_| CrmError::Validation(format!("{field} must be RFC3339")))?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

fn format_optional_utc(value: Option<DateTime<Utc>>) -> Option<String> {
    value.map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn sales_scope(actor: &Actor) -> Option<&str> {
    (actor.role == Role::Sales).then_some(actor.user_id.as_str())
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
            AuthError::InvalidCredentials => Self::new(StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS", "E-posta veya parola geçersiz."),
            AuthError::TemporarilyLocked => Self::new(StatusCode::TOO_MANY_REQUESTS, "LOGIN_TEMPORARILY_LOCKED", "Çok sayıda başarısız deneme nedeniyle giriş geçici olarak kilitlendi."),
            other => {
                error!(error = %other, "analytics authentication operation failed");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "AUTH_INTERNAL_ERROR", "Kimlik doğrulama işlemi tamamlanamadı.")
            }
        }
    }
}

impl From<CrmError> for ApiHttpError {
    fn from(error: CrmError) -> Self {
        match error {
            CrmError::Authorization(AuthorizationError::Forbidden) => Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", "Bu işlem için yetkiniz yok."),
            CrmError::Authorization(AuthorizationError::InvalidRole(role)) => {
                error!(persisted_role = %role, "unsupported persisted authorization role");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "AUTHORIZATION_INTERNAL_ERROR", "Yetkilendirme işlemi tamamlanamadı.")
            }
            CrmError::Validation(message) => Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message),
            CrmError::NotFound(message) => Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message),
            CrmError::Conflict { resource, current_revision } => Self::new(StatusCode::CONFLICT, "STALE_REVISION", format!("{resource} başka bir kullanıcı tarafından değiştirildi. Güncel revision: {current_revision}.")),
            CrmError::Database(database_error) => {
                error!(error = %database_error, "analytics CRM operation failed");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "CRM_INTERNAL_ERROR", "CRM işlemi tamamlanamadı.")
            }
        }
    }
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");
    Router::new()
        .route("/api/v1/analytics", get(get_analytics))
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn get_analytics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(request): Query<AnalyticsRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let response = AnalyticsService::new(state.pool.clone())
        .report(&actor, request)
        .await?;
    Ok(Json(response).into_response())
}

async fn authenticated_actor(state: &AppState, headers: &HeaderMap) -> Result<Actor, ApiHttpError> {
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

    use super::{AnalyticsRequest, AnalyticsService};
    use crate::{authz::{Actor, Role}, db::run_migrations};

    #[tokio::test]
    async fn analytics_preserves_submission_semantics_and_sales_scope() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping analytics integration test");
            return;
        };
        let pool = PgPoolOptions::new().max_connections(4).connect(&database_url).await.expect("connect test postgres");
        run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM submission_product_interests WHERE lead_submission_id LIKE 'm6-analytics-%'").execute(&pool).await.expect("cleanup products");
        sqlx::query("DELETE FROM lead_submissions WHERE id LIKE 'm6-analytics-%'").execute(&pool).await.expect("cleanup submissions");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'm6-analytics-%'").execute(&pool).await.expect("cleanup contacts");
        sqlx::query("DELETE FROM import_batches WHERE id = 'm6-analytics-batch'").execute(&pool).await.expect("cleanup batch");
        sqlx::query("DELETE FROM app_users WHERE id IN ('m6-analytics-sales-a', 'm6-analytics-sales-b')").execute(&pool).await.expect("cleanup users");

        sqlx::query("INSERT INTO app_users (id, display_name, role, is_active, revision, created_at, updated_at) VALUES ('m6-analytics-sales-a', 'Analytics Sales A', 'SALES', TRUE, 0, now(), now()), ('m6-analytics-sales-b', 'Analytics Sales B', 'SALES', TRUE, 0, now(), now())")
            .execute(&pool).await.expect("seed users");
        sqlx::query("INSERT INTO lead_contacts (id, display_name, country_code, status, assigned_user_id, revision, created_at, updated_at, latest_submission_at, submission_count) VALUES ('m6-analytics-a', 'Analytics A', 'TR', 'CONTACTED', 'm6-analytics-sales-a', 0, now(), now(), '2099-01-02T10:00:00Z', 2), ('m6-analytics-b', 'Analytics B', 'GB', 'WON', 'm6-analytics-sales-b', 0, now(), now(), '2099-01-02T11:00:00Z', 1)")
            .execute(&pool).await.expect("seed contacts");
        sqlx::query("INSERT INTO import_batches (id, file_name, file_format, sheet_name, started_at, completed_at, status, total_rows, app_version) VALUES ('m6-analytics-batch', 'fixture.csv', 'CSV', 'CSV', now(), now(), 'COMMITTED', 3, '0.1.0')")
            .execute(&pool).await.expect("seed batch");

        for (id, contact, external, ts, platform, campaign, campaign_name) in [
            ("m6-analytics-a1", "m6-analytics-a", "m6:analytics:a1", "2099-01-01T09:00:00Z", "facebook", "cmp-a", "Campaign A"),
            ("m6-analytics-a2", "m6-analytics-a", "m6:analytics:a2", "2099-01-02T10:00:00Z", "instagram", "cmp-a", "Campaign A"),
            ("m6-analytics-b1", "m6-analytics-b", "m6:analytics:b1", "2099-01-02T11:00:00Z", "facebook", "cmp-b", "Campaign B"),
        ] {
            sqlx::query("INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_utc, source_created_at_raw, platform, campaign_id, campaign_name, raw_payload_json, created_at) VALUES ($1, $2, 'm6-analytics-batch', $3, $4, $4, $5, $6, $7, '{}', $4)")
                .bind(id).bind(contact).bind(external).bind(ts).bind(platform).bind(campaign).bind(campaign_name)
                .execute(&pool).await.expect("seed submission");
        }
        sqlx::query("INSERT INTO submission_product_interests (id, lead_submission_id, product_code, origin, confidence, created_at) VALUES ('m6-analytics-p1', 'm6-analytics-a1', 'FUE_PUNCHES', 'DIRECT_MULTI_SELECT', 'HIGH', now()), ('m6-analytics-p2', 'm6-analytics-a2', 'FUE_PUNCHES', 'DIRECT_MULTI_SELECT', 'HIGH', now())")
            .execute(&pool).await.expect("seed products");

        let request = || AnalyticsRequest { from_utc: Some("2099-01-01T00:00:00Z".to_string()), to_utc: Some("2099-01-03T00:00:00Z".to_string()) };
        let manager = Actor { user_id: "manager".to_string(), role: Role::Manager };
        let sales = Actor { user_id: "m6-analytics-sales-a".to_string(), role: Role::Sales };
        let service = AnalyticsService::new(pool.clone());

        let manager_report = service.report(&manager, request()).await.expect("manager report");
        assert_eq!(manager_report.summary.submissions, 3);
        assert_eq!(manager_report.summary.unique_contacts, 2);
        assert_eq!(manager_report.summary.repeat_submissions, 1);
        assert_eq!(manager_report.trend.len(), 2);
        assert_eq!(manager_report.current_status_funnel.iter().find(|item| item.status == "WON").expect("won").contacts, 1);
        assert_eq!(manager_report.product_breakdown.iter().find(|item| item.key == "FUE_PUNCHES").expect("product").submissions, 2);
        assert_eq!(manager_report.product_breakdown.iter().find(|item| item.key == "NO_PRODUCT").expect("no product").submissions, 1);

        let sales_report = service.report(&sales, request()).await.expect("sales report");
        assert_eq!(sales_report.summary.submissions, 2);
        assert_eq!(sales_report.summary.unique_contacts, 1);
        assert_eq!(sales_report.summary.repeat_submissions, 1);
        assert_eq!(sales_report.current_status_funnel.iter().find(|item| item.status == "WON").expect("won").contacts, 0);
        assert!(sales_report.country_breakdown.iter().all(|item| item.key != "GB"));

        sqlx::query("DELETE FROM submission_product_interests WHERE lead_submission_id LIKE 'm6-analytics-%'").execute(&pool).await.expect("cleanup products");
        sqlx::query("DELETE FROM lead_submissions WHERE id LIKE 'm6-analytics-%'").execute(&pool).await.expect("cleanup submissions");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'm6-analytics-%'").execute(&pool).await.expect("cleanup contacts");
        sqlx::query("DELETE FROM import_batches WHERE id = 'm6-analytics-batch'").execute(&pool).await.expect("cleanup batch");
        sqlx::query("DELETE FROM app_users WHERE id IN ('m6-analytics-sales-a', 'm6-analytics-sales-b')").execute(&pool).await.expect("cleanup users");
    }
}
