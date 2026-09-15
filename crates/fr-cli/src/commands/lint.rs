use std::process::Command;

use anyhow::Context;

pub fn run() -> anyhow::Result<()> {
    println!("\x1b[36mlint:\x1b[0m running cargo clippy ...\n");

    let status = Command::new("cargo")
        .args(["clippy", "--all-features", "--", "-D", "warnings"])
        .current_dir(std::env::current_dir()?)
        .status()
        .context("cargo clippy failed")?;

    if status.success() {
        println!("\n\x1b[32m✓\x1b[0m  clippy passed — no warnings");
    }

    std::process::exit(status.code().unwrap_or(1));
}
