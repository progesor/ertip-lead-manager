use sqlx::{PgPool, migrate::Migrator};

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::run_migrations;

    #[tokio::test]
    async fn canonical_migrations_apply_to_real_postgres_when_configured() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping PostgreSQL migration integration test");
            return;
        };

        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("connect to PostgreSQL test database");

        run_migrations(&pool)
            .await
            .expect("apply canonical PostgreSQL migrations");

        for table in [
            "app_users",
            "lead_contacts",
            "import_batches",
            "lead_submissions",
            "submission_product_interests",
            "contact_product_interest_overrides",
            "lead_notes",
            "lead_activities",
            "follow_ups",
            "lead_data_quality_issues",
            "app_credentials",
            "auth_sessions",
        ] {
            let qualified = format!("public.{table}");
            let exists = sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL")
                .bind(&qualified)
                .fetch_one(&pool)
                .await
                .expect("check migrated table");
            assert!(exists, "missing table {table}");
        }

        let mut transaction = pool.begin().await.expect("begin constraint test");
        let invalid_role = sqlx::query(
            r#"
            INSERT INTO app_users (
                id, display_name, role, is_active, created_at, updated_at
            ) VALUES (
                'invalid-role-user', 'Constraint Test', 'ROOT', TRUE,
                '2026-08-25T00:00:00Z', '2026-08-25T00:00:00Z'
            )
            "#,
        )
        .execute(&mut *transaction)
        .await;
        assert!(invalid_role.is_err(), "role CHECK constraint must reject unsupported values");
        transaction.rollback().await.expect("rollback constraint test");

        pool.close().await;
    }
}
