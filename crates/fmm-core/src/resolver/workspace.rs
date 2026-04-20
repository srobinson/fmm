//! Workspace manifest discovery for monorepo cross-package import resolution.
//!
//! Reads pnpm-workspace.yaml or package.json workspaces globs and produces:
//! - `workspace_packages`: package name → absolute directory (for oxc-resolver alias layer)
//! - `workspace_roots`: all workspace package directories (for directory prefix heuristic)

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

/// Pluggable per-language workspace detector.
///
/// One implementor per ecosystem (JS/TS, Cargo, Go, uv, …). Multiple discoverers
/// run side-by-side in [`discover`]; results merge into a single [`WorkspaceInfo`].
pub trait WorkspaceDiscoverer: Send + Sync {
    /// Return `true` if this discoverer's manifest file exists at `repo_root`.
    /// Cheap existence check; the full parse happens in [`discover`].
    fn detect(&self, repo_root: &Path) -> bool;

    /// Discover workspace members and return their names + directories.
    fn discover(&self, repo_root: &Path) -> WorkspaceInfo;
}

/// JS/TS workspace discoverer. Reads `pnpm-workspace.yaml` (preferred) or
/// `package.json` `workspaces` field. pnpm wins when both are present, matching
/// historical behavior.
pub struct JsWorkspaceDiscoverer;

impl WorkspaceDiscoverer for JsWorkspaceDiscoverer {
    fn detect(&self, repo_root: &Path) -> bool {
        repo_root.join("pnpm-workspace.yaml").exists() || repo_root.join("package.json").exists()
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        discover_js_workspace(repo_root)
    }
}

