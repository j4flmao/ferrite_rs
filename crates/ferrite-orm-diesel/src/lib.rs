//! Diesel-backed storage for `ferrite-orm`.
//!
//! Choose one (or more) of the Cargo features below to compile in the
//! database backends you need (each enables the matching upstream
//! `diesel` driver + `diesel::r2d2` pool):
//!
//! | Feature    | URL scheme             |
//! |------------|------------------------|
//! | `sqlite`   | `sqlite:`              |
//! | `postgres` | `postgres://`          |
//! | `mysql`    | `mysql://`             |
//!
//! Backends compose freely. At DI time [`DieselPick`] parses `DATABASE_URL`
//! and validates scheme support before opening a connection.
//!
//! Registered as a global DI provider:
//!
//! ```text
//! #[entity(table = "users", store = "ferrite_orm_diesel::DieselPick")]
//! ```
//!
//! Without any database features enabled, this crate is empty — the
//! default in-memory picker (see [`ferrite_orm`]) is used instead.
//!
//! Policy note (11-ROADMAP.md Non-Goals §L51): Ferrite does not ship a
//! custom ORM engine. This crate is a thin adapter over the upstream
//! `diesel` crate, mirroring the architecture of `ferrite-orm-sea`.

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
mod diesel;

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub use diesel::{DieselPick, DieselStore};
