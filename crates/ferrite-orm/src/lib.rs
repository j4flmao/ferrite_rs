//! Ferrite's persistence abstraction: entities, `Repository<T>` DI, and
//! pluggable stores.
//!
//! # Overview
//!
//! - [`Entity`] — describes a row: its primary key and table name.
//! - [`Repository<E>`] — the type you inject into services/controllers.
//!   It is registered per-entity as a DI provider by `#[entity]`
//!   (see the `ferrite-macros` crate).
//! - [`Store<E>`] — the backend contract. A [`Picker`] builds a store for a
//!   given entity; [`InMemoryPick`] is the default (zero-dependency) backend,
//!   handy for tests and early development. Database adapters (e.g.
//!   `ferrite-orm-sea`, `ferrite-orm-diesel`, `ferrite-orm-sqlx`) provide
//!   their own picker with per-backend feature flags.
//!
//! # Multi-database support
//!
//! Ferrite's ORM layer itself is database-agnostic. Each adapter ships
//! feature flags to select which upstream drivers are compiled in:
//!
//! | Adapter             | Feature    | URL scheme            |
//! |---------------------|------------|-----------------------|
//! | `ferrite-orm-sea`   | `sqlite`   | `sqlite:` / `sqlite://` |
//! | `ferrite-orm-sea`   | `postgres` | `postgres://` / `postgresql://` |
//! | `ferrite-orm-sea`   | `mysql`    | `mysql://`            |
//! | `ferrite-orm-diesel`| `sqlite`   | `sqlite:`             |
//! | `ferrite-orm-diesel`| `postgres` | `postgres://`         |
//! | `ferrite-orm-diesel`| `mysql`    | `mysql://`            |
//! | `ferrite-orm-sqlx`  | `sqlite`   | `sqlite:` / `sqlite://` |
//! | `ferrite-orm-sqlx`  | `postgres` | `postgres://`         |
//! | `ferrite-orm-sqlx`  | `mysql`    | `mysql://`            |
//!
//! Example:
//! ```text
//! cargo add ferrite-orm-sea --features postgres
//! ```
//!
//! Use [`db::DatabaseKind`] to parse a `DATABASE_URL` string at runtime into
//! a scheme enum.
//!
//! ```no_run
//! use ferrite_orm::Entity;
//!
//! #[derive(Clone)]
//! pub struct User {
//!     pub id: i64,
//!     pub name: String,
//! }
//!
//! impl Entity for User {
//!     type PrimaryKey = i64;
//!     fn table_name() -> &'static str { "users" }
//!     fn primary_key(&self) -> i64 { self.id }
//! }
//! ```

use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;

use fr_core::{AnyArc, Container, Injectable, ProviderEntry, Scope};

/// A persisted row with a primary key and table name.
pub trait Entity: Clone + Send + Sync + 'static {
    /// The type of the primary key (e.g. `i64`, `String`, `Uuid`).
    type PrimaryKey: Clone + PartialEq + fmt::Debug + fmt::Display + Send + Sync + 'static;

    /// Table name used by database backends.
    fn table_name() -> &'static str;

    /// Extract this row's primary key.
    fn primary_key(&self) -> Self::PrimaryKey;
}

/// A CRUD error surfaced by a store backend.
#[derive(Debug, Clone, PartialEq)]
pub enum OrmError {
    /// The primary key did not match any row.
    NotFound,
    /// The key failed to convert/parse (e.g. malformed id string).
    Key(String),
    /// Backend-specific failure.
    Backend(String),
}

impl fmt::Display for OrmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrmError::NotFound => write!(f, "entity not found"),
            OrmError::Key(k) => write!(f, "invalid primary key: {k}"),
            OrmError::Backend(msg) => write!(f, "orm backend error: {msg}"),
        }
    }
}

impl std::error::Error for OrmError {}

/// The object-safe contract a storage backend implements for an entity.
///
/// Primary keys travel as `&str` (via [`Entity::PrimaryKey`]'s `Display`)
/// so the trait stays object-safe and backends stay decoupled.
#[async_trait]
pub trait Store<E: Entity>: Send + Sync + 'static {
    /// Return all rows.
    async fn find_all(&self) -> Result<Vec<E>, OrmError>;
    /// Find a single row by primary key.
    async fn find_by_pk(&self, pk: &str) -> Result<Option<E>, OrmError>;
    /// Insert or update a row.
    async fn save(&self, entity: &E) -> Result<(), OrmError>;
    /// Delete a row by primary key; returns whether a row was removed.
    async fn delete(&self, pk: &str) -> Result<bool, OrmError>;
}

/// Builds the store backend for a *specific* entity at DI time.
///
/// Backends that only support certain entities (e.g. a database adapter
/// requiring a SeaORM bridge) implement `Pick<E>` directly and put their
/// requirements in the impl's `where` clause. Entity-agnostic backends
/// implement [`Picker`] and get `Pick<E>` for free via the blanket impl.
pub trait Pick<E: Entity>: Send + Sync + 'static {
    /// Build a store for `E`.
    fn build(&self) -> Arc<dyn Store<E>>;
}

