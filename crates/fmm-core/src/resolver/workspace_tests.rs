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

fn write_crate(base: &Path, rel: &str, name: &str) {
    write_file(
        base,
        &format!("{rel}/Cargo.toml"),
        &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
    );
}

fn write_go_module(base: &Path, rel: &str, module_path: &str) {
    write_file(
        base,
        &format!("{rel}/go.mod"),
        &format!("module {module_path}\n\ngo 1.23.0\n"),
    );
}

fn write_python_project(base: &Path, rel: &str, name: &str) {
    write_file(
        base,
        &format!("{rel}/pyproject.toml"),
        &format!("[project]\nname = \"{name}\"\n"),
    );
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

#[test]
fn cargo_discoverer_detects_workspace_manifest() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "Cargo.toml", "[workspace]\nmembers = []\n");
    assert!(CargoWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn cargo_discoverer_does_not_detect_missing_manifest() {
    let tmp = TempDir::new().unwrap();
    assert!(!CargoWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn cargo_workspace_single_glob_discovers_all_members() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_crate(tmp.path(), "crates/fmm-core", "fmm-core");
    write_crate(tmp.path(), "crates/fmm-store", "fmm-store");

    let info = discover(tmp.path());

    assert_eq!(info.roots.len(), 2);
    assert_eq!(
        info.packages.get("fmm_core").unwrap(),
        &tmp.path().join("crates/fmm-core")
    );
    assert_eq!(
        info.packages.get("fmm_store").unwrap(),
        &tmp.path().join("crates/fmm-store")
    );
}

#[test]
fn cargo_workspace_exclude_filters_exact_paths() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/skip-me\"]\n",
    );
    write_crate(tmp.path(), "crates/core", "core");
    write_crate(tmp.path(), "crates/skip-me", "skip-me");

    let info = discover(tmp.path());

    assert!(info.packages.contains_key("core"));
    assert!(!info.packages.contains_key("skip_me"));
    assert_eq!(info.roots, vec![tmp.path().join("crates/core")]);
}

#[test]
fn cargo_virtual_workspace_excludes_root_package() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_crate(tmp.path(), "crates/core", "core");

    let info = discover(tmp.path());

    assert_eq!(info.roots, vec![tmp.path().join("crates/core")]);
    assert!(!info.packages.contains_key("root"));
}

#[test]
fn cargo_non_virtual_workspace_includes_root_package() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"root-app\"\nversion = \"0.1.0\"\n\n[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_crate(tmp.path(), "crates/core", "core");

    let info = discover(tmp.path());

    assert_eq!(info.roots.len(), 2);
    assert_eq!(info.packages.get("root_app").unwrap(), tmp.path());
    assert_eq!(
        info.packages.get("core").unwrap(),
        &tmp.path().join("crates/core")
    );
}

#[test]
fn cargo_crate_name_replaces_hyphens_with_underscores() {
    let tmp = TempDir::new().unwrap();
    write_crate(tmp.path(), ".", "hello-world");

    assert_eq!(
        read_cargo_crate_name(tmp.path()),
        Some("hello_world".into())
    );
}

#[test]
fn cargo_lib_name_overrides_package_name() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"package-name\"\nversion = \"0.1.0\"\n\n[lib]\nname = \"custom_lib\"\n",
    );

    assert_eq!(read_cargo_crate_name(tmp.path()), Some("custom_lib".into()));
}

#[test]
fn python_discoverer_detects_uv_workspace() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"root-app\"\n\n[tool.uv.workspace]\nmembers = []\n",
    );

    assert!(PythonWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn python_discoverer_does_not_detect_plain_pyproject() {
    let tmp = TempDir::new().unwrap();
    write_python_project(tmp.path(), ".", "root-app");

    assert!(!PythonWorkspaceDiscoverer.detect(tmp.path()));
    let info = PythonWorkspaceDiscoverer.discover(tmp.path());
    assert!(info.roots.is_empty());
    assert!(info.packages.is_empty());
}

#[test]
fn uv_workspace_single_glob_discovers_members_and_root() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"root-app\"\n\n[tool.uv.workspace]\nmembers = [\"packages/*\"]\n",
    );
    make_dir(tmp.path(), "src/root_app");
    write_python_project(tmp.path(), "packages/alpha", "alpha-lib");
    make_dir(tmp.path(), "packages/alpha/src/alpha_lib");
    write_python_project(tmp.path(), "packages/beta", "beta.lib");
    make_dir(tmp.path(), "packages/beta/beta_lib");

    let info = discover(tmp.path());
    let mut expected_roots = vec![
        tmp.path().to_path_buf(),
        tmp.path().join("packages/alpha"),
        tmp.path().join("packages/beta"),
    ];
    expected_roots.sort();

    assert_eq!(info.roots, expected_roots);
    assert_eq!(info.packages.get("root_app").unwrap(), tmp.path());
    assert_eq!(
        info.packages.get("alpha_lib").unwrap(),
        &tmp.path().join("packages/alpha")
    );
    assert_eq!(
        info.packages.get("beta_lib").unwrap(),
        &tmp.path().join("packages/beta")
    );
}

