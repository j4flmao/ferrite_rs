use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::templates;

pub fn run(name: &str, template: &str, pm: Option<&str>, git: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?.join(name);

    if root.exists() {
        anyhow::bail!("directory `{name}` already exists");
    }

    let files = templates::files(template, name);
    for (path, content) in &files {
        let file_path = root.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, content).with_context(|| format!("failed to write {path}"))?;
    }

    if git {
        let git_dir = root.join(".gitignore");
        let gi = fs::read_to_string(&git_dir).unwrap_or_default();
        fs::write(&git_dir, format!("{gi}\n"))?;
        run_git(&root, &["init"])?;
    }

    if let Some(pm) = pm {
        let pkg_toml = root.join("Cargo.toml");
        let mut toml = fs::read_to_string(&pkg_toml)?;
        let extra = match pm {
            "sqlx" => "sqlx = { version = \"0.8\", features = [\"runtime-tokio-rustls\", \"postgres\"] }\n",
            "sea-orm" => "sea-orm = { version = \"1\", features = [\"sqlx-postgres\", \"runtime-tokio-rustls\"] }\n",
            "diesel" => "diesel = { version = \"2\", features = [\"postgres\", \"r2d2\"] }\n",
            _ => "",
        };
        if !extra.is_empty() {
            toml = toml.trim_end_matches('\n').to_string();
            toml.push('\n');
            toml.push_str(extra);
            fs::write(&pkg_toml, toml)?;
        }

        // Scaffold migrations dir.
        let migrations_dir = root.join("migrations");
        fs::create_dir_all(&migrations_dir).with_context(|| "failed to create migrations/")?;
    }

    println!("\x1b[32m✓\x1b[0m  Project `{name}` created with template `{template}`");
    println!("\n    cd {name} && fr up");

    Ok(())
}

fn run_git(cwd: &Path, args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()?;
    if !status.success() {
        anyhow::bail!("git {}", args.join(" "));
    }
    Ok(())
}

fn _root_exists(root: &Path) -> bool {
    root.exists()
}
