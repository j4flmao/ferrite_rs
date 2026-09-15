use std::fs;
use std::path::Path;

use anyhow::Context;
use clap::Args;

#[derive(Args)]
pub struct Schematic {
    /// Schematic: module | controller | service | resource | dto | entity
    /// | guard | middleware | interceptor | pipe | filter | provider
    schematic: String,
    /// Artifact name
    name: String,
    #[arg(long)]
    module: Option<String>,
    #[arg(long)]
    flat: bool,
    #[arg(long)]
    no_spec: bool,
    #[arg(long)]
    dry_run: bool,
}

pub fn run(schematic: Schematic) -> anyhow::Result<()> {
    generate(
        &schematic.schematic,
        &schematic.name,
        schematic.module.as_deref(),
        schematic.flat,
        schematic.no_spec,
        schematic.dry_run,
    )
}

fn generate(
    schematic: &str,
    name: &str,
    module: Option<&str>,
    _flat: bool,
    no_spec: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    let dir = module
        .map(|m| Path::new("src").join("modules").join(snake(m)))
        .unwrap_or_else(|| Path::new("src").join("modules").join(snake(name)));
    let base = snake(name);

    let mut planned = vec![];
    let mut exports: Vec<(String, String)> = vec![];
    let n = ident(name);

    match schematic {
        "module" => {
            planned.push((
                dir.join(format!("{base}_module.rs")),
                module_template(name)?,
            ));
            exports.push((format!("{base}_module"), format!("{n}Module")));
        }
        "controller" => {
            planned.push((
                dir.join(format!("{base}_controller.rs")),
                controller_template(name)?,
            ));
            exports.push((format!("{base}_controller"), format!("{n}Controller")));
        }
        "service" => {
            planned.push((
                dir.join(format!("{base}_service.rs")),
                service_template(name)?,
            ));
            exports.push((format!("{base}_service"), format!("{n}Service")));
        }
        "resource" => {
            planned.push((dir.join("models.rs"), resource_models_template(name)?));
            planned.push((dir.join("dto.rs"), resource_dto_template(name)?));
            planned.push((
                dir.join(format!("{base}_service.rs")),
                resource_service_template(name)?,
            ));
            planned.push((
                dir.join(format!("{base}_controller.rs")),
                resource_controller_template(name)?,
            ));
            planned.push((
                dir.join(format!("{base}_module.rs")),
                resource_module_template(name)?,
            ));
            exports.push((format!("{base}_module"), format!("{n}Module")));
            exports.push((format!("{base}_controller"), format!("{n}Controller")));
            exports.push((format!("{base}_service"), format!("{n}Service")));
            exports.push(("models".into(), singular(&n)));
            exports.push(("dto".into(), format!("Create{}Dto", singular(&n))));
            exports.push(("dto".into(), format!("Update{}Dto", singular(&n))));
        }
        "dto" => {
            planned.push((dir.join(format!("{base}_dto.rs")), dto_template(name)?));
            exports.push((format!("{base}_dto"), format!("{n}Dto")));
        }
        "entity" => {
            planned.push((
                dir.join(format!("{base}_entity.rs")),
                entity_template(name)?,
            ));
            exports.push((format!("{base}_entity"), n.to_string()));
        }
        "guard" => {
            planned.push((dir.join(format!("{base}_guard.rs")), guard_template(name)?));
            exports.push((format!("{base}_guard"), format!("{n}Guard")));
        }
        "middleware" => {
            planned.push((
                dir.join(format!("{base}_middleware.rs")),
                middleware_template(name)?,
            ));
            exports.push((format!("{base}_middleware"), format!("{n}Middleware")));
        }
        "interceptor" => {
            planned.push((
                dir.join(format!("{base}_interceptor.rs")),
                interceptor_template(name)?,
            ));
            exports.push((format!("{base}_interceptor"), format!("{n}Interceptor")));
        }
        "pipe" => {
            planned.push((dir.join(format!("{base}_pipe.rs")), pipe_template(name)?));
            exports.push((format!("{base}_pipe"), format!("{n}Pipe")));
        }
        "filter" => {
            planned.push((
                dir.join(format!("{base}_filter.rs")),
                filter_template(name)?,
            ));
            exports.push((format!("{base}_filter"), format!("{n}Filter")));
        }
        "provider" => {
            planned.push((
                dir.join(format!("{base}_provider.rs")),
                provider_template(name)?,
            ));
            exports.push((format!("{base}_provider"), format!("{n}Provider")));
        }
        other => anyhow::bail!("unknown schematic `{other}`"),
    }

    for (path, content) in &planned {
        if dry_run {
            println!(
                "  would write  {}://{}",
                display_path(path),
                content.lines().count()
            );
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content).with_context(|| format!("write {}", display_path(path)))?;
        }
    }

    if !no_spec && !dry_run && schematic == "controller" {
        let spec = dir.join(format!("{base}_controller_test.rs"));
        fs::write(&spec, "// unit tests\n")?;
    }

    if !dry_run {
        for (stem, ty) in exports {
            upsert_mod_rs(&dir, &stem, &ty)?;
        }
    }

    let verb = if dry_run {
        "would generate"
    } else {
        "generated"
    };
    let count = planned.len();
    println!("\x1b[32m✓\x1b[0m  {verb} {count} file(s) for `{name}` ({schematic})");
    if !no_spec && schematic == "controller" && !dry_run {
        println!("  \x1b[32m✓\x1b[0m  generated test spec");
    }

    Ok(())
}

