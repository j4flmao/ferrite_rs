//! DI graph validation: reachability, topological sort, cycle detection.
//!
//! `Ferrite::create` calls [`sort_providers`] with the module-owned providers
//! before instantiating anything. The kernel walks the dependency edges
//! emitted by `#[inject]`, collects every reachable provider (including
//! globals such as `ConfigService`), runs Kahn's topological sort, and fails
//! with a readable report listing **unknown providers** and **dependency
//! cycles** with their full trace.

use std::any::TypeId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;

use crate::registry::{lookup_provider, ProviderEntry};

/// Diagnostics produced by graph validation.
#[derive(Debug)]
pub struct DiagReport {
    /// Provider `TypeId`s that are depended on but have no `ProviderEntry`.
    pub missing: Vec<TypeId>,
    /// One cycle trace per detected cycle, e.g. `A -> B -> A`.
    pub cycles: Vec<Vec<String>>,
    /// Debug dependency paths for each missing provider (BFS from roots).
    pub missing_paths: Vec<(TypeId, Vec<String>)>,
}

impl DiagReport {
    /// True when the graph is valid (has a topological order).
    pub fn is_ok(&self) -> bool {
        self.missing.is_empty() && self.cycles.is_empty()
    }

    /// A human-readable summary of everything that is wrong.
    pub fn summary(&self) -> String {
        use crate::registry::all_providers;
        let mut out = String::from("invalid DI graph:\n");
        for (idx, ty) in self.missing.iter().enumerate() {
            let paths = self
                .missing_paths
                .iter()
                .find(|(t, _)| t == ty)
                .map(|(_, p)| p.clone())
                .unwrap_or_default();
            let name = paths
                .last()
                .map(|s| s.trim_start_matches('[').trim_end_matches(']').to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let _ = writeln!(out, "  unknown provider #{idx}: {ty:?} name={name}");
            if !paths.is_empty() {
                let _ = writeln!(out, "    dep chain: {}", paths.join(" <- "));
            }
            let reg_names: Vec<&str> = all_providers().iter().map(|p| p.name).collect();
            let _ = writeln!(out, "    registered providers: {reg_names:?}");
        }
        for trace in &self.cycles {
            let _ = writeln!(out, "  circular dependency: {}", trace.join(" -> "));
        }
        out
    }
}

/// Build the reachable provider graph from `roots`, topologically sorting it
/// (dependencies first). Returns the sorted provider order on success, or a
/// [`DiagReport`] describing unknown providers / dependency cycles.
pub fn sort_providers(roots: &[TypeId]) -> Result<Vec<TypeId>, DiagReport> {
    sort_providers_with(roots, |_| false)
}

fn lookup_dep_path(roots: &[TypeId], target: TypeId) -> Vec<String> {
    use std::collections::{HashMap, HashSet, VecDeque};
    let mut parent: HashMap<TypeId, (TypeId, String)> = HashMap::new();
    let mut visited: HashSet<TypeId> = HashSet::new();
    let mut queue: VecDeque<(TypeId, String)> = VecDeque::new();
    for r in roots {
        let nm = lookup_provider(*r)
            .map(|p| p.name.to_string())
            .unwrap_or(format!("{r:?}"));
        queue.push_back((*r, nm));
        visited.insert(*r);
    }
    while let Some((ty, path)) = queue.pop_front() {
        if ty == target {
            let mut out = vec![path];
            let mut cur = ty;
            while let Some((p, pn)) = parent.get(&cur).cloned() {
                out.push(pn);
                cur = p;
                if cur == target {
                    break;
                }
            }
            out.reverse();
            return out;
        }
        if let Some(entry) = lookup_provider(ty) {
            let pname = entry.name;
            for dep in (entry.deps)() {
                let dname = lookup_provider(dep)
                    .map(|p| p.name.to_string())
                    .unwrap_or(format!("{dep:?}"));
                if visited.insert(dep) {
                    let dp = format!("{path} -> [{dname}]");
                    parent.insert(dep, (ty, pname.to_string()));
                    queue.push_back((dep, dp));
                }
            }
        }
    }
    Vec::new()
}

/// Same as [`sort_providers`], but allows callers to declare some providers
/// as "already satisfied" (typically because the DI container had its
/// singletons map pre-seeded). These nodes are excluded from the topological
/// sort, and any edges departing from them are ignored (dangling edges to
/// ignored nodes still count as satisfied).
pub fn sort_providers_with<F>(roots: &[TypeId], mut ignored: F) -> Result<Vec<TypeId>, DiagReport>
where
    F: FnMut(TypeId) -> bool,
{
    // Collect — but if a node is ignored, treat it and its outbound deps as
    // "already known valid" and don't follow the chain further.
    let mut entries: HashMap<TypeId, ProviderEntry> = HashMap::new();
    let mut missing: Vec<TypeId> = Vec::new();

    let mut stack: Vec<TypeId> = roots.to_vec();
    while let Some(ty) = stack.pop() {
        if entries.contains_key(&ty) || missing.contains(&ty) {
            continue;
        }
        if ignored(ty) {
            continue;
        }
        match lookup_provider(ty) {
            Some(entry) => {
                for dep in (entry.deps)() {
                    if !ignored(dep) {
                        stack.push(dep);
                    }
                }
                entries.insert(ty, entry);
            }
            None => missing.push(ty),
        }
    }

    if !missing.is_empty() {
        let with_paths: Vec<(TypeId, Vec<String>)> = missing
            .iter()
            .map(|m| (*m, lookup_dep_path(roots, *m)))
            .collect();
        return Err(DiagReport {
            missing,
            cycles: Vec::new(),
            missing_paths: with_paths,
        });
    }

    // Kahn's algorithm where an edge `dep -> node` means the user of `dep`
    // must come second, so providers are ordered dependencies-first.
    let mut dependents: HashMap<TypeId, Vec<TypeId>> = HashMap::new();
    let mut indegree: HashMap<TypeId, usize> = entries.keys().map(|ty| (*ty, 0)).collect();
    for (ty, entry) in &entries {
        for dep in (entry.deps)() {
            if entries.contains_key(&dep) {
                dependents.entry(dep).or_default().push(*ty);
                *indegree.get_mut(ty).unwrap() += 1;
            }
            // dep either is in entries (already accounted for) or was
            // explicitly ignored (considered satisfied) — either case is OK.
        }
    }

    let mut ready: VecDeque<TypeId> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(ty, _)| *ty)
        .collect();

