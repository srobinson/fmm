use super::super::{SearchFilters, filter_search};
use super::support::{manifest_with, manifest_with_imports};

#[test]
fn depends_on_with_extension_equals_without() {
    let manifest = manifest_with(vec![
        ("src/db/schema.ts", vec![]),
        ("src/routes/users.ts", vec!["../db/schema"]),
        ("src/routes/posts.ts", vec!["../db/schema.ts"]),
        ("src/services/auth.ts", vec!["../db/schema"]),
    ]);

    let filters_with_ext = SearchFilters {
        export: None,
        imports: None,
        depends_on: Some("src/db/schema.ts".to_string()),
        min_loc: None,
        max_loc: None,
    };
    let filters_without_ext = SearchFilters {
        export: None,
        imports: None,
        depends_on: Some("src/db/schema".to_string()),
        min_loc: None,
        max_loc: None,
    };

    let results_with = filter_search(&manifest, &filters_with_ext);
    let results_without = filter_search(&manifest, &filters_without_ext);

    let files_with: Vec<&str> = results_with.iter().map(|r| r.file.as_str()).collect();
    let files_without: Vec<&str> = results_without.iter().map(|r| r.file.as_str()).collect();

    assert_eq!(
        results_with.len(),
        results_without.len(),
        "extension vs no-extension should return same count; with: {:?}, without: {:?}",
        files_with,
        files_without
    );

    for file in &files_with {
        assert!(
            files_without.contains(file),
            "file {:?} in with-ext results but not in without-ext; without: {:?}",
            file,
            files_without
        );
    }

    assert!(
        files_with.contains(&"src/routes/users.ts"),
        "users.ts should match; got: {:?}",
        files_with
    );
    assert!(
        files_with.contains(&"src/routes/posts.ts"),
        "posts.ts should match; got: {:?}",
        files_with
    );
    assert!(
        files_with.contains(&"src/services/auth.ts"),
        "auth.ts should match; got: {:?}",
        files_with
    );
}

#[test]
fn depends_on_cycle_does_not_return_target_file() {
    let manifest = manifest_with(vec![
        ("src/a.ts", vec!["./b"]),
        ("src/b.ts", vec!["./a"]),
        ("src/c.ts", vec!["./b"]),
    ]);

    let results = filter_search(
        &manifest,
        &SearchFilters {
            export: None,
            imports: None,
            depends_on: Some("src/a.ts".to_string()),
            min_loc: None,
            max_loc: None,
        },
    );
    let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();

    assert!(
        files.contains(&"src/b.ts"),
        "direct dependent should match; got: {:?}",
        files
    );
    assert!(
        files.contains(&"src/c.ts"),
        "transitive dependent should match; got: {:?}",
        files
    );
    assert!(
        !files.contains(&"src/a.ts"),
        "target file should not be its own dependent; got: {:?}",
        files
    );
}

#[test]
fn depends_on_uses_graph_index_for_rust_cross_crate_edges() {
    use crate::manifest::Manifest;
    use crate::parser::Metadata;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
        let path = base.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.dependencies]
fmm-core = { path = "crates/fmm-core" }
"#,
    );
    write_file(
        tmp.path(),
        "crates/fmm-core/Cargo.toml",
        r#"
[package]
name = "fmm-core"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(
        tmp.path(),
        "crates/fmm-core/src/store.rs",
        "pub trait FmmStore {}",
    );
    write_file(tmp.path(), "crates/fmm-core/src/lib.rs", "pub mod store;");
    write_file(
        tmp.path(),
        "crates/fmm-cli/Cargo.toml",
        r#"
[package]
name = "fmm"
version = "0.1.0"
edition = "2024"

[dependencies]
fmm-core.workspace = true
"#,
    );
    write_file(
        tmp.path(),
        "crates/fmm-cli/src/main.rs",
        "use fmm_core::store::FmmStore;",
    );

    let target_key = "crates/fmm-core/src/store.rs".to_string();
    let importer_key = "crates/fmm-cli/src/main.rs".to_string();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("fmm_core".into(), tmp.path().join("crates/fmm-core"));
    manifest
        .workspace_packages
        .insert("fmm".into(), tmp.path().join("crates/fmm-cli"));
    manifest.add_file(&target_key, Metadata::default());
    manifest.add_file(
        &importer_key,
        Metadata {
            named_imports: HashMap::from([(
                "fmm_core::store".to_string(),
                vec!["FmmStore".to_string()],
            )]),
            ..Default::default()
        },
    );

    let results = filter_search(
        &manifest,
        &SearchFilters {
            export: None,
            imports: None,
            depends_on: Some(target_key),
            min_loc: None,
            max_loc: None,
        },
    );
    let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();

    assert!(
        files.contains(&importer_key.as_str()),
        "Rust reverse edge should be searchable through depends_on; got: {:?}",
        files
    );
}

#[test]
fn imports_filter_local_path_checks_dependencies() {
    let manifest = manifest_with(vec![
        ("src/db/client.ts", vec![]),
        ("src/routes/users.ts", vec!["../db/client"]),
        ("src/services/auth.ts", vec!["../db/client"]),
    ]);

    let filters = SearchFilters {
        export: None,
        imports: Some("src/db/client".to_string()),
        depends_on: None,
        min_loc: None,
        max_loc: None,
    };

    let results = filter_search(&manifest, &filters);
    let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();

    assert!(
        files.contains(&"src/routes/users.ts"),
        "users.ts should match local-path imports filter; got: {:?}",
        files
    );
    assert!(
        files.contains(&"src/services/auth.ts"),
        "auth.ts should match local-path imports filter; got: {:?}",
        files
    );
    assert!(
        !files.contains(&"src/db/client.ts"),
        "client.ts should not match; got: {:?}",
        files
    );
}

#[test]
fn imports_filter_external_package_unaffected() {
    let manifest = manifest_with_imports(vec![
        ("src/utils.ts", vec![], vec!["lodash"]),
        ("src/app.ts", vec![], vec!["lodash", "react"]),
        ("src/pure.ts", vec![], vec![]),
    ]);

    let filters = SearchFilters {
        export: None,
        imports: Some("lodash".to_string()),
        depends_on: None,
        min_loc: None,
        max_loc: None,
    };

    let results = filter_search(&manifest, &filters);
    let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();

    assert!(
        files.contains(&"src/utils.ts"),
        "utils.ts imports lodash; got: {:?}",
        files
    );
    assert!(
        files.contains(&"src/app.ts"),
        "app.ts imports lodash; got: {:?}",
        files
    );
    assert!(
        !files.contains(&"src/pure.ts"),
        "pure.ts does not import lodash; got: {:?}",
        files
    );
}
