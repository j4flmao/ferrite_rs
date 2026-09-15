use std::fs;
use std::process::Command;

use anyhow::Context;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DbAction {
    /// Run all pending migrations (in ascending order)
    Migrate,
    /// Create a new migration directory with up.sql/down.sql stubs
    MigrateCreate {
        /// Migration name (e.g. `add_users_table`)
        name: String,
    },
    /// Roll back the single most recently-applied migration
    Revert,
    /// Print the list of applied and pending migrations
    Status,
    /// Run the seed files in `seeds/*.sql` (alphabetically)
    Seed,
    /// Open a local DB browser UI (stub)
    Studio,
}

pub fn run(action: DbAction) -> anyhow::Result<()> {
    match action {
        DbAction::Migrate => run_app_via_env("migrate"),
        DbAction::MigrateCreate { name } => {
            let ts = chrono_ts();
            let dir = format!("{ts}_{name}");
            let path = std::env::current_dir()?.join("migrations").join(&dir);
            fs::create_dir_all(&path)?;
            fs::write(path.join("up.sql"), format!("-- migration: {name}\n-- Add your `CREATE TABLE / ALTER TABLE` statements here, separated by `;`.\n\n"))?;
            fs::write(
                path.join("down.sql"),
                format!("-- revert: {name}\n-- Mirror of up.sql: drop tables / undo alterations here.\n\n"),
            )?;
            println!("\x1b[32m✓\x1b[0m  created migration `{dir}`");
            println!(
                "   \x1b[90m→ {}/up.sql\n   → {}/down.sql\x1b[0m",
                path.join("up.sql").display(),
                path.join("down.sql").display()
            );
            Ok(())
        }
        DbAction::Revert => run_app_via_env("revert"),
        DbAction::Seed => run_app_via_env("seed"),
        DbAction::Status => run_app_via_env("status"),
        DbAction::Studio => {
            println!("\x1b[33mnote:\x1b[0m `db studio` is not yet implemented");
            Ok(())
        }
    }
}

fn run_app_via_env(action: &str) -> anyhow::Result<()> {
    println!("\x1b[90m$ fr db {action}\x1b[0m");
    let status = Command::new("cargo")
        .args(["run"])
        .env("FERRITE_DB_ACTION", action)
        .current_dir(std::env::current_dir()?)
        .status()
        .with_context(|| format!("db {action} failed"))?;
    std::process::exit(status.code().unwrap_or(1));
}

fn chrono_ts() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Approximate UTC timestamp string (good enough for migration names).
    format!("{now:0>13}")
}
