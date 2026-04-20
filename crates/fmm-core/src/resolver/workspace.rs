//! Workspace manifest discovery for monorepo cross-package import resolution.
//!
//! Pluggable per ecosystem. The shipped discoverers cover JS/TS package
//! managers and Cargo workspaces. Future discoverers (Go, uv, Deno) plug in
//! via the [`WorkspaceDiscoverer`] trait and merge into the same
//! [`WorkspaceInfo`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Result of workspace discovery.
#[derive(Debug, Default)]
pub struct WorkspaceInfo {
    /// Maps canonical package name → absolute directory path.
    /// e.g. `"shared"` → `/repo/packages/shared`
    /// Used as `alias` entries in oxc-resolver (Layer 2).
    pub packages: HashMap<String, PathBuf>,
    /// Absolute directory paths of all workspace package roots.
    /// Includes directories even if they have no package.json name.
    /// Used by the directory prefix heuristic (Layer 3).
    pub roots: Vec<PathBuf>,
}

/// Pluggable workspace detector. One implementor per resolver family
/// (JS/TS, Cargo, Go, uv, Deno, ...). Multiple discoverers run side by side
/// in [`discover`]; results merge into a single [`WorkspaceInfo`].
pub trait WorkspaceDiscoverer: Send + Sync {
    /// Cheap signal: is this ecosystem plausibly active at `repo_root`?
    /// `discover()` may still return an empty `WorkspaceInfo`.
    fn detect(&self, repo_root: &Path) -> bool;

    /// Discover workspace members and return their names + directories.
    fn discover(&self, repo_root: &Path) -> WorkspaceInfo;
}

/// JS/TS workspace discoverer. Covers all four mainstream JS package managers:
///
/// | PM   | Manifest              | Field                                  |
/// |------|-----------------------|----------------------------------------|
/// | pnpm | `pnpm-workspace.yaml` | top-level `packages:` list (globs)     |
/// | npm  | `package.json`        | `workspaces` array (globs)             |
/// | yarn | `package.json`        | `workspaces` array OR `{packages:[…]}` |
/// | bun  | `package.json`        | `workspaces` array (globs)             |
///
/// `pnpm-workspace.yaml` wins over `package.json` `workspaces` when both
/// declare members, matching historical behavior.
///
/// Deno is a separate ecosystem (URL imports, JSR, import maps) and ships
/// its own discoverer + resolver.
pub struct JsWorkspaceDiscoverer;

/// Cargo workspace discoverer. Reads a root `Cargo.toml` `[workspace]`
/// manifest, expands member globs, and maps Rust crate names to package roots.
pub struct CargoWorkspaceDiscoverer;

impl WorkspaceDiscoverer for JsWorkspaceDiscoverer {
    fn detect(&self, repo_root: &Path) -> bool {
        // Primary manifests: package.json (npm/yarn/bun/pnpm) or
        // pnpm-workspace.yaml (pnpm workspaces can declare members without a
        // root package.json). node_modules covers the post-install case where
        // a repo has installed deps but the manifests have been moved or hidden
        // from indexing. Lockfiles are ceremony; they do not add information.
        repo_root.join("package.json").exists()
            || repo_root.join("pnpm-workspace.yaml").exists()
            || repo_root.join("node_modules").exists()
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        let mut roots = Vec::new();

        let glob_patterns = Self::detect_workspace_globs(repo_root);
        if !glob_patterns.is_empty() {
            let (includes, excludes) = partition_exclusions(glob_patterns);
            for pattern in &includes {
                let abs_pattern = repo_root.join(pattern);
                let pattern_str = abs_pattern.to_string_lossy();
                match glob::glob(&pattern_str) {
                    Ok(entries) => {
                        for entry in entries.filter_map(Result::ok) {
                            if !entry.is_dir() {
                                continue;
                            }
                            if is_excluded(&entry, repo_root, &excludes) {
                                continue;
                            }
                            roots.push(entry);
                        }
                    }
                    Err(e) => {
                        debug_log(&format!("workspace: bad glob pattern '{}': {}", pattern, e));
                    }
                }
            }
        }

        roots.sort();
        roots.dedup();

        let mut packages = HashMap::new();
        for root in &roots {
            if let Some(name) = read_package_name(root) {
                packages.insert(name, root.to_path_buf());
            }
        }

        WorkspaceInfo { packages, roots }
    }
}

