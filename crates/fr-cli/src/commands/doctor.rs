use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Context;

pub fn run() -> anyhow::Result<()> {
    println!("Ferrite environment diagnostics\n");

    let ok = check_cmd("rustc", &["--version"], "Rust toolchain");
    let _ = ok;

    let _ = check_cmd("cargo", &["--version"], "Cargo");
    let _ = check_cmd("cargo", &["watch", "--version"], "cargo-watch (optional)");
    let _ = check_cmd(
        "mdbook",
        &["--version"],
        "mdbook (optional, for `fr docs build/serve`)",
    );

    let env_path = std::env::current_dir()?.join(".env");
    if env_path.exists() {
        println!("  \x1b[32m✓\x1b[0m  .env file present");
    } else {
        println!("  \x1b[33m-\x1b[0m  .env not found (using .env.example defaults)");
    }

    let ft = std::env::current_dir()?.join("ferrite.toml");
    if ft.exists() {
        println!("  \x1b[32m✓\x1b[0m  ferrite.toml present");
    } else {
        println!("  \x1b[33m-\x1b[0m  ferrite.toml not found");
    }

    if let Ok(url) = std::env::var("DATABASE_URL") {
        println!("  \x1b[32m✓\x1b[0m  DATABASE_URL is set: {url}");
    } else {
        println!("  \x1b[33m-\x1b[0m  DATABASE_URL not set");
    }

    // --- Ferrite ecosystem semver lockstep checks v0.7 (matching v1.0 roadmap policy) ---
    let cargo_path = std::env::current_dir()?.join("Cargo.toml");
    let mut mismatches: u32 = 0;
    let mut baseline: Option<String> = None;
    if cargo_path.exists() {
        println!();
        let deps = ferrite_deps_from_cargo(&fs::read_to_string(&cargo_path)?);
        baseline = baseline_ferrite_version(&deps);
        if let Some(base) = baseline.as_deref() {
            println!("  Ferrite baseline version: {base}");
            for (name, ver, managed) in &deps {
                if *managed {
                    println!("  \x1b[32m✓\x1b[0m  {name} (workspace-managed)");
                    continue;
                }
                match ver.as_deref() {
                    Some(v) if v == base => {
                        let pinned = v.starts_with('=');
                        if pinned {
                            println!("  \x1b[32m✓\x1b[0m  {name} = {v}");
                        } else {
                            println!(
                                "  \x1b[33m-\x1b[0m  {name} = {v} — \x1b[33mnot exact pin\x1b[0m (v1.0 semver policy recommends `=\"{base}\"`)"
                            );
                        }
                    }
                    Some(v) => {
                        mismatches += 1;
                        println!(
                            "  \x1b[31m✗\x1b[0m  {name} = {v} — \x1b[33mversion mismatch\x1b[0m (expected {base}, lockstep policy)"
                        );
                    }
                    None => {
                        println!("  \x1b[33m-\x1b[0m  {name} (unable to parse version)");
                    }
                }
            }
        } else {
            for (name, ver, _) in &deps {
                match ver.as_deref() {
                    Some(v) => println!("  \x1b[32m✓\x1b[0m  {name} = {v}"),
                    None => println!("  \x1b[33m-\x1b[0m  {name} (workspace or inline dep)"),
                }
            }
        }

        // Import missing diagnostics
        let app_mod = std::env::current_dir()?.join("src/app_module.rs");
        if app_mod.exists() {
            if let Ok(src) = fs::read_to_string(&app_mod) {
                check_module_import(&deps, &src, "ferrite-grpc", "GrpcModule");
                check_module_import(&deps, &src, "ferrite-nats", "NatsModule");
                check_module_import(&deps, &src, "ferrite-kafka", "KafkaModule");
                check_module_import(&deps, &src, "ferrite-i18n", "I18nModule");
                check_module_import(&deps, &src, "ferrite-cqrs", "CqrsModule");
            }
        }
    }

    let lenient = std::env::var_os("FERRITE_DOCTOR_LENIENT").is_some();
    if mismatches > 0 {
        if lenient {
            println!("\n\x1b[33m-\x1b[0m  {mismatches} version mismatches (lenient mode via FERRITE_DOCTOR_LENIENT=1; exit 0)");
            println!("\n\x1b[32m✓\x1b[0m  Done.");
            Ok(())
        } else {
            println!("\n\x1b[31m✗\x1b[0m  {mismatches} version mismatches — lockstep policy violation (exit 1)");
            println!("  Hint: pin all ferrite-* deps to `=\"{}\"` exact or run with FERRITE_DOCTOR_LENIENT=1 to ignore.",
                baseline.as_deref().unwrap_or("X.Y.Z"));
            Err(anyhow::anyhow!(
                "{mismatches} ecosystem version mismatches (strict v1.0 default; lenient via FERRITE_DOCTOR_LENIENT=1)"
            ))
        }
    } else {
        println!("\n\x1b[32m✓\x1b[0m  Done.");
        Ok(())
    }
}

