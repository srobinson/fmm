//! Workspace manifest discovery for monorepo cross-package import resolution.
//!
//! Pluggable per-language. The shipped [`JsWorkspaceDiscoverer`] covers the
//! five mainstream JS package managers (npm, yarn, pnpm, bun, deno). Future
//! discoverers (Cargo, Go, uv) plug in via the [`WorkspaceDiscoverer`] trait
//! and merge into the same [`WorkspaceInfo`].

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
    /// Return `true` if this ecosystem looks active at `repo_root`. Cheap signal
    /// only — `discover()` may still return empty `WorkspaceInfo` if the
    /// manifest is malformed or carries no workspace declaration.
    fn detect(&self, repo_root: &Path) -> bool;

    /// Discover workspace members and return their names + directories.
    fn discover(&self, repo_root: &Path) -> WorkspaceInfo;
}

/// JS/TS workspace discoverer. Covers all five mainstream JS package managers:
///
/// | PM   | Manifest                       | Field                                  | Lock file           |
/// |------|--------------------------------|----------------------------------------|---------------------|
/// | pnpm | `pnpm-workspace.yaml`          | top-level `packages:` list (globs)     | `pnpm-lock.yaml`    |
/// | npm  | `package.json`                 | `workspaces` array (globs)             | `package-lock.json` |
/// | yarn | `package.json`                 | `workspaces` array OR `{packages:[…]}` | `yarn.lock`         |
/// | bun  | `package.json`                 | `workspaces` array (globs)             | `bun.lockb`         |
/// | deno | `deno.json` or `deno.jsonc`    | `workspace` array (literal paths)      | `deno.lock`         |
///
/// `pnpm-workspace.yaml` wins over `package.json` `workspaces` when both
/// declare members, matching historical behavior. deno members merge in
/// additively (a polyglot deno + npm repo discovers both sets).
///
/// `detect()` is permissive: any workspace manifest *or* lock file marks the
/// repo as JS. `discover()` parses the manifests strictly and returns empty
/// when nothing is declared.
pub struct JsWorkspaceDiscoverer;

const JS_LOCK_FILES: &[&str] = &[
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "bun.lockb",
    "deno.lock",
];

impl WorkspaceDiscoverer for JsWorkspaceDiscoverer {
    fn detect(&self, repo_root: &Path) -> bool {
        Self::has_pnpm_manifest(repo_root)
            || Self::has_npm_workspaces(repo_root)
            || Self::has_deno_workspace(repo_root)
            || Self::has_js_lock_file(repo_root)
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        let mut roots = Vec::new();

        // pnpm + npm/yarn/bun: glob-expanded patterns from one manifest.
        // pnpm wins when both files exist (historical precedence).
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

        // deno: literal paths, additive (orthogonal to pnpm/npm precedence).
        for rel in Self::parse_deno_workspace(repo_root) {
            let abs = repo_root.join(&rel);
            if abs.is_dir() {
                roots.push(abs);
            } else {
                debug_log(&format!(
                    "workspace: deno workspace member '{}' is not a directory",
                    rel
                ));
            }
        }

        roots.sort();
        roots.dedup();

        let mut packages = HashMap::new();
        for root in &roots {
            if let Some(name) = Self::read_member_name(root) {
                packages.insert(name, root.to_path_buf());
            }
        }

        WorkspaceInfo { packages, roots }
    }
}

impl JsWorkspaceDiscoverer {
    fn has_pnpm_manifest(repo_root: &Path) -> bool {
        repo_root.join("pnpm-workspace.yaml").exists()
    }

