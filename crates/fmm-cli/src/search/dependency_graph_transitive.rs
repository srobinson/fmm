use std::collections::{BTreeSet, HashSet, VecDeque};

use crate::manifest::{FileEntry, Manifest, builtin_source_extensions, try_resolve_local_dep};

/// Transitive dependency traversal with BFS and cycle detection.
///
/// Returns `(upstream, external, downstream)`:
/// - `upstream`: local dep file paths discovered by traversal, each annotated with
///   the hop depth at which it was first reached
/// - `external`: unresolvable dep strings (packages, etc.), deduplicated and sorted
/// - `downstream`: files that transitively depend on `file`, depth-annotated
///
/// `depth=1` gives the same results as `dependency_graph()` but with depth annotations.
/// `depth=N` traverses N hops. `depth=-1` computes the full transitive closure.
/// Cycle detection via `HashSet<String>`: already-visited files are never re-queued.
#[allow(clippy::type_complexity)]
pub fn dependency_graph_transitive(
    manifest: &Manifest,
    file: &str,
    entry: &FileEntry,
    depth: i32,
) -> (Vec<(String, i32)>, Vec<String>, Vec<(String, i32)>) {
    // -------------------------------------------------------------------------
    // Upstream BFS
    // -------------------------------------------------------------------------
    let mut upstream: Vec<(String, i32)> = Vec::new();
    let mut visited_up: HashSet<String> = HashSet::new();
    visited_up.insert(file.to_string());
    let mut external_set: BTreeSet<String> = BTreeSet::new();

    let exts = builtin_source_extensions();
    let mut queue_up: VecDeque<(String, i32)> = VecDeque::new();
    for dep in &entry.dependencies {
        if let Some(resolved) = try_resolve_local_dep(dep, file, manifest, exts) {
            if !visited_up.contains(&resolved) {
                queue_up.push_back((resolved, 1));
            }
        } else {
            external_set.insert(dep.clone());
        }
    }
    for imp in &entry.imports {
        if !imp.contains('/')
            && let Some(resolved) = try_resolve_local_dep(imp, file, manifest, exts)
        {
            if !visited_up.contains(&resolved) {
                queue_up.push_back((resolved, 1));
            }
            continue;
        }
        external_set.insert(imp.clone());
    }

    while let Some((current, d)) = queue_up.pop_front() {
        if visited_up.contains(&current) {
            continue;
        }
        visited_up.insert(current.clone());
        upstream.push((current.clone(), d));

        if (depth == -1 || d < depth)
            && let Some(e) = manifest.files.get(&current)
        {
            for dep in &e.dependencies {
                if let Some(resolved) = try_resolve_local_dep(dep, &current, manifest, exts) {
                    if !visited_up.contains(&resolved) {
                        queue_up.push_back((resolved, d + 1));
                    }
                } else {
                    external_set.insert(dep.clone());
                }
            }
            for imp in &e.imports {
                if !imp.contains('/')
                    && let Some(resolved) = try_resolve_local_dep(imp, &current, manifest, exts)
                {
                    if !visited_up.contains(&resolved) {
                        queue_up.push_back((resolved, d + 1));
                    }
                    continue;
                }
                external_set.insert(imp.clone());
            }
        }
    }

    upstream.sort_by(|a, b| a.0.cmp(&b.0));
    let external: Vec<String> = external_set.into_iter().collect();

    // -------------------------------------------------------------------------
    // Downstream BFS
    // -------------------------------------------------------------------------
    let mut downstream: Vec<(String, i32)> = Vec::new();
    let mut visited_down: HashSet<String> = HashSet::new();
    visited_down.insert(file.to_string());

    let mut queue_down: VecDeque<(String, i32)> = VecDeque::new();

    // Seed with files that directly depend on the start file (O(1) reverse index lookup)
    if let Some(direct) = manifest.reverse_deps.get(file) {
        for path in direct {
            if !visited_down.contains(path.as_str()) {
                queue_down.push_back((path.clone(), 1));
            }
        }
    }

    while let Some((current, d)) = queue_down.pop_front() {
        if visited_down.contains(&current) {
            continue;
        }
        visited_down.insert(current.clone());
        downstream.push((current.clone(), d));

        if depth == -1 || d < depth {
            // Expand next hop using reverse index (O(1) per hop instead of O(N))
            if let Some(dependents) = manifest.reverse_deps.get(&current) {
                for path in dependents {
                    if !visited_down.contains(path.as_str()) {
                        queue_down.push_back((path.clone(), d + 1));
                    }
                }
            }
        }
    }

    downstream.sort_by(|a, b| a.0.cmp(&b.0));

    (upstream, external, downstream)
}
