use std::process::Command;

use anyhow::Context;

pub fn run(suite: &str, watch: bool, coverage: bool) -> anyhow::Result<()> {
    let mut args = vec!["test".to_string()];

    if suite == "e2e" {
        args.push("--test".into());
        args.push("e2e".into());
    }

    if watch {
        let has_watch = Command::new("cargo")
            .args(["watch", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if has_watch {
            let status = Command::new("cargo")
                .args(["watch", "-w", "src", "-w", "tests", "-s", &args.join(" ")])
                .current_dir(std::env::current_dir()?)
                .status()
                .context("cargo watch failed")?;
            std::process::exit(status.code().unwrap_or(1));
        }
    }

    if coverage {
        let status = Command::new("cargo")
            .arg("llvm-cov")
            .args(&args)
            .current_dir(std::env::current_dir()?)
            .status()
            .context("cargo llvm-cov failed")?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let status = Command::new("cargo")
        .args(&args)
        .current_dir(std::env::current_dir()?)
        .status()
        .context("cargo test failed")?;
    std::process::exit(status.code().unwrap_or(1));
}