#[test]
fn uv_workspace_exclude_filters_members() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[tool.uv.workspace]\nmembers = [\"packages/*\"]\nexclude = [\"packages/internal\"]\n",
    );
    write_python_project(tmp.path(), "packages/core", "core");
    make_dir(tmp.path(), "packages/core/core");
    write_python_project(tmp.path(), "packages/internal", "internal");
    make_dir(tmp.path(), "packages/internal/internal");

    let info = discover(tmp.path());

    assert!(info.roots.contains(&tmp.path().to_path_buf()));
    assert!(info.roots.contains(&tmp.path().join("packages/core")));
    assert!(!info.roots.contains(&tmp.path().join("packages/internal")));
    assert!(info.packages.contains_key("core"));
    assert!(!info.packages.contains_key("internal"));
}

#[test]
fn python_import_name_normalizes_project_name() {
    let tmp = TempDir::new().unwrap();
    write_python_project(tmp.path(), ".", "My--Package.Name");
    make_dir(tmp.path(), "src/my_package_name");

    assert_eq!(
        read_python_import_name(tmp.path()),
        Some("my_package_name".into())
    );
}

#[test]
fn python_import_name_detects_src_layout() {
    let tmp = TempDir::new().unwrap();
    write_python_project(tmp.path(), ".", "my-package");
    make_dir(tmp.path(), "src/my_package");

    assert_eq!(
        read_python_import_name(tmp.path()),
        Some("my_package".into())
    );
}

#[test]
fn python_import_name_detects_flat_layout() {
    let tmp = TempDir::new().unwrap();
    write_python_project(tmp.path(), ".", "my-package");
    make_dir(tmp.path(), "my_package");

    assert_eq!(
        read_python_import_name(tmp.path()),
        Some("my_package".into())
    );
}

#[test]
fn go_discoverer_detects_go_work() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "go.work", "go 1.23.0\n");
    assert!(GoWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn go_discoverer_detects_go_mod() {
    let tmp = TempDir::new().unwrap();
    write_go_module(tmp.path(), ".", "github.com/acme/root");
    assert!(GoWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn go_discoverer_does_not_detect_empty_dir() {
    let tmp = TempDir::new().unwrap();
    assert!(!GoWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn go_work_single_use_discovers_module() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "go.work", "go 1.23.0\nuse ./module-a\n");
    write_go_module(tmp.path(), "module-a", "github.com/acme/module-a");

    let info = discover(tmp.path());

    assert_eq!(info.roots, vec![tmp.path().join("module-a")]);
    assert_eq!(
        info.packages.get("github.com/acme/module-a").unwrap(),
        &tmp.path().join("module-a")
    );
}

#[test]
fn go_work_parenthesized_use_block_discovers_modules() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.work",
        "go 1.23.0\n\
         toolchain go1.23.1\n\
         use (\n\
             ./module-a // local module\n\
             \"./module-b\"\n\
         )\n\
         replace example.com/old => example.com/new v1.0.0\n",
    );
    write_go_module(tmp.path(), "module-a", "github.com/acme/module-a");
    write_go_module(tmp.path(), "module-b", "github.com/acme/module-b");

    let info = discover(tmp.path());

    assert_eq!(info.roots.len(), 2);
    assert!(info.packages.contains_key("github.com/acme/module-a"));
    assert!(info.packages.contains_key("github.com/acme/module-b"));
}

#[test]
fn go_work_quoted_use_path_with_spaces_discovers_module() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "go.work", "go 1.23.0\nuse \"./module a\"\n");
    write_go_module(tmp.path(), "module a", "github.com/acme/module-a");

    let info = discover(tmp.path());

    assert_eq!(info.roots, vec![tmp.path().join("module a")]);
    assert_eq!(
        info.packages.get("github.com/acme/module-a").unwrap(),
        &tmp.path().join("module a")
    );
}

#[test]
fn go_work_quoted_path_preserves_comment_like_text() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "go.work", "go 1.23.0\nuse \"./module//a\"\n");
    write_go_module(tmp.path(), "module/a", "github.com/acme/module-a");

    let info = discover(tmp.path());

    assert_eq!(info.roots, vec![tmp.path().join("module//a")]);
    assert!(info.packages.contains_key("github.com/acme/module-a"));
}

#[test]
fn go_module_path_read_from_go_mod() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.mod",
        "// module comment\nmodule github.com/acme/root // trailing comment\n",
    );

    assert_eq!(
        read_go_module_path(tmp.path()),
        Some("github.com/acme/root".into())
    );
}

#[test]
fn single_go_mod_returns_single_member() {
    let tmp = TempDir::new().unwrap();
    write_go_module(tmp.path(), ".", "github.com/acme/root");

    let info = discover(tmp.path());

    assert_eq!(info.roots, vec![tmp.path().to_path_buf()]);
    assert_eq!(
        info.packages.get("github.com/acme/root").unwrap(),
        tmp.path()
    );
}

#[test]
fn default_discover_merges_js_and_cargo_workspaces() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"workspaces":["packages/*"]}"#,
    );
    make_dir(tmp.path(), "packages/web");
    write_file(tmp.path(), "packages/web/package.json", r#"{"name":"web"}"#);
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_crate(tmp.path(), "crates/fmm-core", "fmm-core");
    write_file(tmp.path(), "go.work", "go 1.23.0\nuse ./services/api\n");
    write_go_module(tmp.path(), "services/api", "github.com/acme/api");

    let info = discover(tmp.path());

    assert_eq!(info.roots.len(), 3);
    assert!(info.packages.contains_key("web"));
    assert!(info.packages.contains_key("fmm_core"));
    assert!(info.packages.contains_key("github.com/acme/api"));
}