fn display_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Ensure `dir/mod.rs` declares `pub mod <stem>;` and re-exports the generated
/// type, appending any lines that aren't already present so repeated forge
/// invocations accumulate cleanly.
fn upsert_mod_rs(dir: &Path, stem: &str, ty: &str) -> anyhow::Result<()> {
    use std::collections::HashSet;

    let mod_path = dir.join("mod.rs");
    let existing = fs::read_to_string(&mod_path).unwrap_or_default();
    let seen: HashSet<String> = existing.lines().map(|l| l.trim().to_string()).collect();

    let mod_line = format!("pub mod {stem};");
    let use_line = format!("pub use {stem}::{ty};");

    let mut additions = Vec::new();
    if !seen.contains(&mod_line) {
        additions.push(mod_line);
    }
    if !seen.contains(&use_line) {
        additions.push(use_line);
    }
    if additions.is_empty() {
        return Ok(());
    }

    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for line in additions {
        out.push_str(&line);
        out.push('\n');
    }
    fs::write(&mod_path, out).with_context(|| format!("write {}", display_path(&mod_path)))?;
    Ok(())
}

fn module_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::module;

#[module]
pub struct {n}Module;
"#
    ))
}

fn controller_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{controller, delete, get, impl_controller, inject, patch, post, put}};

use super::{n}Service;

#[controller("/{name}")]
pub struct {n}Controller {{
    service: {n}Service,
}}

#[impl_controller]
impl {n}Controller {{
    #[inject]
    pub fn new(service: {n}Service) -> Self {{
        Self {{ service }}
    }}

    #[get("/")]
    pub async fn root(&self) -> &'static str {{
        "hello from {n}Controller"
    }}
}}
"#
    ))
}

fn service_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{inject, injectable}};

#[injectable]
pub struct {n}Service {{
    _marker: u8,
}}

impl {n}Service {{
    #[inject]
    pub fn new() -> Self {{
        Self {{ _marker: ::std::sync::Arc::new(0) }}
    }}
}}
"#
    ))
}

fn dto_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::Validate;
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct {n}Dto {{
    #[validate(not_empty)]
    pub name: String,
}}
"#
    ))
}

fn entity_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    let table = pluralize_eto_snake(&snake(name));
    Ok(format!(
        r#"use ferrite_framework::entity;
use serde::{{Deserialize, Serialize}};

#[entity(table = "{table}")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {n} {{
    #[entity(primary_key)]
    pub id: i64,
    pub name: String,
}}
"#
    ))
}

fn guard_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{async_trait, inject, injectable, Guard, RequestCtx}};

#[injectable]
pub struct {n}Guard {{
    _marker: u8,
}}

impl {n}Guard {{
    #[inject]
    pub fn new() -> Self {{
        Self {{ _marker: ::std::sync::Arc::new(0) }}
    }}
}}

#[async_trait]
impl Guard for {n}Guard {{
    async fn can_activate(&self, _ctx: &RequestCtx) -> bool {{
        true
    }}
}}
"#
    ))
}

fn middleware_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{async_trait, inject, injectable, Middleware, Next, RequestCtx, Response}};

#[injectable]
pub struct {n}Middleware {{
    _marker: u8,
}}

impl {n}Middleware {{
    #[inject]
    pub fn new() -> Self {{
        Self {{ _marker: ::std::sync::Arc::new(0) }}
    }}
}}