    ready
        .make_contiguous()
        .sort_by_key(|ty| roots.iter().position(|r| r == ty).unwrap_or(usize::MAX));

    let mut order: Vec<TypeId> = Vec::with_capacity(entries.len());
    while let Some(node) = ready.pop_front() {
        order.push(node);
        if let Some(deps) = dependents.get(&node) {
            for dependent in deps {
                let d = indegree.get_mut(dependent).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.push_back(*dependent);
                }
            }
        }
    }

    if order.len() == entries.len() {
        return Ok(order);
    }

    let unresolved: HashSet<TypeId> = entries
        .keys()
        .filter(|ty| !order.contains(ty))
        .copied()
        .collect();

    let names: HashMap<TypeId, String> = entries
        .iter()
        .map(|(ty, e)| (*ty, e.name.to_string()))
        .collect();

    let mut cycles = Vec::new();
    let mut covered: HashSet<TypeId> = HashSet::new();
    for start in &unresolved {
        if covered.contains(start) {
            continue;
        }
        let (trace, in_trace) = trace_cycle(*start, &entries, &unresolved, &names);
        covered.extend(in_trace);
        cycles.push(trace);
    }

    Err(DiagReport {
        missing: Vec::new(),
        cycles,
        missing_paths: Vec::new(),
    })
}

/// Backwards-compat wrapper — used by `ferrite-framework` facade for tests that supply
/// ignored singletons.
pub fn sort_providers_with_ignored<F>(
    roots: &[TypeId],
    ignored: F,
) -> Result<Vec<TypeId>, DiagReport>
where
    F: FnMut(TypeId) -> bool,
{
    sort_providers_with(roots, ignored)
}

#[allow(dead_code)]
fn collect_providers(roots: &[TypeId]) -> Result<HashMap<TypeId, ProviderEntry>, DiagReport> {
    let mut entries: HashMap<TypeId, ProviderEntry> = HashMap::new();
    let mut missing: Vec<TypeId> = Vec::new();

    let mut stack: Vec<TypeId> = roots.to_vec();
    while let Some(ty) = stack.pop() {
        if entries.contains_key(&ty) || missing.contains(&ty) {
            continue;
        }
        match lookup_provider(ty) {
            Some(entry) => {
                for dep in (entry.deps)() {
                    stack.push(dep);
                }
                entries.insert(ty, entry);
            }
            None => missing.push(ty),
        }
    }

    if missing.is_empty() {
        Ok(entries)
    } else {
        Err(DiagReport {
            missing,
            cycles: Vec::new(),
            missing_paths: Vec::new(),
        })
    }
}

