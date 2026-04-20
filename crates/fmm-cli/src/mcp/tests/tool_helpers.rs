use super::super::tools::{compute_import_specifiers, glob_filename_matches, is_reexport_file};
use std::path::Path;

#[test]
fn is_reexport_file_detects_index_files() {
    for path in [
        "agno/__init__.py",
        "src/index.ts",
        "src/index.tsx",
        "src/mod.rs",
        "libs/foo/index.js",
    ] {
        assert!(is_reexport_file(path), "{path} should be a reexport file");
    }

    for path in ["agno/agent/agent.py", "src/auth.ts"] {
        assert!(
            !is_reexport_file(path),
            "{path} should not be a reexport file"
        );
    }
}

#[test]
fn glob_filename_matcher_handles_literals_and_stars() {
    let cases = [
        ("*.py", "agent.py", true),
        ("*.rs", "mod.rs", true),
        ("*.py", "agent.rs", false),
        ("*.py", "agent.pyc", false),
        ("test_*", "test_agent.py", true),
        ("test_*", "test_.py", true),
        ("test_*", "mytest_agent.py", false),
        ("mod.rs", "mod.rs", true),
        ("mod.rs", "mod.ts", false),
        ("*", "anything.py", true),
        ("*", "", true),
    ];

    for (pattern, filename, expected) in cases {
        assert_eq!(
            glob_filename_matches(pattern, filename),
            expected,
            "pattern {pattern:?} against {filename:?}",
        );
    }
}

#[test]
fn compute_import_specifiers_handles_relative_forms() {
    let same_dir = compute_import_specifiers(
        "src/ReactFiberHooks.js",
        "src/ReactFiberWorkLoop.js",
        &[],
        Path::new(""),
    );
    assert!(same_dir.contains(&"./ReactFiberWorkLoop".to_string()));
    assert!(same_dir.contains(&"./ReactFiberWorkLoop.js".to_string()));

    let cross_dir = compute_import_specifiers(
        "packages/react-dom/src/ReactDOMRenderer.js",
        "packages/react-reconciler/src/ReactFiberWorkLoop.js",
        &[],
        Path::new(""),
    );
    assert!(cross_dir.contains(&"../../react-reconciler/src/ReactFiberWorkLoop".to_string()));
    assert!(cross_dir.contains(&"../../react-reconciler/src/ReactFiberWorkLoop.js".to_string()));

    let from_root = compute_import_specifiers("index.js", "src/utils.js", &[], Path::new(""));
    assert!(from_root.contains(&"./src/utils".to_string()));
    assert!(from_root.contains(&"./src/utils.js".to_string()));

    let into_child =
        compute_import_specifiers("src/a/file.ts", "src/a/deep/module.ts", &[], Path::new(""));
    assert!(into_child.contains(&"./deep/module".to_string()));
    assert!(into_child.contains(&"./deep/module.ts".to_string()));
}

#[test]
fn compute_import_specifiers_handles_no_extension_source() {
    let specs = compute_import_specifiers("src/foo.js", "src/bar", &[], Path::new(""));
    assert_eq!(specs, vec!["./bar".to_string()]);
}

#[test]
fn compute_import_specifiers_adds_workspace_bare_specifiers() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path();
    let shared_root = project_root.join("packages").join("shared");

    let specs = compute_import_specifiers(
        "packages/react-reconciler/src/ReactFiberWorkLoop.js",
        "packages/shared/ReactFeatureFlags.js",
        &[shared_root],
        project_root,
    );

    assert!(specs.contains(&"../../shared/ReactFeatureFlags".to_string()));
    assert!(specs.contains(&"shared/ReactFeatureFlags".to_string()));
    assert!(specs.contains(&"shared/ReactFeatureFlags.js".to_string()));
}

#[test]
fn compute_import_specifiers_omits_workspace_bare_specifiers_without_roots() {
    let specs = compute_import_specifiers(
        "packages/react-reconciler/src/ReactFiberWorkLoop.js",
        "packages/shared/ReactFeatureFlags.js",
        &[],
        Path::new(""),
    );

    assert!(
        !specs.iter().any(|s| s == "shared/ReactFeatureFlags"),
        "no bare specifier expected without workspace_roots, got {specs:?}",
    );
}
