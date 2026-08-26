use std::{env, path::PathBuf, process::ExitCode};

use sqlx::postgres::PgPoolOptions;

#[path = "../sqlite_migration.rs"]
mod sqlite_migration;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("migration failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 || args[0] != "--execute" {
        return Err(
            "usage: migrate_sqlite_v4 --execute <path-to-schema-v4-sqlite-database>".into(),
        );
    }

    let sqlite_path = PathBuf::from(&args[1]);
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL must point to the private target PostgreSQL database")?;
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        return Err("DATABASE_URL must use postgres:// or postgresql://".into());
    }

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let report = sqlite_migration::migrate_sqlite_v4(&sqlite_path, &pool).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    pool.close().await;
    Ok(())
}
