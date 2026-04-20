//! Workspace manifest discovery for monorepo cross-package import resolution.
//!
//! Pluggable per ecosystem. The shipped [`JsWorkspaceDiscoverer`] covers the
//! four mainstream JS package managers (npm, yarn, pnpm, bun). Future
//! discoverers (Cargo, Go, uv, Deno) plug in via the [`WorkspaceDiscoverer`]
//! trait and merge into the same [`WorkspaceInfo`].

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
    let discoverers: Vec<Box<dyn WorkspaceDiscoverer>> = vec![Box::new(JsWorkspaceDiscoverer)];
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
    fn js_discoverer_detects_package_json() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"name":"root"}"#);
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_detects_node_modules() {
        let tmp = TempDir::new().unwrap();
        make_dir(tmp.path(), "node_modules");
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_detects_pnpm_workspace_without_root_package_json() {
        // pnpm allows a workspace root with only pnpm-workspace.yaml
        // (no root package.json).
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "pnpm-workspace.yaml", "packages:\n  - 'a/*'\n");
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_does_not_detect_empty_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(!JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn bare_package_json_detects_but_discovers_empty() {
        // Bare package.json (no workspaces field) trips detect() because the
        // gate is a cheap perf check. discover() then returns an empty
        // WorkspaceInfo, which is the correct semantic answer.
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"name":"root"}"#);
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
        let info = JsWorkspaceDiscoverer.discover(tmp.path());
        assert!(info.roots.is_empty());
        assert!(info.packages.is_empty());
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
    fn yarn_berry_object_form_workspaces() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"workspaces":{"packages":["packages/*"],"nohoist":["**/react"]}}"#,
        );
        make_dir(tmp.path(), "packages/lib");
        write_file(tmp.path(), "packages/lib/package.json", r#"{"name":"lib"}"#);

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 1);
        assert!(info.packages.contains_key("lib"));
    }

    #[test]
    fn bun_workspaces_use_npm_format() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"workspaces":["apps/*"]}"#);
        make_dir(tmp.path(), "apps/web");
        write_file(tmp.path(), "apps/web/package.json", r#"{"name":"web"}"#);

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 1);
        assert!(info.packages.contains_key("web"));
    }

    #[test]
    fn pnpm_workspace_takes_precedence_over_package_json() {
        let tmp = TempDir::new().unwrap();
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
        make_dir(tmp.path(), "packages/unnamed");

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 1);
        assert!(info.packages.is_empty());
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

    #[test]
    fn discover_with_merges_dedups_and_sorts_roots() {
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
        let merged = discover_with(tmp.path(), &discoverers);

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
    fn read_package_name_returns_name_field() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"name":"hello"}"#);
        assert_eq!(read_package_name(tmp.path()), Some("hello".into()));
    }

    #[test]
    fn read_package_name_missing_file_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert!(read_package_name(tmp.path()).is_none());
    }
}