/// Builds the store backend for *any* entity at DI time.
pub trait Picker: Send + Sync + 'static {
    /// Build a store for `E`.
    fn build<E: Entity>(&self) -> Arc<dyn Store<E>>;
}

impl<P: Picker, E: Entity> Pick<E> for P {
    fn build(&self) -> Arc<dyn Store<E>> {
        Picker::build::<E>(self)
    }
}

/// A `Repository<E>` is the injectable handle for an entity's rows.
///
/// It is a plain struct (registered per-entity by `#[entity]`), so it works
/// with the DI container like any other provider: inject `Repository<User>`
/// into a service or controller.
pub struct Repository<E: Entity> {
    store: Arc<dyn Store<E>>,
    _marker: PhantomData<fn() -> E>,
}

impl<E: Entity> Repository<E> {
    /// Construct a repository backed by `store`.
    pub fn new(store: Arc<dyn Store<E>>) -> Self {
        Self {
            store,
            _marker: PhantomData,
        }
    }

    /// Return all rows.
    pub async fn find_all(&self) -> Result<Vec<E>, OrmError> {
        self.store.find_all().await
    }

    /// Find a row by primary key.
    pub async fn find_by_pk(&self, pk: E::PrimaryKey) -> Result<Option<E>, OrmError> {
        self.store.find_by_pk(&pk.to_string()).await
    }

    /// Insert or update a row.
    pub async fn save(&self, entity: &E) -> Result<(), OrmError> {
        self.store.save(entity).await
    }

    /// Delete a row by primary key; returns whether a row was removed.
    pub async fn delete(&self, pk: &E::PrimaryKey) -> Result<bool, OrmError> {
        self.store.delete(&pk.to_string()).await
    }
}

impl<E: Entity> Clone for Repository<E> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            _marker: PhantomData,
        }
    }
}

/// Default picker wired as a global provider: builds in-memory stores.
pub struct InMemoryPick;

impl Picker for InMemoryPick {
    fn build<E: Entity>(&self) -> Arc<dyn Store<E>> {
        Arc::new(memory::MemoryStore::<E>::default())
    }
}

impl Injectable for InMemoryPick {
    fn __provider_entry() -> ProviderEntry {
        ProviderEntry::new_static::<InMemoryPick>("InMemoryPick", Scope::Singleton, deps, factory)
    }
}

/// Dependency list for [`InMemoryPick`] (none).
pub fn deps() -> Vec<TypeId> {
    vec![]
}

/// Construction factory for [`InMemoryPick`].
pub fn factory(_c: &Container) -> AnyArc {
    Arc::new(InMemoryPick)
}

// Hand-rolled registration (const-friendly — no macro involved).
fr_core::__export::inventory::submit! {
    fr_core::ProviderEntry::new_static::<InMemoryPick>(
        "InMemoryPick",
        fr_core::Scope::Singleton,
        deps,
        factory,
    )
}

/// Zero-dependency in-memory stores, keyed by primary key's `Display` form.
///
/// Intended for tests and development before a real database is wired up.
pub mod memory {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{Entity, OrmError, Store};
    use async_trait::async_trait;

    /// An owned in-memory store for `E`.
    #[derive(Debug)]
    pub struct MemoryStore<E> {
        rows: Mutex<HashMap<String, E>>,
    }

    impl<E> Default for MemoryStore<E> {
        fn default() -> Self {
            Self {
                rows: Mutex::new(HashMap::new()),
            }
        }
    }

    impl<E: Entity> MemoryStore<E> {
        /// Create an empty store.
        pub fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait]
    impl<E: Entity> Store<E> for MemoryStore<E> {
        async fn find_all(&self) -> Result<Vec<E>, OrmError> {
            let rows = self.rows.lock().unwrap();
            Ok(rows.values().cloned().collect())
        }

        async fn find_by_pk(&self, pk: &str) -> Result<Option<E>, OrmError> {
            let rows = self.rows.lock().unwrap();
            Ok(rows.get(pk).cloned())
        }

        async fn save(&self, entity: &E) -> Result<(), OrmError> {
            let key = entity.primary_key().to_string();
            self.rows.lock().unwrap().insert(key, entity.clone());
            Ok(())
        }

        async fn delete(&self, pk: &str) -> Result<bool, OrmError> {
            Ok(self.rows.lock().unwrap().remove(pk).is_some())
        }
    }
}

/// Database scheme helpers shared by all adapters.
///
/// Parses the scheme component of a `DATABASE_URL` string into a known
/// backend kind. Each adapter then validates that the compiled-in feature
/// flags support the parsed kind before opening a real connection.
pub mod db {
    use super::OrmError;