impl JsWorkspaceDiscoverer {
    /// pnpm > npm/yarn/bun. Returns the first non-empty glob list found.
    fn detect_workspace_globs(repo_root: &Path) -> Vec<String> {
        let pnpm_yaml = repo_root.join("pnpm-workspace.yaml");
        if pnpm_yaml.exists() {
            return Self::parse_pnpm_workspace(&pnpm_yaml);
        }

        let pkg_json = repo_root.join("package.json");
        if pkg_json.exists() {
            return Self::parse_npm_workspaces(&pkg_json);
        }

        Vec::new()
    }

    /// Parse `pnpm-workspace.yaml` → list of glob strings.
    ///
    /// Lightweight line-based parser to avoid pulling in a YAML dep.
    /// ```yaml
    /// packages:
    ///   - 'packages/*'
    ///   - "apps/*"
    ///   - plain-glob/*
    /// ```
    fn parse_pnpm_workspace(path: &Path) -> Vec<String> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };

        let mut in_packages = false;
        let mut results = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "packages:" || trimmed.starts_with("packages:") {
                in_packages = true;
                continue;
            }
            if in_packages {
                if trimmed.starts_with('-') {
                    let raw = trimmed.trim_start_matches('-').trim();
                    let unquoted = raw.trim_matches('\'').trim_matches('"').to_string();
                    if !unquoted.is_empty() {
                        results.push(unquoted);
                    }
                } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    in_packages = false;
                }
            }
        }

        results
    }

    /// Parse `package.json` `workspaces` field. Covers npm, yarn (classic),
    /// yarn berry's object form `{ packages: [...], nohoist: [...] }`, and
    /// bun (which uses the npm format).
    fn parse_npm_workspaces(path: &Path) -> Vec<String> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
            return Vec::new();
        };

        let workspaces = &value["workspaces"];
        if workspaces.is_array() {
            extract_json_string_list(workspaces)
        } else if let Some(pkgs) = workspaces.get("packages") {
            extract_json_string_list(pkgs)
        } else {
            Vec::new()
        }
    }
}

impl WorkspaceDiscoverer for CargoWorkspaceDiscoverer {
    fn detect(&self, repo_root: &Path) -> bool {
        let Some(manifest) = read_cargo_manifest(repo_root) else {
            return false;
        };
        manifest
            .get("workspace")
            .and_then(|v| v.as_table())
            .is_some()
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        let Some(manifest) = read_cargo_manifest(repo_root) else {
            return WorkspaceInfo::default();
        };
        if manifest
            .get("workspace")
            .and_then(|v| v.as_table())
            .is_none()
        {
            return WorkspaceInfo::default();
        }

        let excludes = cargo_workspace_excludes(repo_root, &manifest);
        let mut roots = Vec::new();
        if crate_name_from_cargo_manifest(&manifest).is_some()
            && !is_cargo_excluded(repo_root, &excludes)
        {
            roots.push(repo_root.to_path_buf());
        }

        for pattern in cargo_workspace_members(&manifest) {
            let abs_pattern = repo_root.join(&pattern);
            let pattern_str = abs_pattern.to_string_lossy();
            match glob::glob(&pattern_str) {
                Ok(entries) => {
                    for entry in entries.filter_map(Result::ok) {
                        if !entry.is_dir() || is_cargo_excluded(&entry, &excludes) {
                            continue;
                        }
                        if read_cargo_crate_name(&entry).is_some() {
                            roots.push(entry);
                        }
                    }
                }
                Err(e) => {
                    debug_log(&format!("cargo workspace: bad glob '{}': {}", pattern, e));
                }
            }
        }

        roots.sort();
        roots.dedup();

        let mut packages = HashMap::new();
        for root in &roots {
            if let Some(name) = read_cargo_crate_name(root) {
                packages.insert(name, root.to_path_buf());
            }
        }

        WorkspaceInfo { packages, roots }
    }
}

