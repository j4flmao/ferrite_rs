use clap::Subcommand;

#[derive(Subcommand)]
pub enum OpenApiAction {
    /// Export an OpenAPI 3.1 JSON spec
    Export {
        #[arg(long, default_value = "openapi.json")]
        out: String,
    },
}

pub fn run(action: OpenApiAction) -> anyhow::Result<()> {
    match action {
        OpenApiAction::Export { out } => {
            let path = std::env::current_dir()?.join(&out);
            // Stub: users can implement `ferrite-swagger` to generate the spec.
            std::fs::write(
                &path,
                serde_json::json!({
                    "openapi": "3.1.0",
                    "info": { "title": "Ferrite API", "version": "0.1.0" },
                    "paths": {}
                })
                .to_string(),
            )?;
            println!("\x1b[32m✓\x1b[0m  exported OpenAPI spec to {out}");
            Ok(())
        }
    }
}