    /// A supported (or unsupported) database backend.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DatabaseKind {
        Sqlite,
        Postgres,
        MySql,
    }

    impl DatabaseKind {
        /// Parse a DATABASE_URL string into a [`DatabaseKind`] by looking at
        /// its scheme prefix.
        ///
        /// Unknown schemes (e.g. custom drivers) are returned as
        /// `OrmError::Backend` so callers can surface a helpful error.
        pub fn from_url(url: &str) -> Result<Self, OrmError> {
            let s = url.trim_start();
            if s.starts_with("sqlite:") {
                Ok(DatabaseKind::Sqlite)
            } else if s.starts_with("postgres://") || s.starts_with("postgresql://") {
                Ok(DatabaseKind::Postgres)
            } else if s.starts_with("mysql://") {
                Ok(DatabaseKind::MySql)
            } else {
                let scheme = s
                    .split_once("://")
                    .map(|(l, _)| l)
                    .or_else(|| s.split_once(':').map(|(l, _)| l))
                    .unwrap_or("(none)");
                Err(OrmError::Backend(format!(
                    "unknown DATABASE_URL scheme `{scheme}` — supported schemes are sqlite:, postgres://, mysql://",
                )))
            }
        }

        /// Human-readable feature name expected in the adapter's Cargo.toml.
        pub fn feature_name(&self) -> &'static str {
            match self {
                DatabaseKind::Sqlite => "sqlite",
                DatabaseKind::Postgres => "postgres",
                DatabaseKind::MySql => "mysql",
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_core::Container;

    #[derive(Clone, Debug, PartialEq)]
    struct Dog {
        id: i64,
        name: String,
    }

    impl Entity for Dog {
        type PrimaryKey = i64;
        fn table_name() -> &'static str {
            "dogs"
        }
        fn primary_key(&self) -> i64 {
            self.id
        }
    }

    fn deps() -> Vec<TypeId> {
        vec![TypeId::of::<InMemoryPick>()]
    }

    fn factory(c: &Container) -> AnyArc {
        let pick: Arc<InMemoryPick> = c.get::<InMemoryPick>();
        let store = Pick::<Dog>::build(pick.as_ref());
        Arc::new(Repository::<Dog>::new(store)) as AnyArc
    }

    inventory::submit! {
        ProviderEntry::new_static::<Repository<Dog>>("Repository<Dog>", Scope::Singleton, deps, factory)
    }

    use std::future::Future;
    use std::pin::Pin;

    #[tokio::test]
    async fn memory_store_roundtrip() {
        let pick = InMemoryPick;
        let store = Pick::<Dog>::build(&pick);
        let repo = Repository::<Dog>::new(store);

        assert_eq!(repo.find_all().await.unwrap(), vec![]);
        repo.save(&Dog {
            id: 1,
            name: "Rex".into(),
        })
        .await
        .unwrap();
        repo.save(&Dog {
            id: 2,
            name: "Fido".into(),
        })
        .await
        .unwrap();

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 2);

        let one = repo.find_by_pk(1).await.unwrap().unwrap();
        assert_eq!(one.name, "Rex");
        assert!(repo.find_by_pk(99).await.unwrap().is_none());

        repo.save(&Dog {
            id: 1,
            name: "Rex v2".into(),
        })
        .await
        .unwrap();
        assert_eq!(repo.find_by_pk(1).await.unwrap().unwrap().name, "Rex v2");

        assert!(repo.delete(&2).await.unwrap());
        assert!(!repo.delete(&2).await.unwrap());
    }

    use fr_core::__export::inventory;

    #[tokio::test]
    async fn repository_resolves_through_di() {
        let container = Container::new();
        let repo: Arc<Repository<Dog>> = container.get::<Repository<Dog>>();
        repo.save(&Dog {
            id: 7,
            name: "Bolt".into(),
        })
        .await
        .unwrap();
        let one = repo.find_by_pk(7).await.unwrap().unwrap();
        assert_eq!(one.name, "Bolt");
        // InMemoryPick resolved as a standalone provider too.
        let _pick: Arc<InMemoryPick> = container.get::<InMemoryPick>();
    }

    #[test]
    fn store_trait_is_object_safe() {
        let store: Arc<dyn Store<Dog>> = Arc::new(memory::MemoryStore::<Dog>::new());
        let repo = Repository::<Dog>::new(store);
        let _all: Pin<Box<dyn Future<Output = Result<Vec<Dog>, OrmError>> + Send>> =
            Box::pin(repo.find_all());
        let _pk: Pin<Box<dyn Future<Output = Result<Option<Dog>, OrmError>> + Send>> =
            Box::pin(repo.find_by_pk(1));
        let dog = Dog {
            id: 1,
            name: "X".into(),
        };
        let _save: Pin<Box<dyn Future<Output = Result<(), OrmError>> + Send>> =
            Box::pin(repo.save(&dog));
        let _del: Pin<Box<dyn Future<Output = Result<bool, OrmError>> + Send>> =
            Box::pin(repo.delete(&1));
    }
}
