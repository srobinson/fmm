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

fn load_manifest(root: &Path) -> fmm::manifest::Manifest {
    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true)
        .expect("generate failed");
    fmm::manifest::Manifest::load(root).unwrap_or_default()
}

/// Check whether `importer` is in `reverse_deps[target]`.
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
    write_file(root, "packages/shared/utils.ts", "export const x = 1;\n");

    // packages/app — imports from 'shared/utils'
    write_file(root, "packages/app/package.json", r#"{"name": "app"}"#);
    write_file(
        root,
        "packages/app/index.ts",
        "import { x } from 'shared/utils';\n",
    );

    let manifest = load_manifest(root);

    assert!(
        has_reverse_dep(
            &manifest,
            "packages/shared/utils.ts",
            "packages/app/index.ts"
        ),
        "expected packages/app/index.ts in reverse_deps[packages/shared/utils.ts], got: {:?}",
        manifest.reverse_deps.get("packages/shared/utils.ts")
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
    write_file(
        root,
        "packages/shared/ReactFeatureFlags.js",
        "// feature flags\n",
    );

    // The importer — uses moduleDirectories-style import 'shared/ReactFeatureFlags'
    write_file(
        root,
        "packages/react-reconciler/src/ReactFiberWorkLoop.js",
        "import something from 'shared/ReactFeatureFlags';\n",
    );

    let manifest = load_manifest(root);

    assert!(
        has_reverse_dep(
            &manifest,
            "packages/shared/ReactFeatureFlags.js",
            "packages/react-reconciler/src/ReactFiberWorkLoop.js"
        ),
        "Layer 3 heuristic failed: expected packages/react-reconciler/src/ReactFiberWorkLoop.js in reverse_deps[packages/shared/ReactFeatureFlags.js], got: {:?}",
        manifest.reverse_deps.get("packages/shared/ReactFeatureFlags.js")
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
    write_file(root, "packages/lib/index.ts", "export const lib = 1;\n");

    write_file(
        root,
        "packages/consumer/package.json",
        r#"{"name": "@myorg/consumer", "dependencies": {"@myorg/lib": "workspace:*"}}"#,
    );
    write_file(
        root,
        "packages/consumer/main.ts",
        "import { lib } from '@myorg/lib';\n",
    );

    let manifest = load_manifest(root);

    assert!(
        has_reverse_dep(&manifest, "packages/lib/index.ts", "packages/consumer/main.ts"),
        "pnpm scoped package not resolved: expected packages/consumer/main.ts in reverse_deps[packages/lib/index.ts], got: {:?}",
        manifest.reverse_deps.get("packages/lib/index.ts")
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
    write_file(root, "packages/react/index.js", "// local react\n");

    // App imports 'lodash' — NOT in manifest, must not appear as reverse dep
    write_file(
        root,
        "packages/app/index.ts",
        "import lodash from 'lodash';\n",
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
    write_file(
        root,
        "src/a.ts",
        "import something from 'some-external-package';\nexport const a = 1;\n",
    );
    write_file(root, "src/b.ts", "import { a } from './a';\n");

    // Must not panic
    let manifest = load_manifest(root);

    // Relative dep still works
    assert!(
        has_reverse_dep(&manifest, "src/a.ts", "src/b.ts"),
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

    let manifest = fmm::manifest::Manifest::load(&root)
        .expect("failed to load React manifest — run fmm generate first");

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
