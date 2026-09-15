//! The dependency-injection container.
//!
//! Providers are resolved lazily and cached (Singleton scope) in a
//! type-indexed map, mirroring Nest's behavior — except the graph was
//! validated at compile time.

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::registry::{lookup_provider, ProviderEntry};
use crate::scope::Scope;

/// Type-erased, shareable provider instance.
pub type AnyArc = Arc<dyn Any + Send + Sync>;

/// The DI container.
#[derive(Default, Clone)]
pub struct Container {
    singletons: Arc<Mutex<HashMap<TypeId, AnyArc>>>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            singletons: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Resolve a provider of type `T` (Raw-value ergonomics: returns `Arc<T>`).
    pub fn get<T: Send + Sync + 'static>(&self) -> Arc<T> {
        self.get_any(TypeId::of::<T>())
            .downcast::<T>()
            .expect("provider resolved to the wrong type")
    }

    /// Pre-populate a singleton in the container *before* build-time resolution.
    /// Any subsequent call to [`Self::get`] / [`Self::get_any`] for `T` will
    /// return this exact instance instead of invoking the registered
    /// factory. Used by `ferrite-testing` to inject mocks/overrides.
    pub fn seed_singleton<T: Send + Sync + 'static>(&self, value: Arc<T>) {
        self.singletons
            .lock()
            .unwrap()
            .insert(TypeId::of::<T>(), value as AnyArc);
    }

    /// True if `T` is present in the singletons cache (a `.seed_singleton` call,
    /// or a fully constructed singleton).
    pub fn has<T: Send + Sync + 'static>(&self) -> bool {
        self.singletons
            .lock()
            .unwrap()
            .contains_key(&TypeId::of::<T>())
    }

    /// True if the given `TypeId` is already present in the singletons cache.
    pub fn has_any(&self, ty: TypeId) -> bool {
        self.singletons.lock().unwrap().contains_key(&ty)
    }

    /// Resolve a provider by `TypeId` as a type-erased `Arc`.
    pub fn get_any(&self, ty: TypeId) -> AnyArc {
        self.try_get_any(ty).unwrap_or_else(|| {
            panic!(
                "provider {:?} is not registered — did you forget to import its module?",
                ty
            )
        })
    }

    /// Non-panicking variant of [`Self::get_any`]. Returns `None` when the
    /// provider type is not registered in the inventory or singleton cache.
    pub fn try_get_any(&self, ty: TypeId) -> Option<AnyArc> {
        if let Some(arc) = self.singletons.lock().unwrap().get(&ty) {
            return Some(arc.clone());
        }

        match lookup_provider(ty) {
            Some(entry) => {
                let mut building = HashSet::new();
                Some(self.build(&entry, &mut building))
            }
            None => None,
        }
    }

    /// Construct a provider (and, recursively, its dependencies).
    fn build(&self, entry: &ProviderEntry, building: &mut HashSet<TypeId>) -> AnyArc {
        if !building.insert(entry.ty) {
            let chain: Vec<String> = building.iter().map(|t| format!("{t:?}")).collect();
            panic!(
                "circular dependency detected: {} -> {:?}",
                chain.join(" -> "),
                entry.ty
            );
        }

        let instance = (entry.factory)(self);

        building.remove(&entry.ty);

        // Transient providers are never cached.
        if entry.scope == Scope::Transient {
            return instance;
        }

        // Cache singletons so graph cycles through the map terminate.
        let mut map = self.singletons.lock().unwrap();
        match map.get(&entry.ty) {
            Some(existing) => existing.clone(),
            None => {
                map.insert(entry.ty, instance.clone());
                instance
            }
        }
    }
}
