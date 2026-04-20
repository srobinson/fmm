use super::super::dependency_graph;
use super::support::manifest_with;

#[test]
fn js_extensionless_import_resolves_ts() {
    let manifest = manifest_with(vec![
        ("src/logger/transient-logger.service.ts", vec![]),
        ("src/auth/instance-wrapper.ts", vec![]),
        ("src/auth/session.ts", vec!["./instance-wrapper"]),
    ]);
    let entry = manifest.files["src/auth/session.ts"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/auth/session.ts", &entry);
    assert!(
        local.contains(&"src/auth/instance-wrapper.ts".to_string()),
        "./instance-wrapper should resolve to instance-wrapper.ts, got: {:?}",
        local
    );
    assert!(
        !local.contains(&"src/logger/transient-logger.service.ts".to_string()),
        "should not ghost-match transient-logger.service.ts"
    );
}

#[test]
fn js_extensionless_import_resolves_tsx() {
    let manifest = manifest_with(vec![
        ("src/components/Header.tsx", vec![]),
        ("src/App.tsx", vec!["./components/Header"]),
    ]);
    let entry = manifest.files["src/App.tsx"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/App.tsx", &entry);
    assert!(
        local.contains(&"src/components/Header.tsx".to_string()),
        "./components/Header should resolve to Header.tsx, got: {:?}",
        local
    );
}

#[test]
fn js_extensionless_import_resolves_js() {
    let manifest = manifest_with(vec![
        ("lib/helpers.js", vec![]),
        ("lib/main.js", vec!["./helpers"]),
    ]);
    let entry = manifest.files["lib/main.js"].clone();
    let (local, _, _) = dependency_graph(&manifest, "lib/main.js", &entry);
    assert!(
        local.contains(&"lib/helpers.js".to_string()),
        "./helpers should resolve to helpers.js, got: {:?}",
        local
    );
}

#[test]
fn js_extensionless_import_resolves_jsx() {
    let manifest = manifest_with(vec![
        ("src/Button.jsx", vec![]),
        ("src/index.jsx", vec!["./Button"]),
    ]);
    let entry = manifest.files["src/index.jsx"].clone();
    let (local, _, _) = dependency_graph(&manifest, "src/index.jsx", &entry);
    assert!(
        local.contains(&"src/Button.jsx".to_string()),
        "./Button should resolve to Button.jsx, got: {:?}",
        local
    );
}

#[test]
fn js_parent_relative_resolves_direct_file() {
    let manifest = manifest_with(vec![
        ("src/errors/exceptions.ts", vec![]),
        ("src/auth/session.ts", vec!["../errors/exceptions"]),
    ]);
    let entry = manifest.files["src/auth/session.ts"].clone();
    let (local, external, _) = dependency_graph(&manifest, "src/auth/session.ts", &entry);
    assert!(
        local.contains(&"src/errors/exceptions.ts".to_string()),
        "../errors/exceptions should resolve to exceptions.ts, got: {:?}",
        local
    );
    assert!(
        external.is_empty(),
        "no external expected, got: {:?}",
        external
    );
}

#[test]
fn js_unresolvable_relative_stays_in_external() {
    let manifest = manifest_with(vec![("src/auth/session.ts", vec!["./nonexistent-module"])]);
    let entry = manifest.files["src/auth/session.ts"].clone();
    let (local, external, _) = dependency_graph(&manifest, "src/auth/session.ts", &entry);
    assert!(
        local.is_empty(),
        "unresolvable relative import must not produce ghost local_dep, got: {:?}",
        local
    );
    assert!(
        external.contains(&"./nonexistent-module".to_string()),
        "unresolvable relative import should appear in external, got: {:?}",
        external
    );
}
