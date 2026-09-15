//! Layered configuration service.
//!
//! Layers, lowest to highest priority:
//!
//! 1. Built-in defaults (`APP_ENV`, `PORT`).
//! 2. `ferrite.toml` — app/framework defaults, checked into version control.
//!    Tables flatten to `SECTION__KEY` (e.g. `[server] port = 3000` →
//!    `SERVER__PORT`), matching the typed-config `__` separator convention.
//! 3. `.env` (`.env.<FERRITE_ENV/> `--env` when present) — per-environment
//!    values, not committed.
//! 4. Real process environment — always wins (containers / CI secret
//!    injection).
//!
//! `ConfigService` registers itself as a global provider, so any
//! `#[injectable]` may depend on it without an explicit module import.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use fr_core::{Container, Injectable, ProviderEntry, Scope};

/// Ferrite's configuration service.
#[derive(Debug, Default, Clone)]
pub struct ConfigService {
    map: HashMap<String, String>,
}

impl ConfigService {
    /// Load the merged config from the current working directory.
    pub fn load() -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self::load_from(&cwd)
    }

    /// Build the merged config relative to `dir` (reads `ferrite.toml` and the
    /// `.env` files there). Real process env always wins over both.
    pub fn load_from(dir: &Path) -> Self {
        let real_env: HashMap<String, String> = std::env::vars().collect();
        Self::merge_layers(dir, &real_env)
    }

    /// Pure layering logic, factored out for tests. Priority (low→high):
    /// defaults → `ferrite.toml` → `.env` → `.env.<name>` → `real_env`.
    fn merge_layers(dir: &Path, real_env: &HashMap<String, String>) -> Self {
        let mut map = HashMap::new();
        map.insert("APP_ENV".to_string(), "development".to_string());
        map.insert("PORT".to_string(), "3000".to_string());

        // Layer 2: ferrite.toml (flattened, `SECTION__KEY`).
        if let Ok(text) = std::fs::read_to_string(dir.join("ferrite.toml")) {
            if let Ok(value) = text.parse::<toml::Value>() {
                flatten_toml("", &value, &mut map);
            }
        }

        // Layer 3a: .env
        apply_env_file(dir.join(".env"), &mut map);

        // Layer 3b: .env.<name> when FERRITE_ENV names one.
        let env_name = real_env
            .get("FERRITE_ENV")
            .or_else(|| map.get("FERRITE_ENV"))
            .cloned();
        if let Some(name) = env_name {
            if !name.is_empty() {
                apply_env_file(dir.join(format!(".env.{name}")), &mut map);
            }
        }

        // Layer 4: real process env.
        for (k, v) in real_env {
            map.insert(k.clone(), v.clone());
        }

        Self { map }
    }

    /// Get a value with a fallback.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.map
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    /// Get an optional value.
    pub fn get(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }

    /// Get a value parsed as a type, or a fallback.
    pub fn get_or_parse<T: std::str::FromStr>(&self, key: &str, default: T) -> T {
        self.map
            .get(key)
            .and_then(|v| v.parse::<T>().ok())
            .unwrap_or(default)
    }

    /// Whether a key is present.
    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// All merged key/value pairs.
    pub fn all(&self) -> &HashMap<String, String> {
        &self.map
    }
}

impl Injectable for ConfigService {
    fn __provider_entry() -> ProviderEntry {
        ProviderEntry::new_static::<ConfigService>("ConfigService", Scope::Singleton, deps, factory)
    }
}

/// Dependency list for `ConfigService` (none).
pub fn deps() -> Vec<std::any::TypeId> {
    vec![]
}

/// Construction factory for `ConfigService`.
pub fn factory(_c: &Container) -> Arc<dyn std::any::Any + Send + Sync> {
    Arc::new(ConfigService::load())
}

// Hand-rolled registration (const-friendly — no macro involved).
fr_core::__export::inventory::submit! {
    fr_core::ProviderEntry::new_static::<ConfigService>(
        "ConfigService",
        fr_core::Scope::Singleton,
        deps,
        factory,
    )
}

/// Load `.env` and `.env.<FERRITE_ENV>` from the current directory into the
/// real process environment. Keys already present in the process environ are
/// left untouched, so an externally-exported variable always wins. This exists
/// so `#[bootstrap]` entry points can make plain `std::env::var(...)` reads see
/// file-based config without pulling in a dotenv dependency.
pub fn load_env_file() {
    let cwd = std::env::current_dir().unwrap_or_default();
    load_env_file_from(&cwd);
}

/// Like [`load_env_file`], but anchored at `dir` instead of the cwd.
pub fn load_env_file_from(dir: &Path) {
    apply_env_into_process(&dir.join(".env"));
    let env_name = std::env::var("FERRITE_ENV").unwrap_or_default();
    if !env_name.is_empty() {
        apply_env_into_process(&dir.join(format!(".env.{env_name}")));
    }
}

fn apply_env_into_process(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for (key, value) in parse_env(&text) {
        if std::env::var_os(&key).is_none() {
            std::env::set_var(&key, &value);
        }
    }
}

/// Read an env-style file into `map` (highest priority key wins per line).
fn apply_env_file(path: std::path::PathBuf, map: &mut HashMap<String, String>) {
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    for (key, value) in parse_env(&text) {
        map.insert(key, value);
    }
}