    /// True when `package.json` exists *and* declares a non-null `workspaces`
    /// field (array or object form). A bare package.json with only deps is
    /// not a workspace root.
    fn has_npm_workspaces(repo_root: &Path) -> bool {
        let pkg_json = repo_root.join("package.json");
        let Ok(content) = std::fs::read_to_string(&pkg_json) else {
            return false;
        };
        let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
            return false;
        };
        !value["workspaces"].is_null()
    }

    /// True when `deno.json` or `deno.jsonc` exists *and* declares a non-null
    /// `workspace` field.
    fn has_deno_workspace(repo_root: &Path) -> bool {
        for name in ["deno.json", "deno.jsonc"] {
            let path = repo_root.join(name);
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let stripped = strip_jsonc(&content);
            let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&stripped) else {
                continue;
            };
            if !value["workspace"].is_null() {
                return true;
            }
        }
        false
    }

    fn has_js_lock_file(repo_root: &Path) -> bool {
        JS_LOCK_FILES
            .iter()
            .any(|name| repo_root.join(name).exists())
    }

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

    /// Parse `deno.json` (or `deno.jsonc`) `workspace` field → list of paths.
    /// deno members are literal directory paths, not globs.
    fn parse_deno_workspace(repo_root: &Path) -> Vec<String> {
        for name in ["deno.json", "deno.jsonc"] {
            let path = repo_root.join(name);
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let stripped = strip_jsonc(&content);
            let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&stripped) else {
                continue;
            };
            let ws = &value["workspace"];
            if ws.is_array() {
                return extract_json_string_list(ws);
            }
        }
        Vec::new()
    }

    /// Read the canonical package name for a workspace member directory.
    /// Tries `package.json` `name` first (npm/yarn/pnpm/bun), then
    /// `deno.json` / `deno.jsonc` `name` (deno).
    fn read_member_name(dir: &Path) -> Option<String> {
        if let Some(name) = read_json_string_field(&dir.join("package.json"), "name", false) {
            return Some(name);
        }
        for fname in ["deno.json", "deno.jsonc"] {
            if let Some(name) = read_json_string_field(&dir.join(fname), "name", true) {
                return Some(name);
            }
        }
        None
    }
}

/// Top-level orchestrator: run every registered [`WorkspaceDiscoverer`] whose
/// `detect()` matches and merge results. Roots are sorted + deduped; package
/// names are last-writer-wins on collision (rare but possible across
/// ecosystems).
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

fn read_json_string_field(path: &Path, field: &str, jsonc: bool) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let parsed = if jsonc {
        serde_json::from_str::<serde_json::Value>(&strip_jsonc(&content)).ok()?
    } else {
        serde_json::from_str::<serde_json::Value>(&content).ok()?
    };
    parsed[field].as_str().map(|s| s.to_string())
}

