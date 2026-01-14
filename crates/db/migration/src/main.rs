use sea_orm_migration::cli;

#[tokio::main]
async fn main() {
    cli::run_cli(db_migration::Migrator).await;
}
