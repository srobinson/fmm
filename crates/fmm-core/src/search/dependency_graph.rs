use crate::manifest::{FileEntry, Manifest, builtin_source_extensions, try_resolve_local_dep};

use super::DependencyGraphQuery;
use super::helpers::{
    reverse_deps_resolve_specifier, rust_workspace_resolver, workspace_specifier_names_for_source,
};

/// Compute upstream and downstream dependencies for a file.
///
/// Upstream is split into `local` (resolved to files in the manifest) and
/// `external` (package names that could not be resolved). Python relative
/// imports (`._run`, `..config`) are resolved to file paths where possible.
pub fn dependency_graph<'a>(
    manifest: &'a Manifest,
    file: &str,
    entry: &'a FileEntry,
) -> (Vec<String>, Vec<String>, Vec<&'a String>) {
    let mut local: Vec<String> = Vec::new();
    let mut external: Vec<String> = Vec::new();
    let exts = builtin_source_extensions();
    let graph_query = DependencyGraphQuery::new(manifest).ok();
    let reverse_upstream = graph_query
        .as_ref()
        .map_or_else(Vec::new, |graph| graph.direct_upstream(file));
    let rust_resolver = rust_workspace_resolver(manifest, file);
    let workspace_specifier_names =
        workspace_specifier_names_for_source(manifest, rust_resolver.as_ref(), file);

    for dep in &entry.dependencies {
        if let Some(resolved) = try_resolve_local_dep(dep, file, manifest, exts) {
            if !local.contains(&resolved) {
                local.push(resolved);
            }
        } else if !reverse_deps_resolve_specifier(
            &workspace_specifier_names,
            &reverse_upstream,
            dep,
        ) && !external.contains(dep)
        {
            external.push(dep.clone());
        }
    }
    // entry.imports are classified as external by the parser. Package paths containing '/'
    // (npm scoped packages like `@nestjs/common/services/logger.service`, deep module paths)
    // are always external. Passing them through try_resolve_local_dep causes ghost local_deps
    // via suffix matching (e.g. `logger.service` matches `transient-logger.service.ts`).
    // Only dotted imports without '/' are tried as potential local files (Python absolute
    // imports like `agno.models.message`).
    for imp in &entry.imports {
        if !imp.contains('/')
            && let Some(resolved) = try_resolve_local_dep(imp, file, manifest, exts)
        {
            if !local.contains(&resolved) {
                local.push(resolved);
            }
            continue;
        }
        if !reverse_deps_resolve_specifier(&workspace_specifier_names, &reverse_upstream, imp)
            && !external.contains(imp)
        {
            external.push(imp.clone());
        }
    }
    for resolved in reverse_upstream {
        if !local.contains(&resolved) {
            local.push(resolved);
        }
    }
    local.sort();
    external.sort();

    let downstream = graph_query
        .as_ref()
        .map_or_else(Vec::new, |graph| graph.direct_downstream(file));

    (local, external, downstream)
}
