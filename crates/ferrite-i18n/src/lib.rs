//! ferrite-i18n: lightweight JSON-file based localization service for Ferrite apps.
//!
//! Mirror of `@nestjs/microservices` I18nService: load JSON translation files from
//! `<I18N_DIR>/<locale>.json` (e.g. `locales/en.json`) at construction time, then
//! resolve keys with an explicit locale or the configured `I18N_DEFAULT_LOCALE`.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fr_config::ConfigService;
use fr_core::{Container, ModuleDescriptor, OnApplicationBootstrap, ProviderEntry, Scope};
use serde::Deserialize;
use thiserror::Error;

type AnyArc = Arc<dyn Any + Send + Sync>;

#[derive(Debug, Error)]
pub enum I18nError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error("missing locale `{0}`")]
    MissingLocale(String),
    #[error("missing translation key `{0}` in locale `{1}`")]
    MissingKey(String, String),
}

/// A placeholder OnApplicationBootstrap no-op marker for the i18n module. Used to keep the
/// module lifecycle consistent with other `ferrite-*` crates.
#[derive(Clone)]
pub struct I18nBootstrap;

#[async_trait]
impl OnApplicationBootstrap for I18nBootstrap {
    async fn on_application_bootstrap(&self) {}
}

/// In-memory translation catalog. Holds `locale -> key -> value` maps loaded from
/// JSON files on disk (or populated manually for tests).
#[derive(Clone, Default)]
pub struct TranslationCatalog {
    map: BTreeMap<String, BTreeMap<String, String>>,
}

impl TranslationCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a whole-locale translation map (typically used by tests or by
    /// callers that prefer in-memory registration over reading files).
    pub fn insert_locale<K: Into<String>, V: Into<String>>(
        &mut self,
        locale: &str,
        entries: impl IntoIterator<Item = (K, V)>,
    ) {
        let mut inner = BTreeMap::new();
        for (k, v) in entries {
            inner.insert(k.into(), v.into());
        }
        self.map.insert(locale.to_string(), inner);
    }

    pub fn has_locale(&self, locale: &str) -> bool {
        self.map.contains_key(locale)
    }

    pub fn lookup(&self, locale: &str, key: &str) -> Option<&str> {
        self.map
            .get(locale)
            .and_then(|m| m.get(key).map(|s| s.as_str()))
    }
}

/// Main injectable i18n service.
pub struct I18nService {
    config: Arc<ConfigService>,
    default_locale: String,
    catalog: Arc<TranslationCatalog>,
}

impl I18nService {
    pub fn new(container: Container, config: Arc<ConfigService>) -> Self {
        let default_locale = config.get_or("I18N_DEFAULT_LOCALE", "en");
        let dir = config.get_or("I18N_DIR", "locales");
        let mut catalog = TranslationCatalog::new();

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if !ft.is_file() {
                        continue;
                    }
                }
                let fname = entry.file_name().to_string_lossy().to_string();
                if !fname.ends_with(".json") {
                    continue;
                }
                let locale = fname.trim_end_matches(".json").to_string();
                let path = entry.path();
                if let Ok(bytes) = std::fs::read_to_string(&path) {
                    match serde_json::from_str::<BTreeMap<String, String>>(&bytes) {
                        Ok(map) => {
                            catalog.insert_locale(&locale, map);
                        }
                        Err(err) => {
                            eprintln!(
                                "[ferrite-i18n] skipping {:?}: invalid JSON: {err}",
                                path.display()
                            );
                        }
                    }
                }
            }
        }

        let _ = container; // retained for symmetry with other service factories.
        Self {
            config,
            default_locale,
            catalog: Arc::new(catalog),
        }
    }

    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    pub fn catalog(&self) -> &TranslationCatalog {
        &self.catalog
    }

    pub fn t<S: AsRef<str>>(&self, key: S) -> String {
        self.t_with(key, None::<&str>)
    }

    pub fn t_with<S: AsRef<str>, L: AsRef<str>>(&self, key: S, locale: Option<L>) -> String {
        let key = key.as_ref();
        let locale = locale
            .as_ref()
            .map(|l| l.as_ref())
            .unwrap_or(&self.default_locale);
        if let Some(v) = self.catalog.lookup(locale, key) {
            return v.to_string();
        }
        // Fallback to the default locale if the caller asked for a more specific one.
        if locale != self.default_locale {
            if let Some(v) = self.catalog.lookup(&self.default_locale, key) {
                return v.to_string();
            }
        }
        let _ = &self.config;
        key.to_string()
    }
}

inventory::submit! {
    ProviderEntry::new_static::<I18nService>(
        "I18nService",
        Scope::Singleton,
        || vec![TypeId::of::<Container>(), TypeId::of::<ConfigService>()],
        |container| -> AnyArc {
            let cfg = container.get::<ConfigService>();
            Arc::new(I18nService::new(container.clone(), cfg)) as AnyArc
        },
    )
}

// ---------------------------------------------------------------------------
// I18nModule
// ---------------------------------------------------------------------------

pub struct I18nModule;

impl I18nModule {
    pub fn for_root() -> I18nModuleImpl {
        I18nModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct I18nModuleImpl;

impl fr_core::Module for I18nModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: String::from("I18nModule"),
            providers: vec![TypeId::of::<I18nService>()],
            controllers: vec![],
            imports: vec![],
            exports: vec![TypeId::of::<I18nService>()],
            middleware: vec![],
            global: false,
        }
    }
}

#[async_trait]
impl OnApplicationBootstrap for I18nModuleImpl {
    async fn on_application_bootstrap(&self) {
        let _ = Duration::from_millis(0);
    }
}

/// Utility marker to keep `Deserialize` bound in scope so `serde` is
/// reachable from downstream i18n crates.
#[allow(dead_code)]
#[derive(Deserialize)]
struct _I18nBoundMarker(String);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_locale_returns_registered_translation() {
        let mut cat = TranslationCatalog::new();
        cat.insert_locale("en", [("hello", "Hello"), ("bye", "Bye")]);
        let container = Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg.clone());
        let mut svc = I18nService::new(container, cfg);
        svc.default_locale = String::from("en");
        svc.catalog = Arc::new(cat);
        assert_eq!(svc.t("hello"), "Hello");
        assert_eq!(svc.t("missing"), "missing");
    }

    #[test]
    fn override_locale_returns_variant_with_default_fallback() {
        let mut cat = TranslationCatalog::new();
        cat.insert_locale("en", [("hello", "Hello"), ("shared", "English shared")]);
        cat.insert_locale("vi", [("hello", "Xin chào")]);
        let container = Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg.clone());
        let mut svc = I18nService::new(container, cfg);
        svc.default_locale = String::from("en");
        svc.catalog = Arc::new(cat);
        assert_eq!(svc.t_with("hello", Some("vi")), "Xin chào");
        assert_eq!(svc.t_with("shared", Some("vi")), "English shared");
    }
}