fn read_cargo_manifest(dir: &Path) -> Option<toml::Value> {
    let content = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    toml::from_str(&content).ok()
}

fn cargo_workspace_members(manifest: &toml::Value) -> Vec<String> {
    cargo_workspace_string_list(manifest, "members")
}

fn cargo_workspace_excludes(repo_root: &Path, manifest: &toml::Value) -> Vec<PathBuf> {
    cargo_workspace_string_list(manifest, "exclude")
        .into_iter()
        .map(|p| repo_root.join(p))
        .collect()
}

fn cargo_workspace_string_list(manifest: &toml::Value, key: &str) -> Vec<String> {
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get(key))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn is_cargo_excluded(path: &Path, exclude_paths: &[PathBuf]) -> bool {
    exclude_paths.iter().any(|excluded| excluded == path)
}

fn read_cargo_crate_name(dir: &Path) -> Option<String> {
    let manifest = read_cargo_manifest(dir)?;
    crate_name_from_cargo_manifest(&manifest)
}

fn crate_name_from_cargo_manifest(manifest: &toml::Value) -> Option<String> {
    let package_name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())?;

    let lib_name = manifest
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .and_then(|name| name.as_str());

    Some(
        lib_name
            .unwrap_or(package_name)
            .replace('-', "_")
            .to_string(),
    )
}

/// Read the `name` field from `package.json` at `dir`. Returns `None` when
/// the file is missing, malformed, or has no string `name` field.
pub fn read_package_name(dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value["name"].as_str().map(String::from)
}

/// Top-level orchestrator: run every registered [`WorkspaceDiscoverer`] whose
/// `detect()` matches and merge results. Roots are sorted and deduped;
/// package names follow last writer wins on collision (rare but possible
/// across ecosystems).
pub fn discover(repo_root: &Path) -> WorkspaceInfo {
    let discoverers: Vec<Box<dyn WorkspaceDiscoverer>> = vec![
        Box::new(CargoWorkspaceDiscoverer),
        Box::new(JsWorkspaceDiscoverer),
    ];
    discover_with(repo_root, &discoverers)
}

/// Same as [`discover`] but the discoverer list is supplied by the caller.
/// Enables tests to exercise the merge path against fake discoverers.
pub fn discover_with(
    repo_root: &Path,
    discoverers: &[Box<dyn WorkspaceDiscoverer>],
) -> WorkspaceInfo {
    let mut merged = WorkspaceInfo::default();
    for d in discoverers {
        if d.detect(repo_root) {
            let info = d.discover(repo_root);
            merged.packages.extend(info.packages);
            merged.roots.extend(info.roots);
        }
    }
    merged.roots.sort();
    merged.roots.dedup();
    merged
}

fn partition_exclusions(patterns: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut includes = Vec::new();
    let mut excludes = Vec::new();
    for p in patterns {
        if let Some(excl) = p.strip_prefix('!') {
            excludes.push(excl.to_string());
        } else {
            includes.push(p);
        }
    }
    (includes, excludes)
}

fn is_excluded(path: &Path, repo_root: &Path, exclude_patterns: &[String]) -> bool {
    if exclude_patterns.is_empty() {
        return false;
    }
    for pattern in exclude_patterns {
        let abs_pattern = repo_root.join(pattern);
        if let Ok(matched) = glob::glob(&abs_pattern.to_string_lossy()) {
            for m in matched.filter_map(Result::ok) {
                if m == path {
                    return true;
                }
            }
        }
    }
    false
}

fn extract_json_string_list(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn debug_log(msg: &str) {
    let _ = msg;
    #[cfg(debug_assertions)]
    eprintln!("[fmm debug] {}", msg);
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod workspace_tests;
