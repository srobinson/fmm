use std::collections::HashMap;
use std::path::Path;

use crate::manifest::Manifest;

use super::local::try_resolve_local_dep;
use super::path::{builtin_source_extensions, is_cargo_workspace_source};
use super::workspace::collect_workspace_edges;
use super::{dep_matches, dotted_dep_matches, python_dep_matches};

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
pub fn build_reverse_deps(manifest: &Manifest) -> HashMap<String, Vec<String>> {
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();
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
                    rev.entry(target).or_default().push(source.clone());
                }
            } else {
                for target in manifest.files.keys() {
                    if dep_matches(dep, target, source, exts)
                        || python_dep_matches(dep, target, source)
                    {
                        rev.entry(target.clone()).or_default().push(source.clone());
                    }
                }
            }
        }

        for imp in &entry.imports {
            for target in manifest.files.keys() {
                if dotted_dep_matches(imp, target) {
                    rev.entry(target.clone()).or_default().push(source.clone());
                }
            }
        }
    }

    if has_workspace {
        for (target, importer) in collect_workspace_edges(manifest) {
            rev.entry(target).or_default().push(importer);
        }
    }

    for v in rev.values_mut() {
        v.sort();
        v.dedup();
    }
    rev
}
