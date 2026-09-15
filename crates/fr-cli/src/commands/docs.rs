use std::path::PathBuf;
use std::process::Command;

use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum DocsAction {
    /// Build the documentation site once (requires `mdbook` on PATH).
    Build,
    /// Serve the documentation site locally (requires `mdbook` on PATH).
    Serve {
        /// TCP port for the mdbook dev server.
        #[arg(long, default_value_t = 3001)]
        port: u16,
    },
}

fn docs_dir() -> anyhow::Result<PathBuf> {
    let candidates = [
        std::env::current_dir()?.join("docs"),
        std::env::current_dir()?.join("..").join("docs"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("book.toml").exists())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no docs/book.toml found in the current tree. Run from a Ferrite checkout."
            )
        })
}

pub fn run(action: &DocsAction) -> anyhow::Result<()> {
    // Ensure mdbook binary exists first (friendly error instead of silent exit).
    match Command::new("mdbook").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout);
            eprintln!(
                "\x1b[32m✓\x1b[0m using {}",
                ver.lines().next().unwrap_or("mdbook")
            );
        }
        _ => {
            return Err(anyhow::anyhow!(
                "`mdbook` binary not found on PATH. Install it first:\n    cargo install mdbook"
            ));
        }
    }
    let dir = docs_dir()?;
    let status = match action {
        DocsAction::Build => Command::new("mdbook")
            .arg("build")
            .current_dir(&dir)
            .status()?,
        DocsAction::Serve { port } => Command::new("mdbook")
            .args(["serve", "--port", &port.to_string()])
            .current_dir(&dir)
            .status()?,
    };
    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(1);
        Err(anyhow::anyhow!("mdbook exited with status code {code}"))
    }
}
