use std::{env, net::SocketAddr};

use thiserror::Error;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_DB_MAX_CONNECTIONS: &str = "10";

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub db_max_connections: u32,
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
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_values(
            env::var("ELM_BIND_ADDR").ok().as_deref(),
            env::var("DATABASE_URL").ok().as_deref(),
            env::var("ELM_DB_MAX_CONNECTIONS").ok().as_deref(),
        )
    }

    fn from_values(
        bind_addr: Option<&str>,
        database_url: Option<&str>,
        max_connections: Option<&str>,
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

        Ok(Self {
            bind_addr,
            database_url,
            db_max_connections,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};

    #[test]
    fn defaults_are_deterministic() {
        let config = Config::from_values(None, Some("postgres://user:pass@db/app"), None)
            .expect("valid config");

        assert_eq!(config.bind_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.db_max_connections, 10);
    }

    #[test]
    fn database_url_is_required_and_must_be_postgres() {
        assert_eq!(
            Config::from_values(None, None, None).expect_err("missing DB URL must fail"),
            ConfigError::MissingDatabaseUrl
        );
        assert_eq!(
            Config::from_values(None, Some("sqlite://local.db"), None)
                .expect_err("non-postgres URL must fail"),
            ConfigError::InvalidDatabaseUrl
        );
    }

    #[test]
    fn max_connections_is_bounded() {
        assert_eq!(
            Config::from_values(None, Some("postgres://u:p@db/app"), Some("0"))
                .expect_err("zero max connections must fail"),
            ConfigError::InvalidMaxConnections
        );
        assert_eq!(
            Config::from_values(None, Some("postgres://u:p@db/app"), Some("101"))
                .expect_err("too many connections must fail"),
            ConfigError::InvalidMaxConnections
        );
    }
}