fn trace_cycle(
    start: TypeId,
    entries: &HashMap<TypeId, ProviderEntry>,
    unresolved: &HashSet<TypeId>,
    names: &HashMap<TypeId, String>,
) -> (Vec<String>, HashSet<TypeId>) {
    let mut visited: Vec<TypeId> = vec![start];
    let mut cur = start;
    let loop_pos = loop {
        let Some(entry) = entries.get(&cur) else {
            break visited.len();
        };
        let next = (entry.deps)()
            .into_iter()
            .find(|d| unresolved.contains(d))
            .unwrap_or(cur);
        match visited.iter().position(|t| *t == next) {
            Some(pos) => break pos,
            None => {
                visited.push(next);
                cur = next;
            }
        }
    };

    let mut cycle: Vec<String> = visited[loop_pos..]
        .iter()
        .map(|t| names.get(t).cloned().unwrap_or_else(|| format!("{t:?}")))
        .collect();
    if let Some(first) = cycle.first().cloned() {
        cycle.push(first);
    }
    let in_trace: HashSet<TypeId> = visited[loop_pos..].iter().copied().collect();
    (cycle, in_trace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ProviderEntry;
    use crate::scope::Scope;
    use crate::AnyArc;
    use crate::Container;
    use std::sync::Arc;

    struct Leaf;
    struct Mid;
    struct Top;
    struct CycleA;
    struct CycleB;
    struct Missing;

    fn no_deps() -> Vec<TypeId> {
        vec![]
    }
    fn mid_deps() -> Vec<TypeId> {
        vec![TypeId::of::<Leaf>()]
    }
    fn top_deps() -> Vec<TypeId> {
        vec![TypeId::of::<Mid>()]
    }
    fn cycle_a_deps() -> Vec<TypeId> {
        vec![TypeId::of::<CycleB>()]
    }
    fn cycle_b_deps() -> Vec<TypeId> {
        vec![TypeId::of::<CycleA>()]
    }
    fn missing_deps() -> Vec<TypeId> {
        vec![TypeId::of::<Missing>()]
    }
    fn dummy(_: &Container) -> AnyArc {
        Arc::new(())
    }

    inventory::submit! { ProviderEntry::new_static::<Leaf>("Leaf", Scope::Singleton, no_deps, dummy) }
    inventory::submit! { ProviderEntry::new_static::<Mid>("Mid", Scope::Singleton, mid_deps, dummy) }
    inventory::submit! { ProviderEntry::new_static::<Top>("Top", Scope::Singleton, top_deps, dummy) }
    inventory::submit! { ProviderEntry::new_static::<CycleA>("CycleA", Scope::Singleton, cycle_a_deps, dummy) }
    inventory::submit! { ProviderEntry::new_static::<CycleB>("CycleB", Scope::Singleton, cycle_b_deps, dummy) }
    inventory::submit! { ProviderEntry::new_static::<MissingHolder>("MissingHolder", Scope::Singleton, missing_deps, dummy) }

    struct MissingHolder;

    #[test]
    fn orders_dependencies_first() {
        let order = sort_providers(&[TypeId::of::<Top>()]).unwrap();
        let names: Vec<String> = order
            .iter()
            .map(|t| {
                match *t {
                    t if t == TypeId::of::<Leaf>() => "Leaf",
                    t if t == TypeId::of::<Mid>() => "Mid",
                    t if t == TypeId::of::<Top>() => "Top",
                    _ => "?",
                }
                .to_string()
            })
            .collect();
        assert_eq!(names, vec!["Leaf", "Mid", "Top"]);
    }

    #[test]
    fn detects_missing_provider() {
        let report = sort_providers(&[TypeId::of::<MissingHolder>()]).unwrap_err();
        assert!(report.missing.contains(&TypeId::of::<Missing>()));
        assert!(report.cycles.is_empty());
        assert!(report.summary().contains("invalid DI graph"));
    }

    #[test]
    fn detects_cycle_with_trace() {
        let report = sort_providers(&[TypeId::of::<CycleA>()]).unwrap_err();
        assert!(report.missing.is_empty());
        assert_eq!(report.cycles.len(), 1);
        let trace = &report.cycles[0];
        let ab_forward = vec![
            "CycleA".to_string(),
            "CycleB".to_string(),
            "CycleA".to_string(),
        ];
        let ab_reverse = vec![
            "CycleB".to_string(),
            "CycleA".to_string(),
            "CycleB".to_string(),
        ];
        assert!(
            *trace == ab_forward || *trace == ab_reverse,
            "unexpected trace: {trace:?}"
        );
    }
}
