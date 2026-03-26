use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let (args, action) =
        server::legacy_migrations::parse_export_db_projects_yaml_args(std::env::args().skip(1))?;

    if action == server::legacy_migrations::ExportDbProjectsYamlParseResult::Help {
        println!(
            "{}",
            server::legacy_migrations::export_db_projects_yaml_help()
        );
        return Ok(());
    }

    server::legacy_migrations::run_export_db_projects_yaml(args).await
}
