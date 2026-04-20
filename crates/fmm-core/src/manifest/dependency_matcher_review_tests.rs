use super::*;
use crate::resolver::workspace::WorkspaceEcosystem;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn js_workspace_import_allows_dotted_bare_package_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let package_root = tmp.path().join("packages/lodash.merge");
    let target = write_file(tmp.path(), "packages/lodash.merge/index.ts", "");
    let importer = write_file(tmp.path(), "apps/web/index.ts", "");

    let target_key = target.to_string_lossy().into_owned();
    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("lodash.merge".into(), package_root);
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["lodash.merge".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(importers.contains(&importer_key), "{importers:?}");
}

#[test]
fn js_directory_prefix_fallback_uses_only_js_roots() {
    let tmp = tempfile::TempDir::new().unwrap();
    let rust_shared = tmp.path().join("crates/shared");
    let js_app = tmp.path().join("packages/app");
    let target = write_file(tmp.path(), "crates/shared/foo.js", "");
    let importer = write_file(tmp.path(), "packages/app/index.ts", "");

    let target_key = target.to_string_lossy().into_owned();
    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_roots = vec![rust_shared.clone(), js_app.clone()];
    manifest
        .workspace_roots_by_ecosystem
        .insert(WorkspaceEcosystem::Rust, vec![rust_shared.clone()]);
    manifest
        .workspace_roots_by_ecosystem
        .insert(WorkspaceEcosystem::Js, vec![js_app]);
    manifest.workspace_packages_by_ecosystem.insert(
        WorkspaceEcosystem::Rust,
        HashMap::from([("shared".to_string(), rust_shared)]),
    );
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["shared/foo".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(!importers.contains(&importer_key), "{importers:?}");
}
