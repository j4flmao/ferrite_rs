use std::process::Command;

use anyhow::Context;

pub fn run(port: Option<u16>, env: Option<&str>, no_reload: bool) -> anyhow::Result<()> {
    let port = port.unwrap_or(3000);

    if no_reload {
        return run_direct(port, env);
    }

    // Try cargo-watch; fall back to plain cargo run.
    let has_watch = Command::new("cargo")
        .args(["watch", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_watch {
        let watch_command = format!(
            "FERRITE_PORT={port} FERRITE_ENV={} cargo run --quiet",
            env.unwrap_or("")
        );
        let status = Command::new("cargo")
            .args(["watch", "-w", "src", "-s", &watch_command])
            .current_dir(std::env::current_dir()?)
            .status()
            .context("failed to start cargo-watch")?;
        std::process::exit(status.code().unwrap_or(1));
    }

    eprintln!("\x1b[33mwarn:\x1b[0m cargo-watch not found — falling back to `cargo run`");
    eprintln!("      install: cargo install cargo-watch\n");
    run_direct(port, env)
}

fn run_direct(port: u16, env: Option<&str>) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.args(["run"])
        .env("FERRITE_PORT", port.to_string())
        .current_dir(std::env::current_dir()?);
    if let Some(env) = env {
        cmd.env("FERRITE_ENV", env);
    }
    let status = cmd.status().context("cargo run failed")?;
    std::process::exit(status.code().unwrap_or(1));
}
