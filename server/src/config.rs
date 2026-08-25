use std::{env, net::SocketAddr};

use thiserror::Error;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_DB_MAX_CONNECTIONS: &str = "10";
const DEFAULT_SESSION_TTL_HOURS: &str = "12";
const DEFAULT_BOOTSTRAP_ADMIN_NAME: &str = "Ertip Admin";

#[derive(Clone)]
pub struct BootstrapAdmin {
    pub display_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub db_max_connections: u32,
    pub session_ttl_hours: i64,
    pub bootstrap_admin: Option<BootstrapAdmin>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("DATABASE_URL is required")]
    MissingDatabaseUrl,
    #[error("DATABASE_URL must use postgres:// or postgresql://")]
    InvalidDatabaseUrl,
    #[error("ELM_BIND_ADDR is invalid: {0}")]
    InvalidBindAddr(String),
    #[error("ELM_DB_MAX_CONNECTIONS must be an integer between 1 and 100")]
    InvalidMaxConnections,
    #[error("ELM_SESSION_TTL_HOURS must be an integer between 1 and 720")]
    InvalidSessionTtl,
    #[error("ELM_BOOTSTRAP_ADMIN_EMAIL and ELM_BOOTSTRAP_ADMIN_PASSWORD must be configured together")]
    IncompleteBootstrapAdmin,
    #[error("ELM_BOOTSTRAP_ADMIN_EMAIL is invalid")]
    InvalidBootstrapEmail,
    #[error("ELM_BOOTSTRAP_ADMIN_PASSWORD must be between 12 and 128 characters")]
    InvalidBootstrapPassword,
    #[error("ELM_BOOTSTRAP_ADMIN_NAME must be between 2 and 100 characters")]
    InvalidBootstrapName,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_values(
            env::var("ELM_BIND_ADDR").ok().as_deref(),
            env::var("DATABASE_URL").ok().as_deref(),
            env::var("ELM_DB_MAX_CONNECTIONS").ok().as_deref(),
            env::var("ELM_SESSION_TTL_HOURS").ok().as_deref(),
            env::var("ELM_BOOTSTRAP_ADMIN_EMAIL").ok().as_deref(),
            env::var("ELM_BOOTSTRAP_ADMIN_PASSWORD").ok().as_deref(),
            env::var("ELM_BOOTSTRAP_ADMIN_NAME").ok().as_deref(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_values(
        bind_addr: Option<&str>,
        database_url: Option<&str>,
        max_connections: Option<&str>,
        session_ttl_hours: Option<&str>,
        bootstrap_email: Option<&str>,
        bootstrap_password: Option<&str>,
        bootstrap_name: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let bind_raw = bind_addr.unwrap_or(DEFAULT_BIND_ADDR).trim();
        let bind_addr = bind_raw
            .parse::<SocketAddr>()
            .map_err(|_| ConfigError::InvalidBindAddr(bind_raw.to_string()))?;

        let database_url = database_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(ConfigError::MissingDatabaseUrl)?
            .to_string();

        if !(database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")) {
            return Err(ConfigError::InvalidDatabaseUrl);
        }

        let max_raw = max_connections.unwrap_or(DEFAULT_DB_MAX_CONNECTIONS).trim();
        let db_max_connections = max_raw
            .parse::<u32>()
            .ok()
            .filter(|value| (1..=100).contains(value))
            .ok_or(ConfigError::InvalidMaxConnections)?;

        let ttl_raw = session_ttl_hours.unwrap_or(DEFAULT_SESSION_TTL_HOURS).trim();
        let session_ttl_hours = ttl_raw
            .parse::<i64>()
            .ok()
            .filter(|value| (1..=720).contains(value))
            .ok_or(ConfigError::InvalidSessionTtl)?;

        let bootstrap_admin = parse_bootstrap_admin(
            bootstrap_email,
            bootstrap_password,
            bootstrap_name,
        )?;

        Ok(Self {
            bind_addr,
            database_url,
            db_max_connections,
            session_ttl_hours,
            bootstrap_admin,
        })
    }
}

fn parse_bootstrap_admin(
    email: Option<&str>,
    password: Option<&str>,
    name: Option<&str>,
) -> Result<Option<BootstrapAdmin>, ConfigError> {
    let email = email.map(str::trim).filter(|value| !value.is_empty());
    let password = password.filter(|value| !value.is_empty());

    match (email, password) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(ConfigError::IncompleteBootstrapAdmin),
        (Some(email), Some(password)) => {
            let normalized_email = email.to_ascii_lowercase();
            let valid_email = normalized_email.len() <= 254
                && normalized_email.contains('@')
                && !normalized_email.starts_with('@')
                && !normalized_email.ends_with('@')
                && !normalized_email.chars().any(char::is_whitespace);
            if !valid_email {
                return Err(ConfigError::InvalidBootstrapEmail);
            }

            if !(12..=128).contains(&password.chars().count()) {
                return Err(ConfigError::InvalidBootstrapPassword);
            }

            let display_name = name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_BOOTSTRAP_ADMIN_NAME)
                .to_string();
            if !(2..=100).contains(&display_name.chars().count()) {
                return Err(ConfigError::InvalidBootstrapName);
            }

            Ok(Some(BootstrapAdmin {
                display_name,
                email: normalized_email,
                password: password.to_string(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};

    fn config(
        db: Option<&str>,
        max_connections: Option<&str>,
        ttl: Option<&str>,
        email: Option<&str>,
        password: Option<&str>,
    ) -> Result<Config, ConfigError> {
        Config::from_values(None, db, max_connections, ttl, email, password, None)
    }

    #[test]
    fn defaults_are_deterministic() {
        let config = config(Some("postgres://user:pass@db/app"), None, None, None, None)
            .expect("valid config");

        assert_eq!(config.bind_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.db_max_connections, 10);
        assert_eq!(config.session_ttl_hours, 12);
        assert!(config.bootstrap_admin.is_none());
    }

    #[test]
    fn database_url_is_required_and_must_be_postgres() {
        assert!(matches!(
            config(None, None, None, None, None),
            Err(ConfigError::MissingDatabaseUrl)
        ));
        assert!(matches!(
            config(Some("sqlite://local.db"), None, None, None, None),
            Err(ConfigError::InvalidDatabaseUrl)
        ));
    }

    #[test]
    fn numeric_limits_are_bounded() {
        assert!(matches!(
            config(Some("postgres://u:p@db/app"), Some("0"), None, None, None),
            Err(ConfigError::InvalidMaxConnections)
        ));
        assert!(matches!(
            config(Some("postgres://u:p@db/app"), None, Some("721"), None, None),
            Err(ConfigError::InvalidSessionTtl)
        ));
    }

    #[test]
    fn bootstrap_admin_requires_complete_strong_configuration() {
        assert!(matches!(
            config(
                Some("postgres://u:p@db/app"),
                None,
                None,
                Some("admin@example.test"),
                None,
            ),
            Err(ConfigError::IncompleteBootstrapAdmin)
        ));

        assert!(matches!(
            config(
                Some("postgres://u:p@db/app"),
                None,
                None,
                Some("admin@example.test"),
                Some("short"),
            ),
            Err(ConfigError::InvalidBootstrapPassword)
        ));

        let config = config(
            Some("postgres://u:p@db/app"),
            None,
            None,
            Some("ADMIN@Example.Test"),
            Some("correct-horse-battery-staple"),
        )
        .expect("valid bootstrap config");
        let bootstrap = config.bootstrap_admin.expect("bootstrap admin");
        assert_eq!(bootstrap.email, "admin@example.test");
        assert_eq!(bootstrap.display_name, "Ertip Admin");
    }
}
