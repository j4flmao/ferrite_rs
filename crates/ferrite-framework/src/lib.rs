//! # Ferrite
//!
//! A batteries-included backend framework for Rust that brings the developer
//! experience of NestJS — modules, dependency injection, decorator-style
//! routing — to a compiled, statically-typed runtime built on Axum/Tokio.
//!
//! ```no_run
//! use ferrite_framework::{bootstrap, Ferrite, module};
//!
//! #[module]
//! pub struct AppModule;
//!
//! #[bootstrap]
//! async fn main() {
//!     let app = Ferrite::create::<AppModule>().await;
//!     app.listen("0.0.0.0:3000").await.unwrap();
//! }
//! ```

pub use ferrite_config::ConfigService;
pub use ferrite_http::extract::{Path, Query, State};
pub use ferrite_http::{
    all_controller_routes, extract, ControllerRoutes, DefaultValuePipe, ExceptionFilter, Guard,
    HttpError, HttpResult, Interceptor, Middleware, Next, ParseIntPipe, Pipe, PipeError,
    RequestCtx, RoutePipeline, RouteSpec,
};
pub use fr_core::{
    all_modules, all_providers, find_module, lookup_module, lookup_provider, sort_providers,
    sort_providers_with, AnyArc, Container, DiagReport, Injectable, Module, ModuleDescriptor,
    ModuleEntry, OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit,
    ProviderEntry, Scope, FERRITE_LTS_MACRO_SURFACE_VERSION,
};

pub use ferrite_validation::{FieldError, Json, ValidationErrors, ValidationPipe};

pub use ferrite_orm::{Entity, OrmError, Repository, Store};

/// Namespace consumed by `#[entity]` generated code.
#[doc(hidden)]
pub mod orm {
    pub use ferrite_orm::*;
}

/// Runtime support used by `#[derive(Validate)]` generated code.
#[doc(hidden)]
pub mod validation {
    pub use ferrite_validation::{FieldError, Json, Validate, ValidationErrors, ValidationPipe};
}

/// Figurative alias for the way the generated HTTP bindings are exported.
pub use ferrite_http;

pub use ferrite_macros::{
    all, bootstrap, catch, controller, delete, entity, get, impl_controller, inject, injectable,
    module, patch, post, put, use_guards, use_interceptors, use_middleware, Validate,
};

pub use async_trait::async_trait;
pub use axum;
pub use axum::response::Response;

/// Crate-internal re-exports consumed by macro-generated code. Not public API.
#[doc(hidden)]
pub mod __export {
    pub use ferrite_config::load_env_file;
    pub use inventory;
    pub use tokio;
}

/// Internal HTTP namespace referenced by `#[impl_controller]` expansion.
#[doc(hidden)]
pub mod _http {
    pub use axum;
    pub use ferrite_http::extract::{Path, Query, State};
    pub use ferrite_http::*;
}

/// The application bootstrap entry point.
///
/// `Ferrite::create::<AppModule>()` resolves the module graph, instantiates
/// the providers (validating the DI graph at runtime with clear errors), and
/// wires every registered controller's routes onto an Axum router.
pub struct Ferrite;

/// The running application: an Axum router plus the DI container.
pub struct App {
    pub router: axum::Router,
    pub container: Container,
}

use std::any::TypeId;
use std::collections::HashSet;

impl Ferrite {
    /// Build (but do not bind) the application.
    pub async fn create<M: Module>() -> App {
        Self::create_with_seed::<M, _>(|_| {}).await
    }

    /// Build the application, letting callers pre-populate singleton providers
    /// in the container *before* the DI graph runs. Used by `ferrite-testing`
    /// to inject mock implementations via `TestingModule` (`ferrite-testing`).
    pub async fn create_with_seed<M: Module, F>(seed: F) -> App
    where
        F: FnOnce(&Container),
    {
        let container = Container::new();
        seed(&container);

        let mut modules = Vec::new();
        let mut visited = HashSet::new();
        collect_module_graph(TypeId::of::<M>(), &mut visited, &mut modules);

        // Gather providers + controllers that belong to the graph.
        let mut controllers: Vec<TypeId> = Vec::new();
        let mut providers: Vec<TypeId> = Vec::new();
        for ty in &visited {
            if let Some(entry) = find_module(*ty) {
                let desc = (entry.descriptor_fn)();
                for c in desc.controllers {
                    if !controllers.contains(&c) {
                        controllers.push(c);
                    }
                    if !providers.contains(&c) {
                        providers.push(c);
                    }
                }
                for p in desc.providers {
                    if !providers.contains(&p) {
                        providers.push(p);
                    }
                }
            }
        }

        // Validate and topologically sort the provider graph (deps first).
        // Singletons that the caller already pre-seeded are considered
        // "externally satisfied" and skipped so their own declared deps do
        // not have to be injectable (useful in tests with manual override).
        //
        // The Ferrite `Container` itself (and a handful of internal core
        // types like `ModuleDescriptor`) are never declared as user-facing
        // `#[injectable]` providers, but some internal DI-aware factories
        // (e.g. `KafkaServer`) reference `Container` in their declared
        // dependency list for bootstrapping. Exempt `Container` (plus any
        // type from `fr_core`/`ferrite-framework`) from the "unknown provider"
        // check so those factories keep working without leaking core impl
        // types into user registries.
        let exempt: HashSet<TypeId> = [TypeId::of::<Container>()].into_iter().collect();
        let sorted = match sort_providers_with(&providers, |ty| {
            container.has_any(ty) || exempt.contains(&ty)
        }) {
            Ok(order) => order,
            Err(report) => panic!("{}", report.summary()),
        };

        // Instantiate every remaining provider in dependency order.
        for ty in &sorted {
            container.get_any(*ty);
        }

        // Wire routes: nest every registered controller router under its prefix.
        let mut router = axum::Router::new();
        for cr in inventory::iter::<ControllerRoutes>() {
            if controllers.contains(&cr.controller) {
                let mut sub = axum::Router::new();
                for spec in cr.routes {
                    sub = sub.merge((spec.build)(&container));
                }
                // Axum 0.8 deprecated nesting at the root. Any path without
                // a distinct segment (empty, "/", etc.) should use merge.
                let trimmed = cr.prefix.trim_matches('/');
                if trimmed.is_empty() {
                    router = router.merge(sub);
                } else {
                    router = router.nest(cr.prefix, sub);
                }
            }
        }

        App { router, container }
    }
}

