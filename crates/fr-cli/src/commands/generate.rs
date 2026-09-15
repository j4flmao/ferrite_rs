use clap::Subcommand;

#[derive(Subcommand)]
pub enum GenerateCommand {
    /// Scaffold ferrite.toml
    Config,
    /// Scaffold .env.example
    Env,
}

pub fn run(what: &GenerateCommand) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    match what {
        GenerateCommand::Config => {
            let path = cwd.join("ferrite.toml");
            if !path.exists() {
                std::fs::write(
                    &path,
                    r#"[app]
name = "app"

[server]
transport = "http"
host = "0.0.0.0"
port = 3000
"#,
                )?;
                println!("\x1b[32m✓\x1b[0m  wrote ferrite.toml");
            } else {
                println!("ferrite.toml already exists");
            }
            Ok(())
        }
        GenerateCommand::Env => {
            let path = cwd.join(".env.example");
            if !path.exists() {
                std::fs::write(
                    &path,
                    "APP_ENV=development\nPORT=3000\n\nDATABASE_URL=postgres://user:pass@localhost:5432/app\nJWT_SECRET=change-me\n",
                )?;
                println!("\x1b[32m✓\x1b[0m  wrote .env.example");
            } else {
                println!(".env.example already exists");
            }
            Ok(())
        }
    }
}
