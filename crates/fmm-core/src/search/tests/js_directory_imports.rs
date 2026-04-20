use super::super::dependency_graph;
use super::support::manifest_with;

#[test]
fn js_index_ts_resolves_for_directory_import() {
    let manifest = manifest_with(vec![
        ("src/auth/module/index.ts", vec![]),
        ("src/auth/session.ts", vec!["./module"]),
    ]);
    let entry = manifest.files["src/auth/session.ts"].clone();
    let (local, external, _) = dependency_graph(&manifest, "src/auth/session.ts", &entry);
    assert!(
        local.contains(&"src/auth/module/index.ts".to_string()),
        "./module should resolve to module/index.ts, got local: {:?}",
        local
    );
    assert!(
        external.is_empty(),
        "no external expected, got: {:?}",
        external
    );
}

#[test]
fn js_index_tsx_resolves_for_directory_import() {
    let manifest = manifest_with(vec![
        ("src/components/Button/index.tsx", vec![]),
        ("src/App.tsx", vec!["./components/Button"]),
    ]);
    let entry = manifest.files["src/App.tsx"].clone();
    let (local, external, _) = dependency_graph(&manifest, "src/App.tsx", &entry);
    assert!(
        local.contains(&"src/components/Button/index.tsx".to_string()),
        "./components/Button should resolve to index.tsx, got: {:?}",
        local
    );
    assert!(
        external.is_empty(),
        "no external expected, got: {:?}",
        external
    );
}

#[test]
fn js_index_js_resolves_for_directory_import() {
    let manifest = manifest_with(vec![
        ("src/utils/index.js", vec![]),
        ("src/app.js", vec!["./utils"]),
    ]);
    let entry = manifest.files["src/app.js"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/app.js", &entry);
    assert!(
        local.contains(&"src/utils/index.js".to_string()),
        "./utils should resolve to utils/index.js, got: {:?}",
        local
    );
}

#[test]
fn js_index_jsx_resolves_for_directory_import() {
    let manifest = manifest_with(vec![
        ("src/components/Form/index.jsx", vec![]),
        ("src/Page.jsx", vec!["./components/Form"]),
    ]);
    let entry = manifest.files["src/Page.jsx"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/Page.jsx", &entry);
    assert!(
        local.contains(&"src/components/Form/index.jsx".to_string()),
        "./components/Form should resolve to index.jsx, got: {:?}",
        local
    );
}

#[test]
fn js_direct_file_takes_priority_over_index() {
    let manifest = manifest_with(vec![
        ("src/auth/module.ts", vec![]),
        ("src/auth/module/index.ts", vec![]),
        ("src/auth/session.ts", vec!["./module"]),
    ]);
    let entry = manifest.files["src/auth/session.ts"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/auth/session.ts", &entry);
    assert!(
        local.contains(&"src/auth/module.ts".to_string()),
        "direct file should take priority over index.ts, got: {:?}",
        local
    );
    let count = local.iter().filter(|f| f.contains("module")).count();
    assert_eq!(
        count, 1,
        "should resolve to exactly one file, got: {:?}",
        local
    );
}

#[test]
fn js_parent_relative_resolves_index_file() {
    let manifest = manifest_with(vec![
        ("src/errors/index.ts", vec![]),
        ("src/auth/session.ts", vec!["../errors"]),
    ]);
    let entry = manifest.files["src/auth/session.ts"].clone();
    let (local, external, _) = dependency_graph(&manifest, "src/auth/session.ts", &entry);
    assert!(
        local.contains(&"src/errors/index.ts".to_string()),
        "../errors should resolve to errors/index.ts, got: {:?}",
        local
    );
    assert!(
        external.is_empty(),
        "no external expected, got: {:?}",
        external
    );
}

#[test]
fn js_deep_nesting_index_resolution() {
    let manifest = manifest_with(vec![
        ("shared/utils/index.ts", vec![]),
        ("src/app.ts", vec!["../../shared/utils"]),
    ]);
    let entry = manifest.files["src/app.ts"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/app.ts", &entry);
    assert!(
        local.contains(&"shared/utils/index.ts".to_string()),
        "../../shared/utils (from src/) should resolve to shared/utils/index.ts, got: {:?}",
        local
    );
}

#[test]
fn js_index_resolution_does_not_match_wrong_directory() {
    let manifest = manifest_with(vec![
        ("src/authentication/index.ts", vec![]),
        ("src/app.ts", vec!["./auth"]),
    ]);
    let entry = manifest.files["src/app.ts"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/app.ts", &entry);
    assert!(
        !local.contains(&"src/authentication/index.ts".to_string()),
        "./auth must not resolve to authentication/index.ts"
    );
}

#[test]
fn js_directory_import_downstream_detection() {
    let manifest = manifest_with(vec![
        ("src/auth/module/index.ts", vec![]),
        ("src/auth/session.ts", vec!["./module"]),
    ]);
    let entry = manifest.files["src/auth/module/index.ts"].clone();
    let (_, _, downstream) = dependency_graph(&manifest, "src/auth/module/index.ts", &entry);
    assert!(
        downstream.contains(&&"src/auth/session.ts".to_string()),
        "session.ts should appear as downstream of module/index.ts, got: {:?}",
        downstream
    );
}
