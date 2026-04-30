use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::identity::EdgeKind;
use crate::manifest::Manifest;

use super::local::try_resolve_local_dep;
use super::path::{builtin_source_extensions, is_cargo_workspace_source};
use super::workspace::collect_workspace_edges;
use super::{dep_matches, dotted_dep_matches, python_dep_matches};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyEdge {
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) kind: EdgeKind,
}

/// Build the reverse dependency index from a fully loaded manifest.
///
/// Maps each file to the set of files that directly import it. Built once at
/// manifest load time so downstream lookups are O(1) instead of O(N * D).
///
/// Three resolution paths:
/// - `entry.dependencies`: relative imports resolved via `try_resolve_local_dep` /
///   `dep_matches` / `python_dep_matches`
/// - `entry.imports` in Python dotted style: resolved via `dotted_dep_matches`
/// - workspace imports: resolved by language dispatch. JS/TS uses
///   `CrossPackageResolver`; Rust uses `RustImportResolver`; Go uses
///   `GoImportResolver`.
pub(crate) fn build_reverse_deps(manifest: &Manifest) -> HashMap<String, Vec<String>> {
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();

    for edge in build_dependency_edges(manifest) {
        rev.entry(edge.target).or_default().push(edge.source);
    }

    for v in rev.values_mut() {
        v.sort();
        v.dedup();
    }
    rev
}

pub(crate) fn build_dependency_edges(manifest: &Manifest) -> Vec<DependencyEdge> {
    let mut edges: BTreeMap<(String, String), EdgeKind> = BTreeMap::new();
    let exts = builtin_source_extensions();
    let has_workspace =
        !manifest.workspace_packages.is_empty() || !manifest.workspace_roots.is_empty();

    for (source, entry) in &manifest.files {
        let source_path = Path::new(source.as_str());
        let rust_workspace_source =
            has_workspace && is_cargo_workspace_source(source_path, manifest);
        for dep in &entry.dependencies {
            if rust_workspace_source
                && (dep.starts_with("./") || dep.starts_with("../") || dep.starts_with("crate::"))
            {
                continue;
            } else if dep.starts_with("./") || dep.starts_with("../") {
                if let Some(target) = try_resolve_local_dep(dep, source, manifest, exts) {
                    merge_edge(
                        &mut edges,
                        source.clone(),
                        target,
                        dependency_kind(entry, dep),
                    );
                }
            } else {
                for target in manifest.files.keys() {
                    if dep_matches(dep, target, source, exts)
                        || python_dep_matches(dep, target, source)
                    {
                        merge_edge(
                            &mut edges,
                            source.clone(),
                            target.clone(),
                            dependency_kind(entry, dep),
                        );
                    }
                }
            }
        }

        for imp in &entry.imports {
            for target in manifest.files.keys() {
                if dotted_dep_matches(imp, target) {
                    merge_edge(
                        &mut edges,
                        source.clone(),
                        target.clone(),
                        EdgeKind::Runtime,
                    );
                }
            }
        }
    }

    if has_workspace {
        for (target, importer) in collect_workspace_edges(manifest) {
            merge_edge(&mut edges, importer, target, EdgeKind::Runtime);
        }
    }

    edges
        .into_iter()
        .map(|((source, target), kind)| DependencyEdge {
            source,
            target,
            kind,
        })
        .collect()
}

fn dependency_kind(entry: &crate::manifest::FileEntry, dependency: &str) -> EdgeKind {
    entry
        .dependency_kinds
        .get(dependency)
        .copied()
        .unwrap_or(EdgeKind::Runtime)
}

fn merge_edge(
    edges: &mut BTreeMap<(String, String), EdgeKind>,
    source: String,
    target: String,
    kind: EdgeKind,
) {
    let entry = edges.entry((source, target)).or_insert(EdgeKind::TypeOnly);
    if kind == EdgeKind::Runtime {
        *entry = EdgeKind::Runtime;
    }
}
