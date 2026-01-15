use std::time::Duration;

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
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

impl DBService {
    pub async fn new() -> Result<DBService, DbErr> {
        let db_path = asset_dir().join("db.sqlite");
        let database_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

        if db_path.exists() {
            let mut check_options = ConnectOptions::new(database_url.clone());
            check_options.sqlx_logging(false).map_sqlx_sqlite_opts(|opts| {
                opts.pragma("journal_mode", "WAL")
                    .pragma("synchronous", "NORMAL")
                    .busy_timeout(Duration::from_secs(30))
            });
            let check_pool = Database::connect(check_options).await?;
            let has_migrations = check_pool
                .query_one_raw(Statement::from_string(
                    check_pool.get_database_backend(),
                    "SELECT name FROM sqlite_master WHERE type='table' AND name='seaql_migrations'",
                ))
                .await?
                .is_some();
            drop(check_pool);

            if !has_migrations {
                std::fs::remove_file(&db_path)
                    .map_err(|err| DbErr::Custom(err.to_string()))?;
            }
        }

        let mut options = ConnectOptions::new(database_url);
        options
            .max_connections(5)
            .connect_timeout(Duration::from_secs(30))
            .sqlx_logging(false)
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
