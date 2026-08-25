use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::error;

use crate::{
    auth::{AuthError, AuthService, AuthUser, ClientKind},
    authz::{Actor, AuthorizationError},
    crm::{
        AssignmentRequest, ChangeLeadStatusRequest, CreateStaffRequest, CrmError, CrmService,
        LeadListRequest, SetStaffActiveRequest, UpdateStaffRequest,
    },
    crm_mutations::{
        CreateLeadNoteRequest, CrmMutationService, DeleteLeadNoteQuery, ProductInterestRequest,
        UpdateLeadNoteRequest,
    },
};

const SESSION_COOKIE_NAME: &str = "elm_session";

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub session_ttl_hours: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
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
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorResponse {
                error: ApiErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

impl From<AuthError> for ApiHttpError {
    fn from(auth_error: AuthError) -> Self {
        match auth_error {
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
            AuthError::Unauthorized => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Oturum gerekli veya oturum süresi dolmuş.",
            ),
            other => {
                error!(error = %other, "authentication operation failed");
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
    fn from(crm_error: CrmError) -> Self {
        match crm_error {
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
            CrmError::Validation(message) => {
                Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message)
            }
            CrmError::NotFound(message) => {
                Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
            }
            CrmError::Conflict {
                resource,
                current_revision,
            } => Self::new(
                StatusCode::CONFLICT,
                "STALE_REVISION",
                format!(
                    "{resource} başka bir kullanıcı tarafından değiştirildi. Güncel revision: {current_revision}."
                ),
            ),
            CrmError::Database(database_error) => {
                error!(error = %database_error, "CRM operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CRM_INTERNAL_ERROR",
                    "CRM işlemi tamamlanamadı.",
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    user: AuthUser,
    expires_at: String,
    token: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersonnelListQuery {
    include_inactive: Option<bool>,
}

pub fn build_pool(database_url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_lazy(database_url)
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");

    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/api/v1", get(api_root))
        .route("/api/v1/auth/login/tauri", post(login_tauri))
        .route("/api/v1/auth/login/web", post(login_web))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/me", get(me))
        .route(
            "/api/v1/personnel",
            get(list_personnel).post(create_personnel),
        )
        .route("/api/v1/personnel/{user_id}", patch(update_personnel))
        .route(
            "/api/v1/personnel/{user_id}/active",
            patch(set_personnel_active),
        )
        .route("/api/v1/leads", get(list_leads))
        .route("/api/v1/leads/{contact_id}", get(get_lead))
        .route(
            "/api/v1/leads/{contact_id}/assignment",
            put(assign_lead),
        )
        .route(
            "/api/v1/leads/{contact_id}/status",
            patch(change_lead_status),
        )
        .route(
            "/api/v1/leads/{contact_id}/notes",
            post(create_lead_note),
        )
        .route(
            "/api/v1/leads/{contact_id}/notes/{note_id}",
            patch(update_lead_note).delete(delete_lead_note),
        )
        .route(
            "/api/v1/leads/{contact_id}/product-interests/{product_code}",
            put(set_product_interest),
        )
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn live() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "ertip-lead-manager-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn api_root() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "ertip-lead-manager-api-v1",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn ready(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiHttpError> {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(1) => Ok(Json(HealthResponse {
            status: "ready",
            service: "ertip-lead-manager-server",
            version: env!("CARGO_PKG_VERSION"),
        })),
        Ok(_) | Err(_) => Err(ApiHttpError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "DATABASE_UNAVAILABLE",
            "Database dependency is not ready.",
        )),
    }
}

async fn login_tauri(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiHttpError> {
    let login = auth_service(&state)
        .login(&request.email, &request.password, ClientKind::Tauri)
        .await?;

    Ok(Json(LoginResponse {
        user: login.user,
        expires_at: login.expires_at.to_rfc3339(),
        token: Some(login.token),
    })
    .into_response())
}

async fn login_web(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiHttpError> {
    let login = auth_service(&state)
        .login(&request.email, &request.password, ClientKind::Web)
        .await?;

    let cookie = session_cookie(&login.token, state.session_ttl_hours)?;
    let mut response = Json(LoginResponse {
        user: login.user,
        expires_at: login.expires_at.to_rfc3339(),
        token: None,
    })
    .into_response();
    response.headers_mut().insert(SET_COOKIE, cookie);
    Ok(response)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthUser>, ApiHttpError> {
    let token = session_token_from_headers(&headers).ok_or(AuthError::Unauthorized)?;
    let session = auth_service(&state).resolve(&token).await?;
    Ok(Json(session.user))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiHttpError> {
    let token = session_token_from_headers(&headers).ok_or(AuthError::Unauthorized)?;
    let service = auth_service(&state);
    let session = service.resolve(&token).await?;
    service.logout(&session.session_id).await?;

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_static(
            "elm_session=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0",
        ),
    );
    Ok(response)
}

async fn list_personnel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PersonnelListQuery>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let staff = crm_service(&state)
        .list_staff(&actor, query.include_inactive.unwrap_or(false))
        .await?;
    Ok(Json(staff).into_response())
}

async fn create_personnel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateStaffRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let staff = crm_service(&state).create_staff(&actor, request).await?;
    Ok((StatusCode::CREATED, Json(staff)).into_response())
}

async fn update_personnel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateStaffRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let staff = crm_service(&state)
        .update_staff(&actor, &user_id, request)
        .await?;
    Ok(Json(staff).into_response())
}

async fn set_personnel_active(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<SetStaffActiveRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let staff = crm_service(&state)
        .set_staff_active(&actor, &user_id, request)
        .await?;
    Ok(Json(staff).into_response())
}

async fn list_leads(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(request): Query<LeadListRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let leads = crm_service(&state).list_leads(&actor, request).await?;
    Ok(Json(leads).into_response())
}

async fn get_lead(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    match crm_service(&state).get_lead(&actor, &contact_id).await? {
        Some(lead) => Ok(Json(lead).into_response()),
        None => Err(ApiHttpError::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "lead contact",
        )),
    }
}

async fn assign_lead(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
    Json(request): Json<AssignmentRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = crm_service(&state)
        .assign_lead(&actor, &contact_id, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn change_lead_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
    Json(request): Json<ChangeLeadStatusRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = crm_service(&state)
        .change_lead_status(&actor, &contact_id, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn create_lead_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
    Json(request): Json<CreateLeadNoteRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let note = crm_mutation_service(&state)
        .create_note(&actor, &contact_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(note)).into_response())
}

async fn update_lead_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((contact_id, note_id)): Path<(String, String)>,
    Json(request): Json<UpdateLeadNoteRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = crm_mutation_service(&state)
        .update_note(&actor, &contact_id, &note_id, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn delete_lead_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((contact_id, note_id)): Path<(String, String)>,
    Query(query): Query<DeleteLeadNoteQuery>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = crm_mutation_service(&state)
        .delete_note(&actor, &contact_id, &note_id, query.expected_revision)
        .await?;
    Ok(Json(result).into_response())
}

async fn set_product_interest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((contact_id, product_code)): Path<(String, String)>,
    Json(request): Json<ProductInterestRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = crm_mutation_service(&state)
        .set_product_interest(&actor, &contact_id, &product_code, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn authenticated_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Actor, ApiHttpError> {
    let token = session_token_from_headers(headers).ok_or(AuthError::Unauthorized)?;
    let session = auth_service(state).resolve(&token).await?;
    Actor::from_auth_user(&session.user)
        .map_err(|error| ApiHttpError::from(CrmError::Authorization(error)))
}

fn auth_service(state: &AppState) -> AuthService {
    AuthService::new(state.pool.clone(), state.session_ttl_hours)
}

fn crm_service(state: &AppState) -> CrmService {
    CrmService::new(state.pool.clone())
}

fn crm_mutation_service(state: &AppState) -> CrmMutationService {
    CrmMutationService::new(state.pool.clone())
}

fn session_cookie(token: &str, ttl_hours: i64) -> Result<HeaderValue, ApiHttpError> {
    let max_age = ttl_hours.saturating_mul(60).saturating_mul(60);
    let value = format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age}"
    );
    HeaderValue::from_str(&value).map_err(|_| {
        ApiHttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SESSION_COOKIE_ERROR",
            "Oturum yanıtı oluşturulamadı.",
        )
    })
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(raw_authorization) = headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        if let Some(token) = raw_authorization.strip_prefix("Bearer ") {
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
    use axum::{
        body::Body,
        http::{
            HeaderMap, HeaderValue, Request,
            header::{AUTHORIZATION, COOKIE},
        },
    };
    use tower::ServiceExt;

    use super::{AppState, build_pool, router, session_token_from_headers};

    fn state() -> AppState {
        let pool = build_pool("postgres://user:pass@127.0.0.1:1/app", 1)
            .expect("lazy postgres URL should be valid");
        AppState {
            pool,
            session_ttl_hours: 12,
        }
    }

    #[tokio::test]
    async fn liveness_does_not_require_database_round_trip() {
        let response = router(state())
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn api_v1_root_is_explicit() {
        let response = router(state())
            .oneshot(
                Request::builder()
                    .uri("/api/v1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn crm_routes_require_auth_before_database_access() {
        let response = router(state())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/leads")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn bearer_token_takes_precedence_and_cookie_is_supported() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer tauri-token"));
        headers.insert(
            COOKIE,
            HeaderValue::from_static("other=x; elm_session=web-token"),
        );
        assert_eq!(
            session_token_from_headers(&headers).as_deref(),
            Some("tauri-token")
        );

        headers.remove(AUTHORIZATION);
        assert_eq!(
            session_token_from_headers(&headers).as_deref(),
            Some("web-token")
        );
    }
}
