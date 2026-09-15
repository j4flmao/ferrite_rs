//! The Ferrite CLI (`fr`, alias `frt`).

use clap::{Parser, Subcommand};

mod commands;
mod templates;

#[derive(Parser)]
#[command(
    name = "fr",
    bin_name = "fr",
    about = "Ferrite — a inspired backend framework for Rust",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new project
    New {
        /// Project name
        name: String,
        #[arg(long, default_value = "api")]
        template: String,
        #[arg(long)]
        pm: Option<String>,
        #[arg(long)]
        no_git: bool,
    },
    /// List templates available for `fr new --template`
    Templates {
        /// Print a nicely aligned ASCII gallery table
        #[arg(long, action)]
        gallery: bool,
        /// Print a compact list (default)
        #[arg(long, action)]
        list: bool,
    },
    /// Build or serve the Ferrite docs site (requires `mdbook` installed).
    Docs {
        #[command(subcommand)]
        action: commands::docs::DocsAction,
    },
    /// Generate code artifacts (alias: `f`)
    Forge {
        #[command(flatten)]
        schematic: commands::forge::Schematic,
    },
    /// Run the dev server with hot reload
    Up {
        #[arg(long)]
        port: Option<u16>,
        #[arg(long, help = "load .env.<ENV> on top of .env (e.g. staging)")]
        env: Option<String>,
        #[arg(long)]
        no_reload: bool,
    },
    /// Build the project
    Build {
        #[arg(long)]
        release: bool,
        #[arg(long)]
        target: Option<String>,
    },
    /// Run the project in the foreground
    Run,
    /// Run tests
    Test {
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        coverage: bool,
        #[arg(default_value = "unit")]
        suite: String,
    },
    /// Add an ecosystem crate
    Add { package: String },
    /// Remove an ecosystem crate
    Remove { package: String },
    /// Database lifecycle commands
    Db {
        #[command(subcommand)]
        action: commands::db::DbAction,
    },
    /// Export an OpenAPI spec
    Openapi {
        #[command(subcommand)]
        action: commands::openapi::OpenApiAction,
    },
    /// Check the environment (toolchain, DB, env vars)
    Doctor,
    /// Print project + crate info
    Info,
    /// Run `cargo clippy` with Ferrite presets
    Lint,
    /// Run `cargo fmt`
    Fmt,
    /// Scaffold ferrite.toml + .env.example
    Generate {
        #[command(subcommand)]
        what: commands::generate::GenerateCommand,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::New {
            name,
            template,
            pm,
            no_git,
        } => commands::new::run(&name, &template, pm.as_deref(), !no_git),
        Command::Templates { gallery, .. } => commands::templates::run(gallery),
        Command::Docs { action } => commands::docs::run(&action),
        Command::Forge { schematic } => commands::forge::run(schematic),
        Command::Up {
            port,
            env,
            no_reload,
        } => commands::up::run(port, env.as_deref(), no_reload),
        Command::Build { release, target } => commands::build::run(release, target.as_deref()),
        Command::Run => commands::build::run(false, None),
        Command::Test {
            watch,
            coverage,
            suite,
        } => commands::test::run(&suite, watch, coverage),
        Command::Add { package } => commands::add::run(&package, true),
        Command::Remove { package } => commands::add::run(&package, false),
        Command::Db { action } => commands::db::run(action),
        Command::Openapi { action } => commands::openapi::run(action),
        Command::Doctor => commands::doctor::run(),
        Command::Info => commands::info::run(),
        Command::Lint => commands::lint::run(),
        Command::Fmt => commands::fmt::run(),
        Command::Generate { what } => commands::generate::run(&what),
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("\x1b[31merror\x1b[0m: {err}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[allow(dead_code)]
fn print_banner() {
    let banner = r#"
  ______                _      _
 |  ____|              (_)    | |
 | |__ _ __ ___  _ __   _  ___| |_
 |  __| '__/ _ \| '_ \ | |/ _ \ __|
 | |  | | | (_) | |_) || |  __/ |_
 |_|  |_|  \___/| .__/ |_|\___|\__|
                | |
                |_|
"#;
    println!("{banner}");
}
