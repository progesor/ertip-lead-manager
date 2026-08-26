use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::{AUTHORIZATION, COOKIE}},
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use thiserror::Error;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::error;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::{AuthError, AuthService, hash_password, verify_password},
    authz::{Action, Actor, AuthorizationError},
};

const SESSION_COOKIE_NAME: &str = "elm_session";
const TOKEN_TTL_HOURS: i64 = 24;
const MIN_PASSWORD_LENGTH: usize = 12;
const MAX_PASSWORD_LENGTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenPurpose {
    Provision,
    Reset,
}

impl TokenPurpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::Provision => "PROVISION",
            Self::Reset => "RESET",
        }
    }

    fn event_type(self) -> &'static str {
        match self {
            Self::Provision => "PROVISION_TOKEN_CREATED",
            Self::Reset => "RESET_TOKEN_CREATED",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "PROVISION" => Some(Self::Provision),
            "RESET" => Some(Self::Reset),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error(transparent)]
    Authorization(#[from] AuthorizationError),
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("personnel not found")]
    NotFound,
    #[error("stale personnel revision; current revision is {current_revision}")]
    Conflict { current_revision: i64 },
    #[error("credentials are already enabled")]
    AlreadyEnabled,
    #[error("credentials are not enabled")]
    NotEnabled,
    #[error("activation token is invalid or expired")]
    InvalidToken,
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueCredentialTokenRequest {
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialTokenResponse {
    pub user_id: String,
    pub email: String,
    pub purpose: String,
    pub token: String,
    pub expires_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateCredentialRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivateCredentialResponse {
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub auth_enabled: bool,
    pub revision: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordResponse {
    pub changed: bool,
}

#[derive(Clone)]
pub struct CredentialService {
    pool: PgPool,
}

impl CredentialService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn issue_provision_token(
        &self,
        actor: &Actor,
        user_id: &str,
        request: IssueCredentialTokenRequest,
    ) -> Result<CredentialTokenResponse, CredentialError> {
        self.issue_token(actor, user_id, request, TokenPurpose::Provision)
            .await
    }

    pub async fn issue_reset_token(
        &self,
        actor: &Actor,
        user_id: &str,
        request: IssueCredentialTokenRequest,
    ) -> Result<CredentialTokenResponse, CredentialError> {
        self.issue_token(actor, user_id, request, TokenPurpose::Reset)
            .await
    }

    async fn issue_token(
        &self,
        actor: &Actor,
        user_id: &str,
        request: IssueCredentialTokenRequest,
        purpose: TokenPurpose,
    ) -> Result<CredentialTokenResponse, CredentialError> {
        actor.require(Action::CredentialManage)?;
        let user_id = required_id(user_id)?;
        validate_revision(request.expected_revision)?;

        let raw_token = new_one_time_token();
        let token_hash = hash_one_time_token(&raw_token);
        let now = Utc::now();
        let expires_at = now + Duration::hours(TOKEN_TTL_HOURS);
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            r#"
            SELECT
                u.email,
                u.is_active,
                u.revision,
                EXISTS(SELECT 1 FROM app_credentials c WHERE c.user_id = u.id) AS auth_enabled
            FROM app_users u
            WHERE u.id = $1
            FOR UPDATE OF u
            "#,
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CredentialError::NotFound)?;

        let email: Option<String> = row.try_get("email")?;
        let is_active: bool = row.try_get("is_active")?;
        let current_revision: i64 = row.try_get("revision")?;
        let auth_enabled: bool = row.try_get("auth_enabled")?;

        if current_revision != request.expected_revision {
            return Err(CredentialError::Conflict { current_revision });
        }
        if !is_active {
            return Err(CredentialError::Validation(
                "inactive personnel cannot receive authentication credentials".to_string(),
            ));
        }
        let email = email
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                CredentialError::Validation(
                    "personnel must have an e-mail address before authentication can be enabled"
                        .to_string(),
                )
            })?;

        match purpose {
            TokenPurpose::Provision if auth_enabled => return Err(CredentialError::AlreadyEnabled),
            TokenPurpose::Reset if !auth_enabled => return Err(CredentialError::NotEnabled),
            _ => {}
        }

        sqlx::query(
            r#"
            UPDATE auth_one_time_tokens
            SET revoked_at = COALESCE(revoked_at, $2)
            WHERE user_id = $1 AND used_at IS NULL AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let response_revision = if purpose == TokenPurpose::Reset {
            // Mark reset-pending before revoking sessions. AuthService::login locks this same
            // credential row while its final reset-gate check and session insertion run,
            // so a verified old password cannot race a reset into a late valid session.
            sqlx::query(
                r#"
                UPDATE app_credentials
                SET must_change_password = TRUE,
                    failed_attempts = 0,
                    locked_until = NULL,
                    updated_at = $2
                WHERE user_id = $1
                "#,
            )
            .bind(user_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE auth_sessions SET revoked_at = COALESCE(revoked_at, $2) WHERE user_id = $1 AND revoked_at IS NULL",
            )
            .bind(user_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE app_users SET revision = revision + 1, updated_at = $2 WHERE id = $1",
            )
            .bind(user_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            current_revision + 1
        } else {
            current_revision
        };

        sqlx::query(
            r#"
            INSERT INTO auth_one_time_tokens (
                id, user_id, created_by_user_id, purpose, token_hash,
                created_at, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(&actor.user_id)
        .bind(purpose.as_str())
        .bind(&token_hash)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;

        insert_security_event(
            &mut tx,
            user_id,
            Some(&actor.user_id),
            purpose.event_type(),
            now,
            serde_json::json!({ "purpose": purpose.as_str() }),
        )
        .await?;

        tx.commit().await?;

        Ok(CredentialTokenResponse {
            user_id: user_id.to_string(),
            email,
            purpose: purpose.as_str().to_string(),
            token: raw_token,
            expires_at: format_utc(expires_at),
            revision: response_revision,
        })
    }

    pub async fn activate(
        &self,
        request: ActivateCredentialRequest,
    ) -> Result<ActivateCredentialResponse, CredentialError> {
        let token = request.token.trim();
        if token.is_empty() {
            return Err(CredentialError::InvalidToken);
        }
        validate_password(&request.password)?;

        let password = request.password;
        let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
            .await
            .map_err(|_| AuthError::PasswordTask)??;
        let token_hash = hash_one_time_token(token);
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            r#"
            SELECT
                t.id AS token_id,
                t.user_id,
                t.purpose,
                t.expires_at,
                t.used_at,
                t.revoked_at,
                u.email,
                u.role,
                u.is_active,
                u.revision,
                EXISTS(SELECT 1 FROM app_credentials c WHERE c.user_id = u.id) AS auth_enabled
            FROM auth_one_time_tokens t
            JOIN app_users u ON u.id = t.user_id
            WHERE t.token_hash = $1
            FOR UPDATE OF t, u
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CredentialError::InvalidToken)?;

        let token_id: String = row.try_get("token_id")?;
        let user_id: String = row.try_get("user_id")?;
        let purpose_raw: String = row.try_get("purpose")?;
        let purpose = TokenPurpose::parse(&purpose_raw).ok_or(CredentialError::InvalidToken)?;
        let expires_at: DateTime<Utc> = row.try_get("expires_at")?;
        let used_at: Option<DateTime<Utc>> = row.try_get("used_at")?;
        let revoked_at: Option<DateTime<Utc>> = row.try_get("revoked_at")?;
        let email: Option<String> = row.try_get("email")?;
        let role: String = row.try_get("role")?;
        let is_active: bool = row.try_get("is_active")?;
        let current_revision: i64 = row.try_get("revision")?;
        let auth_enabled: bool = row.try_get("auth_enabled")?;

        if used_at.is_some() || revoked_at.is_some() || expires_at <= now || !is_active {
            return Err(CredentialError::InvalidToken);
        }
        match purpose {
            TokenPurpose::Provision if auth_enabled => return Err(CredentialError::InvalidToken),
            TokenPurpose::Reset if !auth_enabled => return Err(CredentialError::InvalidToken),
            _ => {}
        }
        let email = email
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .ok_or(CredentialError::InvalidToken)?;

        sqlx::query(
            r#"
            INSERT INTO app_credentials (
                user_id, password_hash, must_change_password,
                failed_attempts, locked_until, password_changed_at, updated_at
            ) VALUES ($1, $2, FALSE, 0, NULL, $3, $3)
            ON CONFLICT (user_id) DO UPDATE SET
                password_hash = EXCLUDED.password_hash,
                must_change_password = FALSE,
                failed_attempts = 0,
                locked_until = NULL,
                password_changed_at = EXCLUDED.password_changed_at,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&user_id)
        .bind(&password_hash)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let auth_subject = format!("password:{user_id}");
        sqlx::query(
            r#"
            UPDATE app_users
            SET auth_subject = COALESCE(auth_subject, $2),
                revision = revision + 1,
                updated_at = $3
            WHERE id = $1
            "#,
        )
        .bind(&user_id)
        .bind(&auth_subject)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE auth_one_time_tokens SET used_at = $2 WHERE id = $1")
            .bind(&token_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            UPDATE auth_one_time_tokens
            SET revoked_at = COALESCE(revoked_at, $2)
            WHERE user_id = $1 AND id <> $3 AND used_at IS NULL AND revoked_at IS NULL
            "#,
        )
        .bind(&user_id)
        .bind(now)
        .bind(&token_id)
        .execute(&mut *tx)
        .await?;

        insert_security_event(
            &mut tx,
            &user_id,
            Some(&user_id),
            "CREDENTIAL_ACTIVATED",
            now,
            serde_json::json!({ "purpose": purpose.as_str() }),
        )
        .await?;
        tx.commit().await?;

        Ok(ActivateCredentialResponse {
            user_id,
            email,
            role,
            auth_enabled: true,
            revision: current_revision + 1,
        })
    }

    pub async fn change_password(
        &self,
        user_id: &str,
        session_id: &str,
        request: ChangePasswordRequest,
    ) -> Result<ChangePasswordResponse, CredentialError> {
        validate_password(&request.new_password)?;
        if request.current_password == request.new_password {
            return Err(CredentialError::Validation(
                "new password must differ from current password".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT password_hash, must_change_password FROM app_credentials WHERE user_id = $1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;
        let old_hash: String = row.try_get("password_hash")?;
        let reset_pending: bool = row.try_get("must_change_password")?;
        if reset_pending {
            return Err(AuthError::Unauthorized.into());
        }

        let current = request.current_password;
        let old_hash_for_verify = old_hash.clone();
        let valid = tokio::task::spawn_blocking(move || verify_password(&current, &old_hash_for_verify))
            .await
            .map_err(|_| AuthError::PasswordTask)?;
        if !valid {
            return Err(AuthError::InvalidCredentials.into());
        }

        let new_password = request.new_password;
        let new_hash = tokio::task::spawn_blocking(move || hash_password(&new_password))
            .await
            .map_err(|_| AuthError::PasswordTask)??;
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE app_credentials
            SET password_hash = $2,
                must_change_password = FALSE,
                failed_attempts = 0,
                locked_until = NULL,
                password_changed_at = $3,
                updated_at = $3
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .bind(&new_hash)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE auth_sessions SET revoked_at = COALESCE(revoked_at, $3) WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .bind(session_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE auth_one_time_tokens SET revoked_at = COALESCE(revoked_at, $2) WHERE user_id = $1 AND used_at IS NULL AND revoked_at IS NULL",
        )
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        insert_security_event(
            &mut tx,
            user_id,
            Some(user_id),
            "PASSWORD_CHANGED",
            now,
            serde_json::json!({}),
        )
        .await?;
        tx.commit().await?;

        Ok(ChangePasswordResponse { changed: true })
    }
}

async fn insert_security_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_user_id: &str,
    actor_user_id: Option<&str>,
    event_type: &str,
    occurred_at: DateTime<Utc>,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO auth_security_events (
            id, target_user_id, actor_user_id, event_type, occurred_at, payload_json
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(target_user_id)
    .bind(actor_user_id)
    .bind(event_type)
    .bind(occurred_at)
    .bind(payload.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn required_id(value: &str) -> Result<&str, CredentialError> {
    let value = value.trim();
    if value.is_empty() {
        Err(CredentialError::Validation("userId is required".to_string()))
    } else {
        Ok(value)
    }
}

fn validate_revision(revision: i64) -> Result<(), CredentialError> {
    if revision < 0 {
        Err(CredentialError::Validation(
            "expectedRevision must be zero or greater".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_password(password: &str) -> Result<(), CredentialError> {
    let len = password.chars().count();
    if !(MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(&len) {
        return Err(CredentialError::Validation(format!(
            "password must be between {MIN_PASSWORD_LENGTH} and {MAX_PASSWORD_LENGTH} characters"
        )));
    }
    Ok(())
}

fn new_one_time_token() -> String {
    format!(
        "{}.{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn hash_one_time_token(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn format_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
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

impl From<CredentialError> for ApiHttpError {
    fn from(value: CredentialError) -> Self {
        match value {
            CredentialError::Authorization(AuthorizationError::Forbidden) => Self::new(
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Bu işlem için yetkiniz yok.",
            ),
            CredentialError::Authorization(AuthorizationError::InvalidRole(role)) => {
                error!(persisted_role = %role, "unsupported persisted authorization role");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTHORIZATION_INTERNAL_ERROR",
                    "Yetkilendirme işlemi tamamlanamadı.",
                )
            }
            CredentialError::Auth(AuthError::InvalidCredentials) => Self::new(
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "Mevcut parola geçersiz.",
            ),
            CredentialError::Auth(AuthError::Unauthorized) => Self::new(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Oturum gerekli veya oturum geçersiz.",
            ),
            CredentialError::Auth(other) => {
                error!(error = %other, "credential authentication operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTH_INTERNAL_ERROR",
                    "Kimlik doğrulama işlemi tamamlanamadı.",
                )
            }
            CredentialError::Validation(message) => Self::new(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                message,
            ),
            CredentialError::NotFound => Self::new(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Personel bulunamadı.",
            ),
            CredentialError::Conflict { current_revision } => Self::new(
                StatusCode::CONFLICT,
                "STALE_REVISION",
                format!("Personel başka bir kullanıcı tarafından değiştirildi. Güncel revision: {current_revision}."),
            ),
            CredentialError::AlreadyEnabled => Self::new(
                StatusCode::CONFLICT,
                "CREDENTIALS_ALREADY_ENABLED",
                "Bu personel için kimlik bilgileri zaten etkin.",
            ),
            CredentialError::NotEnabled => Self::new(
                StatusCode::CONFLICT,
                "CREDENTIALS_NOT_ENABLED",
                "Bu personel için henüz kimlik bilgisi oluşturulmamış.",
            ),
            CredentialError::InvalidToken => Self::new(
                StatusCode::BAD_REQUEST,
                "AUTH_TOKEN_INVALID",
                "Aktivasyon veya sıfırlama kodu geçersiz ya da süresi dolmuş.",
            ),
            CredentialError::Database(database_error) => {
                error!(error = %database_error, "credential lifecycle operation failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "AUTH_INTERNAL_ERROR",
                    "Kimlik bilgisi işlemi tamamlanamadı.",
                )
            }
        }
    }
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");
    Router::new()
        .route(
            "/api/v1/personnel/{user_id}/auth/invitation",
            post(issue_invitation),
        )
        .route(
            "/api/v1/personnel/{user_id}/auth/reset",
            post(issue_reset),
        )
        .route("/api/v1/auth/activate", post(activate))
        .route("/api/v1/auth/change-password", post(change_password))
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn issue_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<IssueCredentialTokenRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = CredentialService::new(state.pool.clone())
        .issue_provision_token(&actor, &user_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(result)).into_response())
}

async fn issue_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<IssueCredentialTokenRequest>,
) -> Result<Response, ApiHttpError> {
    let actor = authenticated_actor(&state, &headers).await?;
    let result = CredentialService::new(state.pool.clone())
        .issue_reset_token(&actor, &user_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(result)).into_response())
}

async fn activate(
    State(state): State<AppState>,
    Json(request): Json<ActivateCredentialRequest>,
) -> Result<Response, ApiHttpError> {
    let result = CredentialService::new(state.pool.clone())
        .activate(request)
        .await?;
    Ok(Json(result).into_response())
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Response, ApiHttpError> {
    let token = session_token_from_headers(&headers)
        .ok_or(CredentialError::Auth(AuthError::Unauthorized))?;
    let session = AuthService::new(state.pool.clone(), state.session_ttl_hours)
        .resolve(&token)
        .await
        .map_err(CredentialError::from)?;
    let result = CredentialService::new(state.pool.clone())
        .change_password(&session.user.id, &session.session_id, request)
        .await?;
    Ok(Json(result).into_response())
}

async fn authenticated_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Actor, ApiHttpError> {
    let token = session_token_from_headers(headers)
        .ok_or(CredentialError::Auth(AuthError::Unauthorized))?;
    let session = AuthService::new(state.pool.clone(), state.session_ttl_hours)
        .resolve(&token)
        .await
        .map_err(CredentialError::from)?;
    Actor::from_auth_user(&session.user)
        .map_err(|error| ApiHttpError::from(CredentialError::Authorization(error)))
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

    use super::{
        ActivateCredentialRequest, ChangePasswordRequest, CredentialError, CredentialService,
        IssueCredentialTokenRequest,
    };
    use crate::{
        auth::{AuthError, AuthService, ClientKind},
        authz::{Actor, Role},
        db::run_migrations,
    };

    const ADMIN_ID: &str = "m6-credential-admin";
    const STAFF_ID: &str = "m6-credential-sales";
    const STAFF_EMAIL: &str = "m6-credential-sales@example.test";

    async fn cleanup(pool: &sqlx::PgPool) {
        sqlx::query("DELETE FROM app_users WHERE id IN ($1, $2)")
            .bind(ADMIN_ID)
            .bind(STAFF_ID)
            .execute(pool)
            .await
            .expect("credential lifecycle cleanup");
    }

    #[tokio::test]
    async fn postgres_credential_invitation_activation_change_and_reset_are_safe() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping credential lifecycle test");
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect credential test postgres");
        run_migrations(&pool).await.expect("migrations");
        cleanup(&pool).await;

        sqlx::query(
            "INSERT INTO app_users (id, display_name, email, role, is_active, revision, created_at, updated_at) VALUES ($1, 'Credential Admin', 'credential-admin@example.test', 'ADMIN', TRUE, 0, now(), now()), ($2, 'Credential Sales', $3, 'SALES', TRUE, 0, now(), now())",
        )
        .bind(ADMIN_ID)
        .bind(STAFF_ID)
        .bind(STAFF_EMAIL)
        .execute(&pool)
        .await
        .expect("seed credential users");

        let service = CredentialService::new(pool.clone());
        let admin = Actor { user_id: ADMIN_ID.to_string(), role: Role::Admin };
        let manager = Actor { user_id: "manager-no-access".to_string(), role: Role::Manager };

        let denied = service
            .issue_provision_token(
                &manager,
                STAFF_ID,
                IssueCredentialTokenRequest { expected_revision: 0 },
            )
            .await
            .expect_err("manager credential provisioning must fail");
        assert!(matches!(denied, CredentialError::Authorization(_)));

        let invitation = service
            .issue_provision_token(
                &admin,
                STAFF_ID,
                IssueCredentialTokenRequest { expected_revision: 0 },
            )
            .await
            .expect("issue invitation");
        assert_eq!(invitation.purpose, "PROVISION");
        assert_eq!(invitation.revision, 0);
        let stored_hash: String = sqlx::query_scalar(
            "SELECT token_hash FROM auth_one_time_tokens WHERE user_id = $1 AND purpose = 'PROVISION' AND revoked_at IS NULL",
        )
        .bind(STAFF_ID)
        .fetch_one(&pool)
        .await
        .expect("stored token hash");
        assert_ne!(stored_hash, invitation.token);

        let activated = service
            .activate(ActivateCredentialRequest {
                token: invitation.token.clone(),
                password: "first-credential-password".to_string(),
            })
            .await
            .expect("activate invitation");
        assert!(activated.auth_enabled);
        assert_eq!(activated.revision, 1);
        assert!(matches!(
            service
                .activate(ActivateCredentialRequest {
                    token: invitation.token,
                    password: "another-credential-password".to_string(),
                })
                .await,
            Err(CredentialError::InvalidToken)
        ));

        let auth = AuthService::new(pool.clone(), 12);
        let session_one = auth
            .login(STAFF_EMAIL, "first-credential-password", ClientKind::Tauri)
            .await
            .expect("first login");
        let session_two = auth
            .login(STAFF_EMAIL, "first-credential-password", ClientKind::Tauri)
            .await
            .expect("second login");

        service
            .change_password(
                STAFF_ID,
                &session_one.session_id,
                ChangePasswordRequest {
                    current_password: "first-credential-password".to_string(),
                    new_password: "second-credential-password".to_string(),
                },
            )
            .await
            .expect("change password");
        auth.resolve(&session_one.token)
            .await
            .expect("current session remains valid after password change");
        assert!(matches!(auth.resolve(&session_two.token).await, Err(AuthError::Unauthorized)));
        assert!(matches!(
            auth.login(STAFF_EMAIL, "first-credential-password", ClientKind::Tauri).await,
            Err(AuthError::InvalidCredentials)
        ));
        let post_change = auth
            .login(STAFF_EMAIL, "second-credential-password", ClientKind::Tauri)
            .await
            .expect("new password login");

        let reset = service
            .issue_reset_token(
                &admin,
                STAFF_ID,
                IssueCredentialTokenRequest { expected_revision: 1 },
            )
            .await
            .expect("issue reset");
        assert_eq!(reset.purpose, "RESET");
        assert_eq!(reset.revision, 2);
        assert!(matches!(auth.resolve(&session_one.token).await, Err(AuthError::Unauthorized)));
        assert!(matches!(auth.resolve(&post_change.token).await, Err(AuthError::Unauthorized)));
        assert!(matches!(
            auth.login(STAFF_EMAIL, "second-credential-password", ClientKind::Tauri).await,
            Err(AuthError::InvalidCredentials)
        ));

        let reset_activation = service
            .activate(ActivateCredentialRequest {
                token: reset.token,
                password: "third-credential-password".to_string(),
            })
            .await
            .expect("activate reset");
        assert_eq!(reset_activation.revision, 3);
        auth.login(STAFF_EMAIL, "third-credential-password", ClientKind::Tauri)
            .await
            .expect("login after reset activation");

        let event_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM auth_security_events WHERE target_user_id = $1",
        )
        .bind(STAFF_ID)
        .fetch_one(&pool)
        .await
        .expect("security event count");
        assert_eq!(event_count, 5);

        cleanup(&pool).await;
        pool.close().await;
    }
}
