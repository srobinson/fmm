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

/// Discover workspace packages from the repo root.
///
/// Detection order:
/// 1. `pnpm-workspace.yaml` — pnpm workspaces
/// 2. `package.json` `workspaces` field — npm/yarn/bun workspaces
/// 3. Neither present → returns empty `WorkspaceInfo` (graceful no-op)
pub fn discover(repo_root: &Path) -> WorkspaceInfo {
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
                for entry in entries.flatten() {
                    if !entry.is_dir() {
                        continue;
                    }
                    // Check exclusion patterns
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

    // Stable sort for deterministic output
    roots.sort();

    let mut packages = HashMap::new();
    for root in &roots {
        if let Some(name) = read_package_name(root) {
            packages.insert(name, root.clone());
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
fn parse_pnpm_workspace(path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Expected structure: { packages: ["packages/*", "apps/*", "!**/test/**"] }
    let value: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    extract_string_list(&value["packages"])
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
            for m in matched.flatten() {
                if m == path {
                    return true;
                }
            }
        }
    }
    false
}

fn extract_string_list(value: &serde_yaml::Value) -> Vec<String> {
    match value {
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
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
