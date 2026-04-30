use std::collections::{BTreeSet, HashSet, VecDeque};

use crate::manifest::{FileEntry, Manifest, builtin_source_extensions, try_resolve_local_dep};
use crate::resolver::RustImportResolver;

use super::DependencyGraphQuery;
use super::helpers::{
    reverse_deps_resolve_specifier, rust_workspace_resolver, workspace_specifier_names_for_source,
};

/// Transitive dependency traversal with BFS and cycle detection.
///
/// Returns `(upstream, external, downstream)`:
/// - `upstream`: local dep file paths discovered by traversal, each annotated with
///   the hop depth at which it was first reached
/// - `external`: unresolvable dep strings (packages, etc.), deduplicated and sorted
/// - `downstream`: files that transitively depend on `file`, depth-annotated
#[allow(clippy::type_complexity)]
pub fn dependency_graph_transitive(
    manifest: &Manifest,
    file: &str,
    entry: &FileEntry,
    depth: i32,
) -> (Vec<(String, i32)>, Vec<String>, Vec<(String, i32)>) {
    let graph_query = DependencyGraphQuery::new(manifest).ok();
    let (upstream, external) =
        transitive_upstream(manifest, graph_query.as_ref(), file, entry, depth);
    let downstream = graph_query
        .as_ref()
        .map_or_else(Vec::new, |graph| graph.transitive_downstream(file, depth));

    (upstream, external, downstream)
}

fn transitive_upstream(
    manifest: &Manifest,
    graph_query: Option<&DependencyGraphQuery<'_>>,
    file: &str,
    entry: &FileEntry,
    depth: i32,
) -> (Vec<(String, i32)>, Vec<String>) {
    let exts = builtin_source_extensions();
    let rust_resolver = rust_workspace_resolver(manifest, file);
    let mut upstream = Vec::new();
    let mut visited = HashSet::from([file.to_string()]);
    let mut external_set = BTreeSet::new();
    let mut queue = VecDeque::new();

    queue_upstream_candidates(
        UpstreamContext {
            manifest,
            graph_query,
            rust_resolver: rust_resolver.as_ref(),
            exts,
        },
        file,
        entry,
        1,
        &visited,
        &mut queue,
        &mut external_set,
    );

    while let Some((current, current_depth)) = queue.pop_front() {
        if !visited.insert(current.clone()) {
            continue;
        }
        upstream.push((current.clone(), current_depth));

        if (depth == -1 || current_depth < depth)
            && let Some(current_entry) = manifest.files.get(&current)
        {
            queue_upstream_candidates(
                UpstreamContext {
                    manifest,
                    graph_query,
                    rust_resolver: rust_resolver.as_ref(),
                    exts,
                },
                &current,
                current_entry,
                current_depth + 1,
                &visited,
                &mut queue,
                &mut external_set,
            );
        }
    }

    upstream.sort_by(|a, b| a.0.cmp(&b.0));
    (upstream, external_set.into_iter().collect())
}

struct UpstreamContext<'a> {
    manifest: &'a Manifest,
    graph_query: Option<&'a DependencyGraphQuery<'a>>,
    rust_resolver: Option<&'a RustImportResolver>,
    exts: &'a HashSet<String>,
}

fn queue_upstream_candidates(
    context: UpstreamContext<'_>,
    source_file: &str,
    entry: &FileEntry,
    next_depth: i32,
    visited: &HashSet<String>,
    queue: &mut VecDeque<(String, i32)>,
    external_set: &mut BTreeSet<String>,
) {
    let graph_upstream = context
        .graph_query
        .map_or_else(Vec::new, |graph| graph.direct_upstream(source_file));
    let workspace_specifier_names =
        workspace_specifier_names_for_source(context.manifest, context.rust_resolver, source_file);

    for dep in &entry.dependencies {
        if let Some(resolved) =
            try_resolve_local_dep(dep, source_file, context.manifest, context.exts)
        {
            push_if_unvisited(queue, visited, resolved, next_depth);
        } else if !reverse_deps_resolve_specifier(&workspace_specifier_names, &graph_upstream, dep)
        {
            external_set.insert(dep.clone());
        }
    }
    for imp in &entry.imports {
        if !imp.contains('/')
            && let Some(resolved) =
                try_resolve_local_dep(imp, source_file, context.manifest, context.exts)
        {
            push_if_unvisited(queue, visited, resolved, next_depth);
            continue;
        }
        if !reverse_deps_resolve_specifier(&workspace_specifier_names, &graph_upstream, imp) {
            external_set.insert(imp.clone());
        }
    }
    for resolved in graph_upstream {
        push_if_unvisited(queue, visited, resolved, next_depth);
    }
}

fn push_if_unvisited(
    queue: &mut VecDeque<(String, i32)>,
    visited: &HashSet<String>,
    path: String,
    depth: i32,
) {
    if !visited.contains(&path) {
        queue.push_back((path, depth));
    }
}
