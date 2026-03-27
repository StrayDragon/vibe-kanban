use anyhow::Result;

pub mod asset_config;
pub mod db_projects;
pub mod io;
pub mod secrets;

fn print_help() {
    println!(
        r#"vk migrate

Usage:
  vk migrate export-db-projects-yaml [--out <path>|--install|--print-paths] [--dry-run]
  vk migrate export-asset-config-yaml [--out <path>|--install|--print-paths] [--dry-run]
  vk migrate prompt
"#
    );
}

pub async fn run(args: Vec<String>) -> Result<()> {
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
        "export-db-projects-yaml" => db_projects::run(args.into_iter().skip(1).collect()).await,
        "export-asset-config-yaml" => asset_config::run(args.into_iter().skip(1).collect()).await,
        "prompt" => run_prompt(),
        other => anyhow::bail!("Unknown vk migrate command: {other}. Run `vk migrate --help`."),
    }
}

fn run_prompt() -> Result<()> {
    let config_dir = utils_core::vk_config_dir();
    let asset_dir = utils_assets::asset_dir();

    println!(
        r#"You are a migration assistant for vibe-kanban (VK).

Goal:
- Migrate legacy configuration into the YAML file-first config directory.

Inputs (legacy sources):
- DB: DATABASE_URL or {asset_dir}/db.sqlite
- Legacy asset config: {asset_dir}/config.json
- Legacy executor profiles: {asset_dir}/profiles.json

Generated outputs (run these first):
- `vk migrate export-db-projects-yaml --install` -> {config_dir}/projects.migrated.<timestamp>.yaml
- `vk migrate export-asset-config-yaml --install` -> {config_dir}/config.migrated.<timestamp>.yaml + {config_dir}/secret.env.migrated.<timestamp>

Merge targets:
- {config_dir}/projects.yaml (or {config_dir}/projects.d/*.yaml)
- {config_dir}/config.yaml
- {config_dir}/secret.env

Safety:
- Do NOT commit secrets (secret.env / secret.env.migrated.*).
- Keep secrets in secret.env and reference via {{secret.NAME}} in YAML.

After merging:
1) Generate schemas: `vk config schema upsert`
2) Apply config: `curl -s -X POST http://localhost:<BACKEND_PORT>/api/config/reload`
3) Verify `/api/config/status` and key flows (projects, task create, attempts).
"#,
        asset_dir = asset_dir.display(),
        config_dir = config_dir.display(),
    );
    Ok(())
}