fn check_cmd(cmd: &str, args: &[&str], label: &str) -> anyhow::Result<bool> {
    let out = match Command::new(cmd).args(args).output() {
        Ok(o) => o,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            println!("  \x1b[33m-\x1b[0m  {label}: not installed (optional, `cargo install {cmd}` if needed)");
            return Ok(false);
        }
        Err(err) => {
            return Err(err).with_context(|| format!("could not run `{cmd}`"));
        }
    };

    if out.status.success() {
        let version = String::from_utf8_lossy(&out.stdout);
        let version = version.trim().lines().next().unwrap_or("?");
        println!("  \x1b[32m✓\x1b[0m  {label}: {version}");
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let head = stderr.trim().lines().next().unwrap_or("?");
        println!("  \x1b[31m✗\x1b[0m  {label}: error — {head}");
        Ok(false)
    }
}

/// Parse (crate_name, version_string_or_None_if_workspace, is_workspace_managed)
/// for dependencies starting with `ferrite` (excluding random lines mentioning ferrite in comments).
fn ferrite_deps_from_cargo(content: &str) -> Vec<(String, Option<String>, bool)> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        // name = "version" form
        if let Some(idx) = trimmed.find('=') {
            let (name_part, rest) = trimmed.split_at(idx);
            let name = name_part.trim().trim_matches('"').to_string();
            if !name.starts_with("ferrite") {
                continue;
            }
            let value = rest.trim().trim_start_matches('=').trim();
            if value.contains("workspace") && value.contains("true") {
                out.push((name, None, true));
                continue;
            }
            // Extract quoted version (first "...")
            let version = value
                .trim_start_matches('{')
                .split(',')
                .find_map(|chunk| extract_first_quoted(chunk.trim()))
                .or_else(|| extract_first_quoted(value));
            if version.is_some() {
                out.push((name, version, false));
            } else {
                out.push((name, None, false));
            }
        }
    }
    out
}

fn extract_first_quoted(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let rest = &s[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn baseline_ferrite_version(deps: &[(String, Option<String>, bool)]) -> Option<String> {
    let (_, preferred, _) = deps.iter().find(|(n, _, _)| n == "ferrite-framework")?;
    preferred.clone().or_else(|| {
        deps.iter()
            .find(|(n, _, _)| n == "fr-core")
            .and_then(|(_, v, _)| v.clone())
    })
}

fn check_module_import(
    deps: &[(String, Option<String>, bool)],
    app_module_src: &str,
    dep_name: &str,
    mod_name: &str,
) {
    let found_dep = deps.iter().any(|(n, _, _)| n == dep_name);
    if !found_dep {
        return;
    }
    let has_import = app_module_src.lines().any(|l| l.contains(mod_name));
    if has_import {
        println!("  \x1b[32m✓\x1b[0m  `{mod_name}` import present (for {dep_name})");
    } else {
        println!(
            "  \x1b[33m-\x1b[0m  dependency `{dep_name}` installed but \x1b[33mmissing `{mod_name}` import\x1b[0m in src/app_module.rs"
        );
    }
    let _ = Path::new(".");
}