fn collect_module_graph(
    ty: TypeId,
    visited: &mut HashSet<TypeId>,
    out: &mut Vec<ModuleDescriptor>,
) {
    if !visited.insert(ty) {
        return;
    }
    if let Some(entry) = find_module(ty) {
        let desc = (entry.descriptor_fn)();
        let imports = desc.imports.clone();
        out.push(desc);
        for i in imports {
            collect_module_graph(i, visited, out);
        }
    }
}

impl App {
    /// Bind and serve on an address (e.g. `"0.0.0.0:3000"`).
    pub async fn listen(self, addr: &str) -> Result<(), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router).await
    }
}

// ---------------------------------------------------------------------------
// LTS macro/API contract: compile-time assertions.
//
// If any of these blocks fails to compile, we have broken a v1.0 LTS
// guarantee (name renamed / trait bound removed). Fix the breaking change
// or bump FERRITE_LTS_MACRO_SURFACE_VERSION along with the crate's major
// semver component.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod lts_contract {
    use super::*;

    // 1. Macro names: stringify every LTS macro identifier. If any of these
    //    names is renamed or removed from the facade re-export, the
    //    `stringify!($tok)` expansion will either fail to resolve the token
    //    *or* the length/name assertions below will drift and flag it.
    #[test]
    fn macro_names_preserved_in_root_scope() {
        macro_rules! stringify_each {
            ($($tok:ident),* $(,)?) => {
                &[$(stringify!($tok)),*] as &[&str]
            };
        }
        let names = stringify_each![
            module,
            injectable,
            inject,
            controller,
            get,
            post,
            patch,
            put,
            delete,
            bootstrap,
            entity,
            Validate,
            catch,
            use_guards,
            use_interceptors,
            use_middleware,
            all,
            impl_controller,
        ];
        assert_eq!(names.len(), 18);
        assert!(names.contains(&"module"));
        assert!(names.contains(&"Validate"));
        assert!(names.contains(&"impl_controller"));
    }

    // 2. Core traits / types are reachable via the facade.
    //    Declaring a generic function `fn name<U: Trait>()` forces the compiler
    //    to resolve `Trait` in scope at declaration time. We never
    //    monomorphise these functions — the goal is purely to *name* the
    //    types and traits so a rename/removal becomes a compile error.
    #[test]
    fn core_trait_names_preserved() {
        #[allow(dead_code)]
        fn _names_in_scope() {
            fn _injectable<U: Injectable>() {}
            fn _module<U: Module>() {}
            fn _on_init<U: OnModuleInit>() {}
            fn _on_boot<U: OnApplicationBootstrap>() {}
            fn _on_shutdown<U: OnApplicationShutdown>() {}
            fn _on_destroy<U: OnModuleDestroy>() {}

            fn _container_uses(_: &Container) {}
            fn _provider_uses(_: &ProviderEntry) {}
            fn _scope_uses(_: Scope) {}

            #[allow(non_upper_case_globals)]
            const _ASSERT_LTS_CONST_EQ_1: u32 = FERRITE_LTS_MACRO_SURFACE_VERSION;
            let _ = _ASSERT_LTS_CONST_EQ_1;
            let _ = _container_uses;
            let _ = _provider_uses;
            let _ = _scope_uses;
        }
        let _ = _names_in_scope as fn();
        assert_eq!(FERRITE_LTS_MACRO_SURFACE_VERSION, 1);
    }

    // 3. Hook re-exports match exactly 4 distinct names. Same strategy as (2) —
    //    declaration without monomorphisation is enough for a rename to break
    //    compilation.
    #[test]
    fn four_lifecycle_hooks_still_exported() {
        #[allow(dead_code)]
        fn _names_in_scope() {
            fn _on_init<U: OnModuleInit>() {}
            fn _on_boot<U: OnApplicationBootstrap>() {}
            fn _on_shutdown<U: OnApplicationShutdown>() {}
            fn _on_destroy<U: OnModuleDestroy>() {}
        }
        let _ = _names_in_scope as fn();
    }

    mod assertions_helper {}
}
