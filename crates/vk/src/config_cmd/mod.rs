use anyhow::Result;

pub mod schema;

fn print_help() {
    println!(
        r#"vk config

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
        "schema" => schema::run(args.into_iter().skip(1).collect()).await,
        other => anyhow::bail!("Unknown vk config command: {other}. Run `vk config --help`."),
    }
}