#[async_trait]
impl Middleware for {n}Middleware {{
    async fn handle(&self, ctx: RequestCtx, next: Next) -> Response {{
        next.run(ctx).await
    }}
}}
"#
    ))
}

fn interceptor_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{async_trait, inject, injectable, Interceptor, Next, RequestCtx, Response}};

#[injectable]
pub struct {n}Interceptor {{
    _marker: u8,
}}

impl {n}Interceptor {{
    #[inject]
    pub fn new() -> Self {{
        Self {{ _marker: ::std::sync::Arc::new(0) }}
    }}
}}

#[async_trait]
impl Interceptor for {n}Interceptor {{
    async fn intercept(&self, ctx: RequestCtx, next: Next) -> Response {{
        next.run(ctx).await
    }}
}}
"#
    ))
}

fn pipe_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{Pipe, PipeError}};

pub struct {n}Pipe;

impl Pipe<String> for {n}Pipe {{
    type Output = String;

    fn transform(&self, value: String) -> Result<Self::Output, PipeError> {{
        Ok(value)
    }}
}}
"#
    ))
}

fn filter_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use axum::response::IntoResponse;
use ferrite_framework::{{ExceptionFilter, HttpError, Response}};

/// Converts controlled [`HttpError`]s into the response shape this app wants.
pub struct {n}Filter;

impl ExceptionFilter for {n}Filter {{
    fn catch(&self, err: HttpError) -> Response {{
        (err.status(), err.message().to_string()).into_response()
    }}
}}
"#
    ))
}

fn provider_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    Ok(format!(
        r#"use ferrite_framework::{{inject, injectable}};

#[injectable]
pub struct {n}Provider {{
    _marker: u8,
}}

impl {n}Provider {{
    #[inject]
    pub fn new() -> Self {{
        Self {{ _marker: ::std::sync::Arc::new(0) }}
    }}
}}
"#
    ))
}

fn ident(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    let mut upper = true;
    for c in name.chars() {
        if c == '-' || c == '_' || c == '.' {
            upper = true;
            continue;
        }
        if upper {
            s.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            s.push(c);
        }
    }
    s
}

fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c == '-' || c == '.' || c == ' ' {
            out.push('_');
        } else if c.is_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out.trim_matches('_').to_string()
}

fn singular(name: &str) -> String {
    // Simple singularizer: matches the common cases the codegen uses.
    let trimmed = name.trim_end_matches('s');
    if trimmed.is_empty() {
        name.to_string()
    } else {
        trimmed.to_string()
    }
}

fn pluralize_eto_snake(snake: &str) -> String {
    // Crude pluralizer for generated table names.
    if snake.ends_with('s') {
        snake.to_string()
    } else if let Some(stem) = snake.strip_suffix('y') {
        format!("{stem}ies")
    } else if snake.ends_with('x') || snake.ends_with("sh") || snake.ends_with("ch") {
        format!("{snake}es")
    } else {
        format!("{snake}s")
    }
}

fn resource_models_template(name: &str) -> anyhow::Result<String> {
    let singular_pascal = singular(&ident(name));
    let snake_singular = snake(&singular_pascal);
    let table = pluralize_eto_snake(&snake_singular);
    Ok(format!(
        r#"use ferrite_framework::entity;
use serde::{{Deserialize, Serialize}};

#[entity(table = "{table}")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {singular_pascal} {{
    #[entity(primary_key)]
    pub id: i64,
    pub name: String,
}}
"#
    ))
}

fn resource_dto_template(name: &str) -> anyhow::Result<String> {
    let singular_pascal = singular(&ident(name));
    Ok(format!(
        r#"use ferrite_framework::Validate;
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Create{singular_pascal}Dto {{
    #[validate(length(min = 2, max = 128))]
    pub name: String,
}}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Update{singular_pascal}Dto {{
    #[validate(length(min = 2, max = 128))]
    pub name: Option<String>,
}}
"#
    ))
}

