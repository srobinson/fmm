//! Integration tests for cross-package import resolution (ALP-874/ALP-879).
//!
//! Tests Layers 1, 2, and 3 of the three-layer resolver and their integration
//! into `build_reverse_deps()`. False-positive safety is tested as rigorously
//! as correctness — external packages must never appear as local dependents.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_file(base: &Path, rel: &str, content: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

/// Write a source file and its .fmm sidecar in one call.
fn write_sidecar(base: &Path, rel_source: &str, sidecar_content: &str) {
    let p = base.join(rel_source);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "// source").unwrap();
    let sidecar = format!("{}.fmm", p.to_string_lossy());
    fs::write(sidecar, sidecar_content).unwrap();
}

fn load_manifest(root: &Path) -> fmm::manifest::Manifest {
    fmm::manifest::Manifest::load_from_sidecars(root).unwrap()
}

/// Check whether `target` is in `reverse_deps[source]`.
fn has_reverse_dep(manifest: &fmm::manifest::Manifest, target: &str, importer: &str) -> bool {
    manifest
        .reverse_deps
        .get(target)
        .map(|v| v.iter().any(|s| s == importer))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Fixture 1: Workspace package name resolution (Layer 2)
// ---------------------------------------------------------------------------

#[test]
fn layer2_workspace_package_name_resolves_to_reverse_dep() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // npm workspace config
    write_file(root, "package.json", r#"{"workspaces": ["packages/*"]}"#);

    // packages/shared — named package with an exported utility
    write_file(
        root,
        "packages/shared/package.json",
        r#"{"name": "shared"}"#,
    );
    write_sidecar(
        root,
        "packages/shared/utils.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports:\n  x: [1, 1]\nimports: []\ndependencies: []\nloc: 1\n",
            root.join("packages/shared/utils.ts").to_string_lossy()
        ),
    );

    // packages/app — imports from 'shared/utils'
    write_file(root, "packages/app/package.json", r#"{"name": "app"}"#);
    write_sidecar(
        root,
        "packages/app/index.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: [shared/utils]\ndependencies: []\nloc: 1\n",
            root.join("packages/app/index.ts").to_string_lossy()
        ),
    );

    let manifest = load_manifest(root);

    let target = root
        .join("packages/shared/utils.ts")
        .to_string_lossy()
        .to_string();
    let importer = root
        .join("packages/app/index.ts")
        .to_string_lossy()
        .to_string();

    assert!(
        has_reverse_dep(&manifest, &target, &importer),
        "expected {} in reverse_deps[{}], got: {:?}",
        importer,
        target,
        manifest.reverse_deps.get(&target)
    );
}

// ---------------------------------------------------------------------------
// Fixture 2: Directory prefix heuristic / React pattern (Layer 3)
// ---------------------------------------------------------------------------

#[test]
fn layer3_directory_prefix_resolves_unnamed_package() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // Workspace config — packages/shared has no package.json (unnamed → Layer 2 misses it)
    write_file(root, "package.json", r#"{"workspaces": ["packages/*"]}"#);

    // The target file — in unnamed package directory
    write_sidecar(
        root,
        "packages/shared/ReactFeatureFlags.js",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: []\ndependencies: []\nloc: 1\n",
            root.join("packages/shared/ReactFeatureFlags.js")
                .to_string_lossy()
        ),
    );

    // The importer — uses moduleDirectories-style import 'shared/ReactFeatureFlags'
    write_sidecar(
        root,
        "packages/react-reconciler/src/ReactFiberWorkLoop.js",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: [shared/ReactFeatureFlags]\ndependencies: []\nloc: 1\n",
            root.join("packages/react-reconciler/src/ReactFiberWorkLoop.js")
                .to_string_lossy()
        ),
    );

    let manifest = load_manifest(root);

    let target = root
        .join("packages/shared/ReactFeatureFlags.js")
        .to_string_lossy()
        .to_string();
    let importer = root
        .join("packages/react-reconciler/src/ReactFiberWorkLoop.js")
        .to_string_lossy()
        .to_string();

    assert!(
        has_reverse_dep(&manifest, &target, &importer),
        "Layer 3 heuristic failed: expected {} in reverse_deps[{}], got: {:?}",
        importer,
        target,
        manifest.reverse_deps.get(&target)
    );
}

// ---------------------------------------------------------------------------
// Fixture 3: pnpm workspace
// ---------------------------------------------------------------------------

#[test]
fn pnpm_workspace_package_resolves_scoped_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(root, "pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n");

    write_file(
        root,
        "packages/lib/package.json",
        r#"{"name": "@myorg/lib"}"#,
    );
    write_sidecar(
        root,
        "packages/lib/index.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports:\n  lib: [1, 1]\nimports: []\ndependencies: []\nloc: 1\n",
            root.join("packages/lib/index.ts").to_string_lossy()
        ),
    );

    write_file(
        root,
        "packages/consumer/package.json",
        r#"{"name": "@myorg/consumer", "dependencies": {"@myorg/lib": "workspace:*"}}"#,
    );
    write_sidecar(
        root,
        "packages/consumer/main.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: ['@myorg/lib']\ndependencies: []\nloc: 1\n",
            root.join("packages/consumer/main.ts").to_string_lossy()
        ),
    );

    let manifest = load_manifest(root);

    let target = root
        .join("packages/lib/index.ts")
        .to_string_lossy()
        .to_string();
    let importer = root
        .join("packages/consumer/main.ts")
        .to_string_lossy()
        .to_string();

    assert!(
        has_reverse_dep(&manifest, &target, &importer),
        "pnpm scoped package not resolved: expected {} in reverse_deps[{}], got: {:?}",
        importer,
        target,
        manifest.reverse_deps.get(&target)
    );
}

