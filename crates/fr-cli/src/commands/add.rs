use std::fs;

const ECOSYSTEM_VERSION: &str = "=0.1.0";

pub fn run(package: &str, add: bool) -> anyhow::Result<()> {
    let cargo_path = std::env::current_dir()?.join("Cargo.toml");
    if !cargo_path.exists() {
        anyhow::bail!("Cargo.toml not found — run this from a Ferrite project root");
    }

    let mut toml = fs::read_to_string(&cargo_path)?;

    let crate_name = match package {
        "auth-jwt" => "ferrite-auth-jwt",
        "auth-oauth" => "ferrite-auth-oauth",
        "orm-sea" => "ferrite-orm-sea",
        "orm-diesel" => "ferrite-orm-diesel",
        "orm-sqlx" => "ferrite-orm-sqlx",
        "swagger" => "ferrite-swagger",
        "validation" => "ferrite-validation",
        "cache-redis" => "ferrite-cache-redis",
        "queue" => "ferrite-queue",
        "cron" => "ferrite-cron",
        "ws" => "ferrite-ws",
        "grpc" => "ferrite-grpc",
        "nats" => "ferrite-nats",
        "kafka" => "ferrite-kafka",
        "i18n" => "ferrite-i18n",
        "throttler" => "ferrite-throttler",
        "health" => "ferrite-health",
        "cqrs" => "ferrite-cqrs",
        "testing" => "ferrite-testing",
        _ => {
            eprintln!("warning: unknown short name `{package}` — adding as-is");
            package
        }
    };

    if add {
        let line = format!("{crate_name} = \"{ECOSYSTEM_VERSION}\"\n");
        if toml.contains(&format!("{crate_name} =")) {
            println!("`{crate_name}` already in Cargo.toml — skipping");
        } else {
            toml.push_str(&line);
            fs::write(&cargo_path, &toml)?;
            println!("\x1b[32m✓\x1b[0m  added `{crate_name}` to [dependencies] (pinned {ECOSYSTEM_VERSION})");
        }
    } else {
        let prefix = format!("{crate_name} =");
        if toml.contains(&prefix) {
            let mut out = String::with_capacity(toml.len());
            for line in toml.lines() {
                if line.trim_start().starts_with(&prefix) {
                    continue;
                }
                out.push_str(line);
                out.push('\n');
            }
            fs::write(&cargo_path, &out)?;
            println!("\x1b[32m✓\x1b[0m  removed `{crate_name}` from [dependencies]");
        } else {
            println!("`{crate_name}` was not in Cargo.toml — nothing to remove");
        }
    }

    Ok(())
}
