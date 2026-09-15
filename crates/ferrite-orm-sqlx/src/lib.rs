//! SQLx-backed storage for `ferrite-orm`.
//!
//! Choose one (or more) of the Cargo features below to compile in the
//! database backends you need (each enables the matching upstream
//! `sqlx` driver + `runtime-tokio-rustls` runtime):
//!
//! | Feature    | URL scheme             |
//! |------------|------------------------|
//! | `sqlite`   | `sqlite:` / `sqlite://` |
//! | `postgres` | `postgres://`          |
//! | `mysql`    | `mysql://`             |
//!
//! Backends compose freely. At DI time [`SqlxPick`] parses `DATABASE_URL`
//! and validates scheme support before opening a connection.
//!
//! Registered as a global DI provider:
//!
//! ```text
//! #[entity(table = "users", store = "ferrite_orm_sqlx::SqlxPick")]
//! ```
//!
//! Without any database features enabled, this crate is empty — the
//! default in-memory picker (see [`ferrite_orm`]) is used instead.
//!
//! Policy note (11-ROADMAP.md Non-Goals §L51): Ferrite does not ship a
//! custom ORM engine. This crate is a thin adapter over the upstream
//! `sqlx` crate, mirroring the architecture of `ferrite-orm-sea`.

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
mod sqlx;

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub use sqlx::{SqlxPick, SqlxStore};
