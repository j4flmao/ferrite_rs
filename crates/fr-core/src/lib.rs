//! Ferrite core kernel: compile-time dependency injection, module graph,
//! and application bootstrap.

pub mod container;
pub mod graph;
pub mod hooks;
pub mod module;
pub mod registry;
pub mod scope;

pub use container::{AnyArc, Container};
pub use graph::{sort_providers, sort_providers_with, DiagReport};
pub use hooks::{OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit};
pub use module::{Module, ModuleDescriptor, ModuleEntry};
pub use registry::{
    all_modules, all_providers, find_module, lookup_module, lookup_provider, Injectable,
    ProviderEntry,
};
pub use scope::Scope;

/// The macro API surface version for Ferrite's long-term-support promise.
/// Any breaking change to the names documented under `LTS contract` in the
/// v1.0 book will bump this constant (and, per policy, bump the crate's
/// major semver component so downstream cargo refuses to update silently).
pub const FERRITE_LTS_MACRO_SURFACE_VERSION: u32 = 1;

/// Crate-internal re-exports for generated code.
#[doc(hidden)]
pub mod __export {
    pub use inventory;
    pub use tokio;
}
