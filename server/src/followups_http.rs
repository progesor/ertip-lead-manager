use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::{AUTHORIZATION, COOKIE}},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::Serialize;
use tracing::error;

use crate::{
    app::AppState,
    auth::{AuthError, AuthService},
    authz::{Actor, AuthorizationError},
    crm::CrmError,
    followups::{
        CreateFollowUpRequest, FollowUpService, FollowUpTransitionRequest,
        RescheduleFollowUpRequest,
    },
};

const SESSION_COOKIE_NAME: &str = "elm_session";

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
                error!(error = %other, "follow-up authentication operation failed");
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
                error!(error = %database_error, "follow-up CRM operation failed");
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
    Router::new()
        .route(
            "/api/v1/leads/{contact_id}/follow-ups",
            get(list_followups).post(create_followup),
        )
        .route(
            "/api/v1/leads/{contact_id}/follow-ups/{follow_up_id}",
            patch(reschedule_followup),
        )
        .route(
            "/api/v1/leads/{contact_id}/follow-ups/{follow_up_id}/complete",
            post(complete_followup),
        )
        .route(
            "/api/v1/leads/{contact_id}/follow-ups/{follow_up_id}/cancel",
            post(cancel_followup),
        )
        .with_state(state)
}

async fn list_followups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let items = follow_up_service(&state)
        .list_for_contact(&actor, &contact_id)
        .await?;
    Ok(Json(items).into_response())
}

async fn create_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
    Json(request): Json<CreateFollowUpRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let item = follow_up_service(&state)
        .create(&actor, &contact_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(item)).into_response())
}

async fn reschedule_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((contact_id, follow_up_id)): Path<(String, String)>,
    Json(request): Json<RescheduleFollowUpRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = follow_up_service(&state)
        .reschedule(&actor, &contact_id, &follow_up_id, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn complete_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((contact_id, follow_up_id)): Path<(String, String)>,
    Json(request): Json<FollowUpTransitionRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = follow_up_service(&state)
        .complete(&actor, &contact_id, &follow_up_id, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn cancel_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((contact_id, follow_up_id)): Path<(String, String)>,
    Json(request): Json<FollowUpTransitionRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = follow_up_service(&state)
        .cancel(&actor, &contact_id, &follow_up_id, request)
        .await?;
    Ok(Json(result).into_response())
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

fn follow_up_service(state: &AppState) -> FollowUpService {
    FollowUpService::new(state.pool.clone())
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
        http::Request,
    };
    use tower::ServiceExt;

    use super::router;
    use crate::app::{AppState, build_pool};

    fn state() -> AppState {
        AppState {
            pool: build_pool("postgres://user:pass@127.0.0.1:1/app", 1)
                .expect("lazy postgres URL should be valid"),
            session_ttl_hours: 12,
        }
    }

    #[tokio::test]
    async fn follow_up_routes_require_auth_before_database_access() {
        let response = router(state())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/leads/example/follow-ups")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }
}
