//! Global registries (inventory slices) for providers and modules, plus
//! lookup helpers used by the kernel during bootstrap.
//!
//! Every `#[injectable]`, `#[controller]` and `#[module]` macro submits a
//! value into one of these linker-folded collections, so `Ferrite::create`
//! can resolve the whole graph without runtime reflection.

use std::any::{Any, TypeId};
use std::sync::Arc;

use crate::container::Container;
use crate::module::{ModuleDescriptor, ModuleEntry};
use crate::scope::Scope;

inventory::collect!(ProviderEntry);
inventory::collect!(ModuleEntry);

/// A single registered provider.
#[derive(Clone)]
pub struct ProviderEntry {
    pub ty: TypeId,
    /// Readable type name used in DI diagnostics/cycle traces.
    pub name: &'static str,
    pub scope: Scope,
    pub deps: fn() -> Vec<TypeId>,
    pub factory: fn(&Container) -> Arc<dyn Any + Send + Sync>,
}

#[doc(hidden)]
pub trait Injectable: 'static {
    fn __provider_entry() -> ProviderEntry {
        panic!(
            "provider {} has no constructor — annotate one with #[inject]",
            std::any::type_name::<Self>()
        )
    }
}

impl ProviderEntry {
    /// Build a provider entry for `T` from const-compatible function pointers.
    /// The `factory` must already produce a type-erased instance (as generated
    /// by `#[inject]`), which keeps the initializer const-evaluable.
    pub const fn new_static<T: 'static>(
        name: &'static str,
        scope: Scope,
        deps: fn() -> Vec<TypeId>,
        factory: fn(&Container) -> Arc<dyn Any + Send + Sync>,
    ) -> Self {
        Self {
            ty: TypeId::of::<T>(),
            name,
            scope,
            deps,
            factory,
        }
    }
}

/// Find a module descriptor function by `TypeId`.
pub fn lookup_module(ty: TypeId) -> Option<fn() -> ModuleDescriptor> {
    find_module(ty).map(|e| e.descriptor_fn)
}

/// Find a module entry by `TypeId`.
pub fn find_module(ty: TypeId) -> Option<ModuleEntry> {
    inventory::iter::<ModuleEntry>()
        .find(|e| e.ty == ty)
        .cloned()
}

/// Find a provider entry by `TypeId`.
pub fn lookup_provider(ty: TypeId) -> Option<ProviderEntry> {
    inventory::iter::<ProviderEntry>()
        .find(|e| e.ty == ty)
        .cloned()
}

/// Collect every module entry registered in the binary.
pub fn all_modules() -> Vec<ModuleEntry> {
    inventory::iter::<ModuleEntry>().cloned().collect()
}

/// Collect every provider entry registered in the binary.
pub fn all_providers() -> Vec<ProviderEntry> {
    inventory::iter::<ProviderEntry>().cloned().collect()
}