/// Discover workspace packages from the repo root.
///
/// Runs every registered [`WorkspaceDiscoverer`] whose `detect()` returns true
/// and merges their results. Returns empty `WorkspaceInfo` when no discoverer
/// matches.
pub fn discover(repo_root: &Path) -> WorkspaceInfo {
    let discoverers: Vec<Box<dyn WorkspaceDiscoverer>> = vec![Box::new(JsWorkspaceDiscoverer)];

    let mut merged = WorkspaceInfo::default();
    for d in &discoverers {
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

fn discover_js_workspace(repo_root: &Path) -> WorkspaceInfo {
    let glob_patterns = detect_workspace_globs(repo_root);
    if glob_patterns.is_empty() {
        return WorkspaceInfo::default();
    }

    let (include_patterns, exclude_patterns) = partition_exclusions(glob_patterns);

    let mut roots = Vec::new();

    for pattern in &include_patterns {
        let abs_pattern = repo_root.join(pattern);
        let pattern_str = abs_pattern.to_string_lossy();

        match glob::glob(&pattern_str) {
            Ok(entries) => {
                for entry in entries.filter_map(Result::ok) {
                    if !entry.is_dir() {
                        continue;
                    }
                    if is_excluded(&entry, repo_root, &exclude_patterns) {
                        continue;
                    }
                    roots.push(entry);
                }
            }
            Err(e) => {
                tracing_debug_or_eprintln(&format!(
                    "workspace: bad glob pattern '{}': {}",
                    pattern, e
                ));
            }
        }
    }

    roots.sort();

    let mut packages = HashMap::new();
    for root in &roots {
        if let Some(name) = read_package_name(root) {
            packages.insert(name, root.to_path_buf());
        }
    }

    WorkspaceInfo { packages, roots }
}

/// Detect workspace glob patterns from the repo root.
/// Returns an empty vec if no workspace config is found.
fn detect_workspace_globs(repo_root: &Path) -> Vec<String> {
    // pnpm takes precedence when both files exist
    let pnpm_yaml = repo_root.join("pnpm-workspace.yaml");
    if pnpm_yaml.exists() {
        return parse_pnpm_workspace(&pnpm_yaml);
    }

    let pkg_json = repo_root.join("package.json");
    if pkg_json.exists() {
        return parse_npm_workspaces(&pkg_json);
    }

    Vec::new()
}

/// Parse pnpm-workspace.yaml → list of glob strings.
///
/// Uses a lightweight line-based parser instead of a full YAML library.
/// Expected format:
/// ```yaml
/// packages:
///   - 'packages/*'
///   - "apps/*"
///   - plain-glob/*
/// ```
fn parse_pnpm_workspace(path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
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

/// Parse package.json `workspaces` field → list of glob strings.
fn parse_npm_workspaces(path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let workspaces = &value["workspaces"];

    // npm/yarn format: { "workspaces": ["packages/*"] }
    // yarn berry format: { "workspaces": { "packages": ["packages/*"] } }
    if workspaces.is_array() {
        extract_json_string_list(workspaces)
    } else if let Some(pkgs) = workspaces.get("packages") {
        extract_json_string_list(pkgs)
    } else {
        Vec::new()
    }
}

/// Read the `name` field from a package.json in the given directory.
pub fn read_package_name(dir: &Path) -> Option<String> {
    let pkg_json = dir.join("package.json");
    let content = std::fs::read_to_string(pkg_json).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value["name"].as_str().map(|s| s.to_string())
}

/// Partition glob patterns into (includes, excludes).
/// Exclusion patterns start with `!`.
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

/// Check whether `path` matches any exclusion pattern relative to `repo_root`.
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

fn tracing_debug_or_eprintln(msg: &str) {
    // Use eprintln at debug level — avoids pulling in tracing just for this module.
    // In production builds this is silent; only visible with RUST_LOG=debug.
    let _ = msg; // suppress unused warning in release; callers can check RUST_LOG
    #[cfg(debug_assertions)]
    eprintln!("[fmm debug] {}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_dir(base: &Path, rel: &str) -> PathBuf {
        let p = base.join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_file(base: &Path, rel: &str, content: &str) {
        let p = base.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn no_workspace_config_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let info = discover(tmp.path());
        assert!(info.packages.is_empty());
        assert!(info.roots.is_empty());
    }

    #[test]
    fn js_discoverer_detects_pnpm_workspace_yaml() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "pnpm-workspace.yaml", "packages:\n  - 'a/*'\n");
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_detects_package_json() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"name":"root"}"#);
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_does_not_detect_when_neither_present() {
        let tmp = TempDir::new().unwrap();
        assert!(!JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn discover_merges_dedups_and_sorts_roots() {
        // Stand-in second discoverer that returns a fixed set of roots and packages.
        // Demonstrates the merge contract: extend, sort, dedup.
        struct FakeDiscoverer {
            extra_root: PathBuf,
            pkg_name: String,
        }
        impl WorkspaceDiscoverer for FakeDiscoverer {
            fn detect(&self, _r: &Path) -> bool {
                true
            }
            fn discover(&self, _r: &Path) -> WorkspaceInfo {
                let mut packages = HashMap::new();
                packages.insert(self.pkg_name.clone(), self.extra_root.clone());
                WorkspaceInfo {
                    packages,
                    roots: vec![self.extra_root.clone()],
                }
            }
        }

        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"workspaces":["packages/*"]}"#,
        );
        make_dir(tmp.path(), "packages/alpha");
        write_file(
            tmp.path(),
            "packages/alpha/package.json",
            r#"{"name":"alpha"}"#,
        );

        let alpha_root = tmp.path().join("packages/alpha");
        let extra_root = tmp.path().join("crates/foo");
        make_dir(tmp.path(), "crates/foo");

        // Manually exercise the merge code path with two discoverers, including
        // one that overlaps a root the JS discoverer also returns. The dedup
        // step must collapse the overlap.
        let discoverers: Vec<Box<dyn WorkspaceDiscoverer>> = vec![
            Box::new(JsWorkspaceDiscoverer),
            Box::new(FakeDiscoverer {
                extra_root: extra_root.clone(),
                pkg_name: "foo".into(),
            }),
            Box::new(FakeDiscoverer {
                extra_root: alpha_root.clone(),
                pkg_name: "alpha-twin".into(),
            }),
        ];
        let mut merged = WorkspaceInfo::default();
        for d in &discoverers {
            if d.detect(tmp.path()) {
                let info = d.discover(tmp.path());
                merged.packages.extend(info.packages);
                merged.roots.extend(info.roots);
            }
        }
        merged.roots.sort();
        merged.roots.dedup();

        assert_eq!(merged.roots, {
            let mut v = vec![alpha_root.clone(), extra_root.clone()];
            v.sort();
            v
        });
        assert_eq!(merged.packages.get("alpha").unwrap(), &alpha_root);
        assert_eq!(merged.packages.get("foo").unwrap(), &extra_root);
        assert_eq!(merged.packages.get("alpha-twin").unwrap(), &alpha_root);
    }

    #[test]
    fn npm_workspaces_single_glob() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"workspaces": ["packages/*"]}"#,
        );
        make_dir(tmp.path(), "packages/alpha");
        write_file(
            tmp.path(),
            "packages/alpha/package.json",
            r#"{"name": "alpha"}"#,
        );
        make_dir(tmp.path(), "packages/beta");
        write_file(
            tmp.path(),
            "packages/beta/package.json",
            r#"{"name": "@scope/beta"}"#,
        );

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 2);
        assert_eq!(
            info.packages.get("alpha").unwrap(),
            &tmp.path().join("packages/alpha")
        );
        assert_eq!(
            info.packages.get("@scope/beta").unwrap(),
            &tmp.path().join("packages/beta")
        );
    }

    #[test]
    fn pnpm_workspace_takes_precedence() {
        let tmp = TempDir::new().unwrap();
        // Both files present — pnpm should win
        write_file(tmp.path(), "package.json", r#"{"workspaces": ["apps/*"]}"#);
        write_file(
            tmp.path(),
            "pnpm-workspace.yaml",
            "packages:\n  - 'packages/*'\n",
        );
        make_dir(tmp.path(), "packages/lib");
        write_file(
            tmp.path(),
            "packages/lib/package.json",
            r#"{"name": "lib"}"#,
        );
        make_dir(tmp.path(), "apps/web");
        write_file(tmp.path(), "apps/web/package.json", r#"{"name": "web"}"#);

        let info = discover(tmp.path());
        // Only packages/* was expanded (pnpm config)
        assert!(info.packages.contains_key("lib"));
        assert!(!info.packages.contains_key("web"));
    }

    #[test]
    fn directory_without_package_json_included_in_roots_not_packages() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"workspaces": ["packages/*"]}"#,
        );
        make_dir(tmp.path(), "packages/unnamed"); // no package.json

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 1);
        assert!(info.packages.is_empty()); // no name → not in packages map
    }

    #[test]
    fn multiple_workspace_glob_patterns() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"workspaces": ["packages/*", "apps/*"]}"#,
        );
        make_dir(tmp.path(), "packages/lib");
        write_file(
            tmp.path(),
            "packages/lib/package.json",
            r#"{"name": "lib"}"#,
        );
        make_dir(tmp.path(), "apps/frontend");
        write_file(
            tmp.path(),
            "apps/frontend/package.json",
            r#"{"name": "frontend"}"#,
        );

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 2);
        assert!(info.packages.contains_key("lib"));
        assert!(info.packages.contains_key("frontend"));
    }

    #[test]
    fn exclusion_patterns_respected() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "pnpm-workspace.yaml",
            "packages:\n  - 'packages/*'\n  - '!packages/test-utils'\n",
        );
        make_dir(tmp.path(), "packages/core");
        write_file(
            tmp.path(),
            "packages/core/package.json",
            r#"{"name": "core"}"#,
        );
        make_dir(tmp.path(), "packages/test-utils");
        write_file(
            tmp.path(),
            "packages/test-utils/package.json",
            r#"{"name": "test-utils"}"#,
        );

        let info = discover(tmp.path());
        assert!(info.packages.contains_key("core"));
        assert!(!info.packages.contains_key("test-utils"));
    }
}