// ---------------------------------------------------------------------------
// Fixture 4: False-positive safety — external packages never resolved
// ---------------------------------------------------------------------------

#[test]
fn external_package_import_does_not_create_reverse_dep() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(root, "package.json", r#"{"workspaces": ["packages/*"]}"#);

    // Local react package (same name as npm react — still resolves as local)
    write_file(root, "packages/react/package.json", r#"{"name": "react"}"#);
    write_sidecar(
        root,
        "packages/react/index.js",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: []\ndependencies: []\nloc: 1\n",
            root.join("packages/react/index.js").to_string_lossy()
        ),
    );

    // App imports 'lodash' — NOT in manifest, must not appear as reverse dep
    write_sidecar(
        root,
        "packages/app/index.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: [lodash]\ndependencies: []\nloc: 1\n",
            root.join("packages/app/index.ts").to_string_lossy()
        ),
    );

    let manifest = load_manifest(root);

    // 'lodash' must not appear in any reverse_deps entry
    let lodash_anywhere = manifest
        .reverse_deps
        .iter()
        .any(|(k, _)| k.contains("lodash"));
    assert!(
        !lodash_anywhere,
        "lodash should not appear in reverse_deps: {:?}",
        manifest
            .reverse_deps
            .iter()
            .filter(|(k, _)| k.contains("lodash"))
            .collect::<HashMap<_, _>>()
    );
}

// ---------------------------------------------------------------------------
// Fixture 5: Graceful degradation — no workspace config
// ---------------------------------------------------------------------------

#[test]
fn no_workspace_config_does_not_crash_and_relative_deps_still_work() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // No package.json / pnpm-workspace.yaml — pure relative-import codebase
    write_sidecar(
        root,
        "src/a.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports:\n  a: [1, 1]\nimports: [some-external-package]\ndependencies: []\nloc: 1\n",
            root.join("src/a.ts").to_string_lossy()
        ),
    );
    write_sidecar(
        root,
        "src/b.ts",
        &format!(
            "file: {}\nfmm: v0.3\nexports: []\nimports: []\ndependencies: [./a]\nloc: 1\n",
            root.join("src/b.ts").to_string_lossy()
        ),
    );

    // Must not panic
    let manifest = load_manifest(root);

    // Relative dep still works
    let target = root.join("src/a.ts").to_string_lossy().to_string();
    let importer = root.join("src/b.ts").to_string_lossy().to_string();
    assert!(
        has_reverse_dep(&manifest, &target, &importer),
        "relative dep should still resolve without workspace config"
    );

    // External package not indexed
    let external_anywhere = manifest
        .reverse_deps
        .iter()
        .any(|(k, _)| k.contains("some-external-package"));
    assert!(
        !external_anywhere,
        "external package should not appear in reverse_deps"
    );
}

// ---------------------------------------------------------------------------
// Fixture 6: Workspace discovery unit assertions
// ---------------------------------------------------------------------------

#[test]
fn workspace_discovery_npm_finds_packages_and_roots() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "package.json",
        r#"{"workspaces": ["packages/*", "apps/*"]}"#,
    );

    // packages/
    write_file(root, "packages/alpha/package.json", r#"{"name": "alpha"}"#);
    write_file(
        root,
        "packages/beta/package.json",
        r#"{"name": "@scope/beta"}"#,
    );
    // apps/ — unnamed package
    fs::create_dir_all(root.join("apps/web")).unwrap();

    let info = fmm::resolver::workspace::discover(root);

    assert_eq!(info.roots.len(), 3, "3 workspace package dirs expected");
    assert!(info.packages.contains_key("alpha"));
    assert!(info.packages.contains_key("@scope/beta"));
    // apps/web has no package.json — in roots, but not packages
    assert_eq!(info.packages.len(), 2);

    assert_eq!(
        info.packages.get("alpha").unwrap(),
        &root.join("packages/alpha")
    );
}

#[test]
fn workspace_discovery_pnpm_takes_precedence() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // Both files present — pnpm should win
    write_file(root, "package.json", r#"{"workspaces": ["apps/*"]}"#);
    write_file(root, "pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n");
    write_file(root, "packages/lib/package.json", r#"{"name": "lib"}"#);
    write_file(root, "apps/web/package.json", r#"{"name": "web"}"#);

    let info = fmm::resolver::workspace::discover(root);
    assert!(
        info.packages.contains_key("lib"),
        "pnpm packages should be found"
    );
    assert!(
        !info.packages.contains_key("web"),
        "npm apps should be ignored when pnpm wins"
    );
}

// ---------------------------------------------------------------------------
// Performance smoke test (ignored by default — requires REACT_SRC env var)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires React source at REACT_SRC env var; run with: REACT_SRC=<path> cargo test react_shared -- --ignored"]
fn react_shared_downstream_count() {
    let root_str = std::env::var("REACT_SRC")
        .expect("REACT_SRC environment variable must be set to the React repo root");
    let root = PathBuf::from(root_str);

    let manifest = fmm::manifest::Manifest::load_from_sidecars(&root)
        .expect("failed to load React manifest — run fmm index first");

    let feature_flags = "packages/shared/ReactFeatureFlags.js";
    let downstream = manifest
        .reverse_deps
        .get(feature_flags)
        .map(|v| v.len())
        .unwrap_or(0);

    assert!(
        downstream > 10,
        "expected 10+ downstream dependents for ReactFeatureFlags, got {}",
        downstream
    );
}
