use axum::{
    Json, Router,
    extract::State,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::error;

use crate::auth::{AuthError, AuthService, AuthUser, ClientKind};

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

fn auth_service(state: &AppState) -> AuthService {
    AuthService::new(state.pool.clone(), state.session_ttl_hours)
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
