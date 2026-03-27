use anyhow::Result;

mod config_cmd;
mod migrate;

fn print_help() {
    println!(
        r#"vk (Vibe Kanban operator CLI)

Usage:
  vk --help
  vk migrate <command> [args]
  vk config <command> [args]

Commands:
  vk migrate export-db-projects-yaml    Export legacy DB-backed projects to YAML (output-only)
  vk migrate export-asset-config-yaml   Export legacy asset config (config.json/profiles.json) to YAML + secret.env (output-only)
  vk migrate prompt                     Print an AI prompt to help merge migrated files
  vk config schema upsert               Write/update config.schema.json + projects.schema.json
"#
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    let first = args[0].as_str();
    if matches!(first, "--help" | "-h" | "help") {
        print_help();
        return Ok(());
    }

    match first {
        "migrate" => migrate::run(args.into_iter().skip(1).collect()).await,
        "config" => config_cmd::run(args.into_iter().skip(1).collect()).await,
        other => anyhow::bail!("Unknown command: {other}. Run `vk --help` for usage."),
    }
}
