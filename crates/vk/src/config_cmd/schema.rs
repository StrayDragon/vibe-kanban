use anyhow::Result;

fn print_help() {
    println!(
        r#"vk config schema

Usage:
  vk config schema upsert
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
        "upsert" => run_upsert(),
        other => anyhow::bail!(
            "Unknown vk config schema command: {other}. Run `vk config schema --help`."
        ),
    }
}

fn run_upsert() -> Result<()> {
    let config_schema_path = utils_core::vk_config_schema_path();
    let projects_schema_path = utils_core::vk_projects_schema_path();

    config::write_config_schema_json(&config_schema_path)?;
    config::write_projects_schema_json(&projects_schema_path)?;

    println!(
        "Wrote schemas:\n- {}\n- {}",
        config_schema_path.display(),
        projects_schema_path.display()
    );
    Ok(())
}
