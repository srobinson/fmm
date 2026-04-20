use crate::manifest::{FileEntry, Manifest, builtin_source_extensions, try_resolve_local_dep};

use super::helpers::{direct_upstream_from_reverse_deps, reverse_deps_resolve_specifier};

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
    let reverse_upstream = direct_upstream_from_reverse_deps(manifest, file);

    for dep in &entry.dependencies {
        if let Some(resolved) = try_resolve_local_dep(dep, file, manifest, exts) {
            if !local.contains(&resolved) {
                local.push(resolved);
            }
        } else if !reverse_deps_resolve_specifier(manifest, &reverse_upstream, dep)
            && !external.contains(dep)
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
        if !reverse_deps_resolve_specifier(manifest, &reverse_upstream, imp)
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

    // O(1) lookup using the pre-built reverse dependency index (built at manifest load time).
    // The index maps each file to the sorted list of files that directly import it.
    let mut downstream: Vec<&String> = manifest
        .reverse_deps
        .get(file)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    downstream.sort();

    (local, external, downstream)
}
