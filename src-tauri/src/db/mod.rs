use std::path::{Path, PathBuf};

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

use crate::error::AppError;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    path: PathBuf,
}

impl Database {
    pub async fn connect(path: PathBuf) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePool::connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool, path })
    }

    #[cfg(test)]
    pub async fn connect_memory() -> Result<Self, AppError> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePool::connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self {
            pool,
            path: PathBuf::from(":memory:"),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn schema_version(&self) -> Result<i64, AppError> {
        let version = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::Database;
    use crate::repositories::contact_repository::ContactRepository;

    #[tokio::test]
    async fn migrations_apply_to_an_empty_database() {
        let database = Database::connect_memory().await.expect("open test database");
        let version = database.schema_version().await.expect("read schema version");

        assert_eq!(version, 3);
    }

    #[tokio::test]
    async fn contact_repository_round_trip_works_after_migration() {
        let database = Database::connect_memory().await.expect("open test database");
        let repository = ContactRepository::new(database.pool().clone());

        repository
            .create_minimal("contact-test-1", "Demo Contact")
            .await
            .expect("insert contact");

        let contact = repository
            .find_by_id("contact-test-1")
            .await
            .expect("read contact")
            .expect("contact exists");

        assert_eq!(contact.id, "contact-test-1");
        assert_eq!(contact.display_name.as_deref(), Some("Demo Contact"));
        assert_eq!(contact.status, "NEW");
    }
}
