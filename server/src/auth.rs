use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::config::BootstrapAdmin;

const MAX_FAILED_ATTEMPTS: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Web,
    Tauri,
}

impl ClientKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Web => "WEB",
            Self::Tauri => "TAURI",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct LoginSession {
    pub session_id: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: AuthUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapStatus {
    Created(String),
    ExistingUsers,
    NotConfigured,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("password hashing failed")]
    PasswordHash,
    #[error("password worker failed")]
    PasswordTask,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("account is temporarily locked")]
    TemporarilyLocked,
    #[error("authentication is required")]
    Unauthorized,
}

#[derive(Clone)]
pub struct AuthService {
    pool: PgPool,
    session_ttl_hours: i64,
}

impl AuthService {
    pub fn new(pool: PgPool, session_ttl_hours: i64) -> Self {
        Self {
            pool,
            session_ttl_hours,
        }
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        client_kind: ClientKind,
    ) -> Result<LoginSession, AuthError> {
        let email = email.trim().to_ascii_lowercase();
        if email.is_empty() || password.is_empty() {
            return Err(AuthError::InvalidCredentials);
        }

        let row = sqlx::query(
            r#"
            SELECT
                u.id,
                u.display_name,
                u.email,
                u.role,
                u.is_active,
                c.password_hash,
                c.locked_until
            FROM app_users u
            JOIN app_credentials c ON c.user_id = u.id
            WHERE lower(u.email) = lower($1)
            LIMIT 1
            "#,
        )
        .bind(&email)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(AuthError::InvalidCredentials);
        };

        let user_id: String = row.try_get("id")?;
        let is_active: bool = row.try_get("is_active")?;
        let password_hash: String = row.try_get("password_hash")?;
        let locked_until: Option<DateTime<Utc>> = row.try_get("locked_until")?;

        if !is_active {
            return Err(AuthError::InvalidCredentials);
        }

        if locked_until.is_some_and(|until| until > Utc::now()) {
            return Err(AuthError::TemporarilyLocked);
        }

        let password_owned = password.to_string();
        let hash_owned = password_hash.clone();
        let password_valid = tokio::task::spawn_blocking(move || {
            verify_password(&password_owned, &hash_owned)
        })
        .await
        .map_err(|_| AuthError::PasswordTask)?;

        if !password_valid {
            self.record_failed_login(&user_id).await?;
            return Err(AuthError::InvalidCredentials);
        }

        sqlx::query(
            "UPDATE app_credentials SET failed_attempts = 0, locked_until = NULL, updated_at = now() WHERE user_id = $1",
        )
        .bind(&user_id)
        .execute(&self.pool)
        .await?;

        let user = AuthUser {
            id: user_id.clone(),
            display_name: row.try_get("display_name")?,
            email: row.try_get("email")?,
            role: row.try_get("role")?,
        };

        let raw_token = new_session_token();
        let session_id = Uuid::new_v4().to_string();
        let token_hash = hash_session_token(&raw_token);
        let now = Utc::now();
        let expires_at = now + Duration::hours(self.session_ttl_hours);

