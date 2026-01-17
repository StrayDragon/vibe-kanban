use std::path::{Path, PathBuf};
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

fn reset_db_on_migration_error() -> bool {
    match std::env::var("VIBE_DB_RESET_ON_MIGRATION_ERROR") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        Err(_) => false,
    }
}

fn build_connect_options(database_url: &str) -> Result<ConnectOptions, DbErr> {
    let mut options = ConnectOptions::new(database_url.to_string());
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
        // If Postgres (or multi-backend) is enabled later, add map_sqlx_postgres_opts too.
        .map_sqlx_sqlite_opts(|opts| {
            opts.pragma("journal_mode", "WAL")
                .pragma("synchronous", "NORMAL")
                .busy_timeout(Duration::from_secs(30))
        });
    Ok(options)
}

fn sqlite_path_from_url(database_url: &str) -> Option<PathBuf> {
    let trimmed = database_url.trim();
    if !trimmed.starts_with("sqlite:") {
        return None;
    }
    let mut rest = &trimmed["sqlite:".len()..];
    if rest.starts_with("//") {
        rest = &rest[2..];
    }
    let path_part = rest.split('?').next().unwrap_or(rest);
    if path_part.is_empty() || path_part == ":memory:" {
        return None;
    }
    Some(PathBuf::from(path_part))
}

fn reset_sqlite_files(db_path: &Path) -> Result<(), DbErr> {
    let wal_path = PathBuf::from(format!("{}-wal", db_path.to_string_lossy()));
    let shm_path = PathBuf::from(format!("{}-shm", db_path.to_string_lossy()));
    for path in [db_path.to_path_buf(), wal_path, shm_path] {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|err| {
                DbErr::Custom(format!(
                    "Failed to remove sqlite file {}: {err}",
                    path.to_string_lossy()
                ))
            })?;
        }
    }
    Ok(())
}

impl DBService {
    pub async fn new() -> Result<DBService, DbErr> {
        // Use DATABASE_URL when present; otherwise fall back to the project SQLite path.
        // DATABASE_URL accepts SQLite URLs like `sqlite:./db.sqlite?mode=rwc` or `sqlite::memory:`.
        let database_url = resolve_database_url()?;
        let options = build_connect_options(&database_url)?;
        let pool = Database::connect(options).await?;
        if let Err(err) = db_migration::Migrator::up(&pool, None).await {
            if reset_db_on_migration_error() {
                tracing::warn!(?err, "migration failed; resetting database");
                if let Some(db_path) = sqlite_path_from_url(&database_url) {
                    let _ = pool.close().await;
                    reset_sqlite_files(&db_path)?;
                    let options = build_connect_options(&database_url)?;
                    let pool = Database::connect(options).await?;
                    db_migration::Migrator::up(&pool, None).await?;
                    return Ok(DBService { pool });
                }
                db_migration::Migrator::fresh(&pool).await?;
                db_migration::Migrator::up(&pool, None).await?;
            } else {
                tracing::error!(
                    ?err,
                    "migration failed; set VIBE_DB_RESET_ON_MIGRATION_ERROR=1 to reset the local SQLite database"
                );
                return Err(err);
            }
        }
        Ok(DBService { pool })
    }
}