/// Strip `//` line comments and `/* … */` block comments from a JSONC source,
/// preserving string literal content (including escaped quotes). Trailing
/// commas are not handled — deno.jsonc files in the wild do not use them, and
/// serde_json is strict.
fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    for nc in chars.by_ref() {
                        if nc == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut prev_star = false;
                    for nc in chars.by_ref() {
                        if prev_star && nc == '/' {
                            break;
                        }
                        prev_star = nc == '*';
                    }
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }

    out
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
    fn js_discoverer_detects_pnpm_workspace_yaml() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "pnpm-workspace.yaml", "packages:\n  - 'a/*'\n");
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_detects_package_json_with_workspaces() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"name":"root","workspaces":["packages/*"]}"#,
        );
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_does_not_detect_bare_package_json_without_workspaces() {
        // package.json present but no workspaces field AND no lock file → not a
        // workspace root and not a JS project we can speak about.
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"name":"root"}"#);
        assert!(!JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_detects_lock_files_individually() {
        // Each lock file alone is a sufficient JS-ecosystem signal.
        for lock in JS_LOCK_FILES {
            let tmp = TempDir::new().unwrap();
            write_file(tmp.path(), lock, "");
            assert!(
                JsWorkspaceDiscoverer.detect(tmp.path()),
                "expected detect() = true for {}",
                lock
            );
        }
    }

    #[test]
    fn js_discoverer_detects_deno_json_workspace() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "deno.json",
            r#"{"workspace":["./add","./subtract"]}"#,
        );
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_detects_deno_jsonc_workspace_with_comments() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "deno.jsonc",
            "// top comment\n{\n  /* block */\n  \"workspace\": [\"./a\"]\n}\n",
        );
        assert!(JsWorkspaceDiscoverer.detect(tmp.path()));
    }

    #[test]
    fn js_discoverer_does_not_detect_when_nothing_present() {
        let tmp = TempDir::new().unwrap();
        assert!(!JsWorkspaceDiscoverer.detect(tmp.path()));
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
        // yarn berry: { workspaces: { packages: [...], nohoist: [...] } }
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
        // bun shares npm's `workspaces` array. Lock file is bun.lockb. Both
        // should be enough — together or alone — to drive the same discovery.
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "package.json", r#"{"workspaces":["apps/*"]}"#);
        write_file(tmp.path(), "bun.lockb", "");
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
    fn deno_workspace_paths_are_literal() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "deno.json",
            r#"{"workspace":["./add","./subtract"]}"#,
        );
        make_dir(tmp.path(), "add");
        write_file(tmp.path(), "add/deno.json", r#"{"name":"@scope/add"}"#);
        make_dir(tmp.path(), "subtract");
        write_file(
            tmp.path(),
            "subtract/deno.json",
            r#"{"name":"@scope/subtract"}"#,
        );

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 2);
        assert_eq!(
            info.packages.get("@scope/add").unwrap(),
            &tmp.path().join("add")
        );
        assert_eq!(
            info.packages.get("@scope/subtract").unwrap(),
            &tmp.path().join("subtract")
        );
    }

    #[test]
    fn deno_jsonc_workspace_with_comments_parses() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "deno.jsonc",
            r#"{
  // workspace members
  "workspace": [
    "./mod-a", /* first */
    "./mod-b"  // second
  ]
}
"#,
        );
        make_dir(tmp.path(), "mod-a");
        write_file(tmp.path(), "mod-a/deno.json", r#"{"name":"mod-a"}"#);
        make_dir(tmp.path(), "mod-b");
        write_file(tmp.path(), "mod-b/deno.json", r#"{"name":"mod-b"}"#);

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 2);
        assert!(info.packages.contains_key("mod-a"));
        assert!(info.packages.contains_key("mod-b"));
    }

    #[test]
    fn deno_and_npm_workspaces_merge_additively() {
        // Polyglot deno + npm repo: both sets of members surface.
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "package.json",
            r#"{"workspaces":["js-pkgs/*"]}"#,
        );
        write_file(tmp.path(), "deno.json", r#"{"workspace":["./deno-mod"]}"#);
        make_dir(tmp.path(), "js-pkgs/web");
        write_file(tmp.path(), "js-pkgs/web/package.json", r#"{"name":"web"}"#);
        make_dir(tmp.path(), "deno-mod");
        write_file(tmp.path(), "deno-mod/deno.json", r#"{"name":"deno-mod"}"#);

        let info = discover(tmp.path());
        assert_eq!(info.roots.len(), 2);
        assert!(info.packages.contains_key("web"));
        assert!(info.packages.contains_key("deno-mod"));
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
    fn discover_merges_dedups_and_sorts_roots() {
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
    fn strip_jsonc_preserves_string_with_slash_slash() {
        // String literals must not be touched by the comment stripper.
        let input = r#"{"path":"a//b","note":"/* not a comment */"}"#;
        let stripped = strip_jsonc(input);
        let v: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["path"].as_str().unwrap(), "a//b");
        assert_eq!(v["note"].as_str().unwrap(), "/* not a comment */");
    }

    #[test]
    fn strip_jsonc_handles_escaped_quote_in_string() {
        let input = r#"{"q":"he said \"hi\" // not comment"}"#;
        let stripped = strip_jsonc(input);
        let v: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["q"].as_str().unwrap(), r#"he said "hi" // not comment"#);
    }
}