/// Parse `KEY=VALUE` lines (dotenv-style). Supports comments (`#`), blank
/// lines, optional `export` prefix, single/double-quoted values, and common
/// escapes.
fn parse_env(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw_line in text.lines() {
        let mut line = raw_line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        line = line.strip_prefix("export ").unwrap_or(line);

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            // `export KEY` alone has nothing to override — skipped.
            continue;
        };

        let key = raw_key.trim().to_string();
        if key.is_empty() {
            continue;
        }

        let trimmed = raw_value.trim();
        let quoted = trimmed.len() >= 2
            && (trimmed.starts_with('"') || trimmed.starts_with('\''))
            && trimmed.ends_with(trimmed.chars().next().unwrap());

        let mut value = if quoted {
            let double = trimmed.starts_with('"');
            let inner = &trimmed[1..trimmed.len() - 1];
            if double {
                inner.replace("\\n", "\n").replace("\\t", "\t")
            } else {
                inner.to_string()
            }
        } else {
            // Strip a trailing inline comment: `PORT=3000 # note`.
            let without_comment = trimmed.split_once(" #").map(|(v, _)| v).unwrap_or(trimmed);
            without_comment.trim_end().to_string()
        };

        if value.starts_with('#') {
            value.clear();
        }
        out.push((key, value));
    }
    out
}

/// Flatten a TOML document into `SECTION__KEY` strings.
fn flatten_toml(prefix: &str, value: &toml::Value, out: &mut HashMap<String, String>) {
    match value {
        toml::Value::Table(table) => {
            for (k, v) in table {
                let key = if prefix.is_empty() {
                    k.to_uppercase()
                } else {
                    format!("{prefix}__{}", k.to_uppercase())
                };
                flatten_toml(&key, v, out);
            }
        }
        toml::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        toml::Value::Integer(i) => {
            out.insert(prefix.to_string(), i.to_string());
        }
        toml::Value::Float(f) => {
            out.insert(prefix.to_string(), f.to_string());
        }
        toml::Value::Boolean(b) => {
            out.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_env_lines() {
        let text = r#"
# comment
APP_ENV=development
QUOTED="hello world"
SINGLE='a b'
EXPORTED=value
export WITH_EXPORT=ok
EMPTY=
"#;
        let entries = parse_env(text);
        let map: HashMap<String, String> = entries.into_iter().collect();
        assert_eq!(map.get("APP_ENV").map(String::as_str), Some("development"));
        assert_eq!(map.get("QUOTED").map(String::as_str), Some("hello world"));
        assert_eq!(map.get("SINGLE").map(String::as_str), Some("a b"));
        assert_eq!(map.get("EXPORTED").map(String::as_str), Some("value"));
        assert_eq!(map.get("WITH_EXPORT").map(String::as_str), Some("ok"));
        assert_eq!(map.get("EMPTY").map(String::as_str), Some(""));
    }

    #[test]
    fn flattens_toml_sections() {
        let mut map = HashMap::new();
        let text = r#"
[app]
name = "my-api"
[server]
host = "0.0.0.0"
port = 3000
tls = true
"#;
        let value: toml::Value = text.parse().unwrap();
        flatten_toml("", &value, &mut map);
        assert_eq!(map.get("APP__NAME").map(String::as_str), Some("my-api"));
        assert_eq!(map.get("SERVER__HOST").map(String::as_str), Some("0.0.0.0"));
        assert_eq!(map.get("SERVER__PORT").map(String::as_str), Some("3000"));
        assert_eq!(map.get("SERVER__TLS").map(String::as_str), Some("true"));
    }

    #[test]
    fn layering_precedence() {
        let dir = std::env::temp_dir().join(format!("ferrite-config-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("ferrite.toml"),
            "[app]\nname = \"toml-name\"\n[server]\nport = 7000\n",
        )
        .unwrap();
        fs::write(
            dir.join(".env"),
            "APP__NAME=env-name\nFROM_ENV=yes\nPORT=8000\n",
        )
        .unwrap();
        fs::write(dir.join(".env.staging"), "FROM_STAGING=yes\nPORT=9000\n").unwrap();

        let mut real = HashMap::new();
        real.insert("PORT".to_string(), "1234".to_string());
        real.insert("FERRITE_ENV".to_string(), "staging".to_string());
        real.insert("ONLY_REAL".to_string(), "top".to_string());

        let cfg = ConfigService::merge_layers(&dir, &real);

        // toml beats defaults
        assert_eq!(cfg.get("SERVER__PORT").as_deref(), Some("7000"));
        // .env beats toml
        assert_eq!(cfg.get("APP__NAME").as_deref(), Some("env-name"));
        // real env beats .env
        assert_eq!(cfg.get("PORT").as_deref(), Some("1234"));
        // staging file applies
        assert_eq!(cfg.get("FROM_STAGING").as_deref(), Some("yes"));
        assert_eq!(cfg.get("FROM_ENV").as_deref(), Some("yes"));
        assert_eq!(cfg.get("ONLY_REAL").as_deref(), Some("top"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn accessor_helpers() {
        let mut map = HashMap::new();
        map.insert("TIMEOUT".to_string(), "42".to_string());
        let cfg = ConfigService { map };

        assert_eq!(cfg.get_or("TIMEOUT", "10"), "42");
        assert_eq!(cfg.get_or_parse::<u64>("TIMEOUT", 10), 42);
        assert_eq!(cfg.get_or_parse::<u64>("MISSING", 7), 7);
        assert!(cfg.contains("TIMEOUT"));
        assert!(!cfg.contains("OTHER"));
        assert_eq!(cfg.all().len(), 1);
    }
}
