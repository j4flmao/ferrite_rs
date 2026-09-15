//! SQLx store backend.

use std::any::TypeId;
use std::sync::Arc;

use ferrite_orm::db::DatabaseKind;
use ferrite_orm::OrmError;
use fr_core::__export::inventory;
use fr_core::{AnyArc, Container, Injectable, ProviderEntry, Scope};

fn default_database_url() -> &'static str {
    #[cfg(feature = "sqlite")]
    {
        "sqlite::memory:"
    }
    #[cfg(all(not(feature = "sqlite"), any(feature = "postgres", feature = "mysql")))]
    {
        ""
    }
    #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
    {
        ""
    }
}

fn validate_kind_enabled(kind: DatabaseKind) -> Result<(), OrmError> {
    let enabled = match kind {
        DatabaseKind::Sqlite => cfg!(feature = "sqlite"),
        DatabaseKind::Postgres => cfg!(feature = "postgres"),
        DatabaseKind::MySql => cfg!(feature = "mysql"),
    };
    if enabled {
        Ok(())
    } else {
        Err(OrmError::Backend(format!(
            "DATABASE_URL scheme {:?} requires `ferrite-orm-sqlx` feature \"{}\" to be enabled. Re-run `cargo add ferrite-orm-sqlx --features {}` or add the feature to your Cargo.toml.",
            kind,
            kind.feature_name(),
            kind.feature_name(),
        )))
    }
}

pub struct SqlxPick {
    #[allow(dead_code)]
    pub(crate) url: String,
}

impl SqlxPick {
    #[doc(hidden)]
    pub fn deps() -> Vec<TypeId> {
        vec![TypeId::of::<fr_config::ConfigService>()]
    }

    #[doc(hidden)]
    pub fn factory(c: &Container) -> AnyArc {
        let config: Arc<fr_config::ConfigService> = c.get();
        let url = config.get_or("DATABASE_URL", default_database_url());
        match DatabaseKind::from_url(&url).and_then(validate_kind_enabled) {
            Ok(()) => Arc::new(SqlxPick { url }) as AnyArc,
            Err(e) => panic!(
                "ferrite-orm-sqlx: invalid DATABASE_URL `{url}`: {e}. \
                Enable the matching Cargo feature for `ferrite-orm-sqlx` (sqlite/postgres/mysql)."
            ),
        }
    }
}

impl Injectable for SqlxPick {
    fn __provider_entry() -> ProviderEntry {
        ProviderEntry::new_static::<SqlxPick>(
            "SqlxPick",
            Scope::Singleton,
            SqlxPick::deps,
            SqlxPick::factory,
        )
    }
}

inventory::submit! {
    ProviderEntry::new_static::<SqlxPick>(
        "SqlxPick",
        Scope::Singleton,
        SqlxPick::deps,
        SqlxPick::factory,
    )
}

pub struct SqlxStore<E> {
    #[allow(dead_code)]
    pub(crate) url: String,
    pub(crate) _marker: std::marker::PhantomData<fn() -> E>,
}

impl<E> SqlxStore<E> {
    pub fn new(url: String) -> Self {
        Self {
            url,
            _marker: std::marker::PhantomData,
        }
    }
}