fn resource_service_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    let singular_pascal = singular(&n);
    let snake_singular = snake(&singular_pascal);
    Ok(format!(
        r#"use ferrite_framework::{{inject, injectable, orm::Repository}};

use super::{{Create{singular_pascal}Dto, Update{singular_pascal}Dto, {singular_pascal}}};

#[injectable]
pub struct {n}Service {{
    repository: Repository<{singular_pascal}>,
}}

impl {n}Service {{
    #[inject]
    pub fn new(repository: Repository<{singular_pascal}>) -> Self {{
        Self {{ repository }}
    }}

    pub async fn find_all(&self) -> Result<Vec<{singular_pascal}>, super::Error> {{
        let rows = self.repository.find_all().await?;
        Ok(rows)
    }}

    pub async fn find_one(&self, id: i64) -> Result<Option<{singular_pascal}>, super::Error> {{
        let row = self.repository.find_by_pk(&id.to_string()).await?;
        Ok(row)
    }}

    pub async fn create(&self, dto: Create{singular_pascal}Dto) -> Result<{singular_pascal}, super::Error> {{
        let next_id = self
            .repository
            .find_all()
            .await?
            .into_iter()
            .map(|u| u.id)
            .max()
            .unwrap_or(0)
            + 1;
        let record = {singular_pascal} {{
            id: next_id,
            name: dto.name,
        }};
        self.repository.save(&record).await?;
        Ok(record)
    }}

    pub async fn update(&self, id: i64, dto: Update{singular_pascal}Dto) -> Result<{singular_pascal}, super::Error> {{
        let Some(mut existing) = self.repository.find_by_pk(&id.to_string()).await? else {{
            return Err(super::Error::NotFound("no such {snake_singular}".into()));
        }};
        if let Some(name) = dto.name {{
            existing.name = name;
        }}
        self.repository.save(&existing).await?;
        Ok(existing)
    }}

    pub async fn remove(&self, id: i64) -> Result<bool, super::Error> {{
        let ok = self.repository.delete(&id.to_string()).await?;
        Ok(ok)
    }}
}}
"#
    ))
}

fn resource_controller_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    let singular_pascal = singular(&n);
    Ok(format!(
        r#"use ferrite_framework::{{
    controller, delete, get, impl_controller, inject, orm::Json, patch, post, put,
}};

use super::{{
    {n}Service, Create{singular_pascal}Dto, Update{singular_pascal}Dto,
}};

#[controller("/{name}")]
pub struct {n}Controller {{
    service: {n}Service,
}}

#[impl_controller]
impl {n}Controller {{
    #[inject]
    pub fn new(service: {n}Service) -> Self {{
        Self {{ service }}
    }}

    #[get("/")]
    pub async fn find_all(&self) -> Json<Vec<{singular_pascal}>> {{
        let rows = self.service.find_all().await.expect("find_all");
        Json(rows)
    }}

    #[get("/:id")]
    pub async fn find_one(&self, id: i64) -> Json<Option<{singular_pascal}>> {{
        let row = self.service.find_one(id).await.expect("find_one");
        Json(row)
    }}

    #[post("/")]
    pub async fn create(&self, Json(dto): Json<Create{singular_pascal}Dto>) -> Json<{singular_pascal}> {{
        let row = self.service.create(dto).await.expect("create");
        Json(row)
    }}

    #[put("/:id")]
    pub async fn update_put(&self, id: i64, Json(dto): Json<Create{singular_pascal}Dto>) -> Json<{singular_pascal}> {{
        let row = self.service
            .update(
                id,
                Update{singular_pascal}Dto {{
                    name: Some(dto.name),
                }},
            )
            .await
            .expect("update_put");
        Json(row)
    }}

    #[patch("/:id")]
    pub async fn update_patch(&self, id: i64, Json(dto): Json<Update{singular_pascal}Dto>) -> Json<{singular_pascal}> {{
        let row = self.service.update(id, dto).await.expect("update_patch");
        Json(row)
    }}

    #[delete("/:id")]
    pub async fn remove(&self, id: i64) -> Json<bool> {{
        let ok = self.service.remove(id).await.expect("remove");
        Json(ok)
    }}
}}
"#
    ))
}

fn resource_module_template(name: &str) -> anyhow::Result<String> {
    let n = ident(name);
    let singular_pascal = singular(&n);
    Ok(format!(
        r#"use ferrite_framework::{{module, orm::Repository}};

use super::{{Create{singular_pascal}Dto, Update{singular_pascal}Dto, {n}Controller, {n}Service, {singular_pascal}}};

/// Error alias shared by the service and controller layers of this module.
#[derive(Debug)]
pub enum Error {{
    Orm(ferrite_framework::orm::OrmError),
    NotFound(String),
}}

impl From<ferrite_framework::orm::OrmError> for Error {{
    fn from(value: ferrite_framework::orm::OrmError) -> Self {{
        Self::Orm(value)
    }}
}}

#[module(
    controllers = [{n}Controller],
    providers = [{n}Service, Repository<{singular_pascal}>],
)]
pub struct {n}Module;
"#
    ))
}
