use std::process::Command;

use anyhow::Context;

pub fn run(release: bool, target: Option<&str>) -> anyhow::Result<()> {
    let mut args = vec!["build".to_string()];
    if release {
        args.push("--release".into());
    }
    if let Some(t) = target {
        args.push("--target".into());
        args.push(t.to_string());
    }

    let status = Command::new("cargo")
        .args(&args)
        .current_dir(std::env::current_dir()?)
        .status()
        .context("cargo build failed")?;
    std::process::exit(status.code().unwrap_or(1));
}
