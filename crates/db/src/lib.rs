use std::time::Duration;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::MigratorTrait;
use utils::assets::asset_dir;

pub mod events;
pub mod models;
pub mod entities;
pub mod types;

#[derive(Clone)]
pub struct DBService {
    pub pool: DatabaseConnection,
}

pub type DbPool = DatabaseConnection;
pub use sea_orm::DbErr;
pub use sea_orm::TransactionTrait;

fn default_sqlite_url() -> String {
    let db_path = asset_dir().join("db.sqlite");
    format!("sqlite://{}?mode=rwc", db_path.to_string_lossy())
}

fn resolve_database_url() -> Result<String, DbErr> {
    match std::env::var("DATABASE_URL") {
        Ok(url) => {
            let trimmed = url.trim();
            if trimmed.is_empty() {
                return Err(DbErr::Custom(
                    "DATABASE_URL is set but empty".to_string(),
                ));
            }
            // Postgres URLs (e.g. postgres://...) are rejected until supported.
            if !trimmed.starts_with("sqlite:") {
                return Err(DbErr::Custom(
                    "Only sqlite DATABASE_URL values are supported for now".to_string(),
                ));
            }
            Ok(trimmed.to_string())
        }
        Err(std::env::VarError::NotPresent) => Ok(default_sqlite_url()),
        Err(err) => Err(DbErr::Custom(format!(
            "Failed to read DATABASE_URL: {err}"
        ))),
    }
}

impl DBService {
    pub async fn new() -> Result<DBService, DbErr> {
        // Use DATABASE_URL when present; otherwise fall back to the project SQLite path.
        // DATABASE_URL accepts SQLite URLs like `sqlite:./db.sqlite?mode=rwc` or `sqlite::memory:`.
        let database_url = resolve_database_url()?;
        let mut options = ConnectOptions::new(database_url);
        options
            .max_connections(5)
            .connect_timeout(Duration::from_secs(30))
            .sqlx_logging(false)
            .after_connect(|conn| {
                Box::pin(async move {
                    let backend = conn.get_database_backend();
                    if backend == DatabaseBackend::Sqlite {
                        conn.execute_raw(Statement::from_string(
                            backend,
                            "PRAGMA foreign_keys = ON;",
                        ))
                        .await?;
                    }
                    Ok(())
                })
            })
            .map_sqlx_sqlite_opts(|opts| {
                opts.pragma("journal_mode", "WAL")
                    .pragma("synchronous", "NORMAL")
                    .busy_timeout(Duration::from_secs(30))
            });
        let pool = Database::connect(options).await?;
        db_migration::Migrator::up(&pool, None).await?;
        Ok(DBService { pool })
    }
}
