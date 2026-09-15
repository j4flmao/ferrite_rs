//! SeaORM-backed storage for `ferrite-orm`.
//!
//! Choose one (or more) of the Cargo features below to compile in the database backends
//! you need (each enables the matching upstream `sea-orm` driver):
//!
//! | Feature    | URL scheme                              |
//! |------------|-----------------------------------------|
//! | `sqlite`   | `sqlite:` / `sqlite://`               |
//! | `postgres` | `postgres://` / `postgresql://`          |
//! | `mysql`    | `mysql://`                              |
//!
//! Backends compose freely (you can enable multiple features simultaneously). At DI time
//! [`SeaPick`] parses `DATABASE_URL` and validates that the compiled crate
//! has support for its scheme before attempting a real connection.
//!
//! Registered as a global DI provider, so an entity simply swaps its store:
//!
//! ```text
//! #[entity(table = "users", store = "ferrite_orm_sea::SeaPick")]
//! ```
//!
//! Without any of the database features enabled, this crate is empty — the
//! default in-memory picker (see [`ferrite_orm`]) is used instead.
//!
//! Policy note (11-ROADMAP.md Non-Goals §L51): Ferrite does not ship a
//! custom ORM engine. This crate is a thin adapter over the upstream
//! `sea-orm` crate, using its SQLx-backed drivers.

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub mod harness;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
mod sea;

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub use sea::{SeaPick, SeaStore};
