use std::fs;
use std::path::Path;

pub fn run() -> anyhow::Result<()> {
    let cargo_path = std::env::current_dir()?.join("Cargo.toml");
    if !cargo_path.exists() {
        anyhow::bail!("Cargo.toml not found");
    }

    let content = fs::read_to_string(&cargo_path)?;
    let name = extract_field(&content, "name");
    let version = extract_field(&content, "version");
    let edition = extract_field(&content, "edition");

    println!("Project:   {name}");
    println!("Version:   {version}");
    println!("Edition:   {edition}");

    let deps = ferrite_deps_from_cargo(&content);
    if deps.is_empty() {
        println!("Ferrite:   none detected");
    } else {
        println!("Ferrite packages:");
        for (name, ver, managed) in &deps {
            match (ver.as_deref(), *managed) {
                (Some(v), _) => println!("  · {name} = {v}"),
                (None, true) => println!("  · {name} (workspace-managed)"),
                (None, false) => println!("  · {name} (inline dep)"),
            }
        }
    }

    if Path::new("apps").exists() || Path::new("libs").exists() {
        println!("Layout:    workspace (monorepo)");
        if Path::new("apps").exists() {
            if let Ok(entries) = fs::read_dir("apps") {
                let apps: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                if !apps.is_empty() {
                    println!("Apps:      {}", apps.join(", "));
                }
            }
        }
        if Path::new("libs").exists() {
            if let Ok(entries) = fs::read_dir("libs") {
                let libs: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                if !libs.is_empty() {
                    println!("Libs:      {}", libs.join(", "));
                }
            }
        }
    } else {
        println!("Layout:    single crate");
    }

    // Surface-level module breakdown (if single-crate and src/app_module.rs exists)
    let app_mod = std::env::current_dir()?.join("src/app_module.rs");
    if app_mod.exists() {
        if let Ok(src) = fs::read_to_string(&app_mod) {
            print_module_slice("Modules", &src, "imports");
            print_module_slice("Providers", &src, "providers");
            print_module_slice("Controllers", &src, "controllers");
        }
    }

    Ok(())
}

fn extract_field(toml: &str, field: &str) -> String {
    for line in toml.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix(&field.to_string()) {
            let val = val.trim().trim_start_matches('=').trim().trim_matches('"');
            return val.to_string();
        }
    }
    "unknown".to_string()
}

fn ferrite_deps_from_cargo(content: &str) -> Vec<(String, Option<String>, bool)> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
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
            let version = value
                .trim_start_matches('{')
                .split(',')
                .find_map(|chunk| {
                    let chunk = chunk.trim();
                    let q = chunk.find('"')?;
                    let r = chunk[q + 1..].find('"')?;
                    Some(chunk[q + 1..q + 1 + r].to_string())
                })
                .or_else(|| {
                    if value.starts_with('"') && value.len() >= 2 {
                        let rest = &value[1..];
                        let end = rest.find('"')?;
                        Some(rest[..end].to_string())
                    } else {
                        None
                    }
                });
            if version.is_some() {
                out.push((name, version, false));
            } else {
                out.push((name, None, false));
            }
        }
    }
    out
}

fn print_module_slice(label: &str, src: &str, key: &str) {
    // Very rough parser that finds `key = [ A, B, C ]` (possibly multi-line)
    let Some(start) = src.find(&key.to_string()) else {
        return;
    };
    let opening = match src[start..].find('[') {
        Some(i) => start + i,
        None => return,
    };
    let mut depth = 0;
    let mut end = None;
    for (i, ch) in src[opening..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(opening + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = end else { return };
    let inside: String = src[opening + 1..close]
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    let items: Vec<String> = inside
        .split(',')
        .map(|s| {
            s.trim()
                .trim_matches(|c: char| c.is_whitespace())
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    if items.is_empty() {
        return;
    }
    println!("{label}:    {}", items.join(", "));
}