        sqlx::query(
            r#"
            INSERT INTO auth_sessions (
                id, user_id, token_hash, client_kind,
                created_at, expires_at, last_seen_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $5)
            "#,
        )
        .bind(&session_id)
        .bind(&user_id)
        .bind(&token_hash)
        .bind(client_kind.as_str())
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(LoginSession {
            session_id,
            token: raw_token,
            expires_at,
            user,
        })
    }

    pub async fn resolve(&self, raw_token: &str) -> Result<LoginSession, AuthError> {
        if raw_token.trim().is_empty() {
            return Err(AuthError::Unauthorized);
        }

        let token_hash = hash_session_token(raw_token.trim());
        let row = sqlx::query(
            r#"
            SELECT
                s.id AS session_id,
                s.expires_at,
                u.id,
                u.display_name,
                u.email,
                u.role
            FROM auth_sessions s
            JOIN app_users u ON u.id = s.user_id
            WHERE s.token_hash = $1
              AND s.revoked_at IS NULL
              AND s.expires_at > now()
              AND u.is_active = TRUE
            LIMIT 1
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(AuthError::Unauthorized);
        };

        let session_id: String = row.try_get("session_id")?;
        sqlx::query("UPDATE auth_sessions SET last_seen_at = now() WHERE id = $1")
            .bind(&session_id)
            .execute(&self.pool)
            .await?;

        Ok(LoginSession {
            session_id,
            token: raw_token.trim().to_string(),
            expires_at: row.try_get("expires_at")?,
            user: AuthUser {
                id: row.try_get("id")?,
                display_name: row.try_get("display_name")?,
                email: row.try_get("email")?,
                role: row.try_get("role")?,
            },
        })
    }

    pub async fn logout(&self, session_id: &str) -> Result<(), AuthError> {
        sqlx::query(
            "UPDATE auth_sessions SET revoked_at = COALESCE(revoked_at, now()) WHERE id = $1",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_failed_login(&self, user_id: &str) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE app_credentials
            SET
                failed_attempts = failed_attempts + 1,
                locked_until = CASE
                    WHEN failed_attempts + 1 >= $2
                    THEN now() + interval '15 minutes'
                    ELSE locked_until
                END,
                updated_at = now()
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .bind(MAX_FAILED_ATTEMPTS)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

pub async fn bootstrap_admin(
    pool: &PgPool,
    bootstrap: Option<&BootstrapAdmin>,
) -> Result<BootstrapStatus, AuthError> {
    let existing_users = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM app_users")
        .fetch_one(pool)
        .await?;

    if existing_users > 0 {
        return Ok(BootstrapStatus::ExistingUsers);
    }

    let Some(bootstrap) = bootstrap else {
        return Ok(BootstrapStatus::NotConfigured);
    };

    let password = bootstrap.password.clone();
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| AuthError::PasswordTask)??;

    let user_id = Uuid::new_v4().to_string();
    let auth_subject = format!("password:{user_id}");
    let now = Utc::now();
    let mut transaction = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO app_users (
            id, display_name, email, role, is_active,
            auth_subject, created_at, updated_at
        ) VALUES ($1, $2, $3, 'ADMIN', TRUE, $4, $5, $5)
        "#,
    )
    .bind(&user_id)
    .bind(&bootstrap.display_name)
    .bind(&bootstrap.email)
    .bind(&auth_subject)
    .bind(now)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO app_credentials (
            user_id, password_hash, password_changed_at, updated_at
        ) VALUES ($1, $2, $3, $3)
        "#,
    )
    .bind(&user_id)
    .bind(&password_hash)
    .bind(now)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(BootstrapStatus::Created(user_id))
}

pub(crate) fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
        .map_err(|_| AuthError::PasswordHash)?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHash)
}

pub(crate) fn verify_password(password: &str, encoded: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn new_session_token() -> String {
    format!(
        "{}.{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

pub fn hash_session_token(raw_token: &str) -> String {
    hex::encode(Sha256::digest(raw_token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::{
        AuthError, AuthService, BootstrapStatus, ClientKind, bootstrap_admin, hash_session_token,
    };
    use crate::{config::BootstrapAdmin, db::run_migrations};

    #[test]
    fn session_token_hash_is_deterministic_without_storing_raw_token() {
        let hash = hash_session_token("example-token");
        assert_eq!(hash.len(), 64);
        assert_eq!(hash, hash_session_token("example-token"));
        assert_ne!(hash, "example-token");
    }

    #[tokio::test]
    async fn bootstrap_login_resolve_and_logout_work_against_postgres_when_configured() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping auth integration test");
            return;
        };

        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect to PostgreSQL test database");
        run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM auth_sessions")
            .execute(&pool)
            .await
            .expect("clear sessions");
        sqlx::query("DELETE FROM app_credentials")
            .execute(&pool)
            .await
            .expect("clear credentials");
        sqlx::query("DELETE FROM app_users")
            .execute(&pool)
            .await
            .expect("clear users");

        let bootstrap = BootstrapAdmin {
            display_name: "Integration Admin".to_string(),
            email: "integration-admin@example.test".to_string(),
            password: "correct-horse-battery-staple".to_string(),
        };
        let created = bootstrap_admin(&pool, Some(&bootstrap))
            .await
            .expect("bootstrap admin");
        assert!(matches!(created, BootstrapStatus::Created(_)));

        let service = AuthService::new(pool.clone(), 12);
        assert!(matches!(
            service
                .login(
                    "integration-admin@example.test",
                    "wrong-password",
                    ClientKind::Tauri,
                )
                .await,
            Err(AuthError::InvalidCredentials)
        ));

        let login = service
            .login(
                "INTEGRATION-ADMIN@example.test",
                "correct-horse-battery-staple",
                ClientKind::Tauri,
            )
            .await
            .expect("login");
        assert_eq!(login.user.role, "ADMIN");
        assert_eq!(login.token.len(), 65);

        let resolved = service.resolve(&login.token).await.expect("resolve session");
        assert_eq!(resolved.user.id, login.user.id);
        assert_eq!(resolved.session_id, login.session_id);

        service.logout(&login.session_id).await.expect("logout");
        assert!(matches!(
            service.resolve(&login.token).await,
            Err(AuthError::Unauthorized)
        ));

        pool.close().await;
    }
}
