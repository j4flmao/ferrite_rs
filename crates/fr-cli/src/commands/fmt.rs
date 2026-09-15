use std::process::Command;

use anyhow::Context;

pub fn run() -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .arg("fmt")
        .current_dir(std::env::current_dir()?)
        .status()
        .context("cargo fmt failed")?;
    std::process::exit(status.code().unwrap_or(1));
}
