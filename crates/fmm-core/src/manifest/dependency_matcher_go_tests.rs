use super::*;
use std::path::{Path, PathBuf};

fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn build_reverse_deps_resolves_go_cross_module_workspace_import() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "services/shared/go.mod",
        "module github.com/acme/shared\n\ngo 1.23.0\n",
    );
    let target = write_file(
        tmp.path(),
        "services/shared/config/config.go",
        "package config\n",
    );
    let sibling = write_file(
        tmp.path(),
        "services/shared/config/options.go",
        "package config\n",
    );
    let test_file = write_file(
        tmp.path(),
        "services/shared/config/config_test.go",
        "package config\n",
    );
    write_file(
        tmp.path(),
        "services/api/go.mod",
        "module github.com/acme/api\n\ngo 1.23.0\n",
    );
    let importer = write_file(
        tmp.path(),
        "services/api/handler/handler.go",
        "package handler\n",
    );

    let target_key = target.to_string_lossy().into_owned();
    let sibling_key = sibling.to_string_lossy().into_owned();
    let test_key = test_file.to_string_lossy().into_owned();
    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_packages.insert(
        "github.com/acme/shared".into(),
        tmp.path().join("services/shared"),
    );
    manifest.workspace_packages.insert(
        "github.com/acme/api".into(),
        tmp.path().join("services/api"),
    );
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest
        .files
        .insert(sibling_key.clone(), crate::manifest::FileEntry::default());
    manifest
        .files
        .insert(test_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["github.com/acme/shared/config".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let target_importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();
    let sibling_importers = reverse_deps.get(&sibling_key).cloned().unwrap_or_default();
    let test_importers = reverse_deps.get(&test_key).cloned().unwrap_or_default();

    assert!(
        target_importers.contains(&importer_key),
        "Go package import should resolve to indexed package file, got: {:?}",
        target_importers
    );
    assert!(
        sibling_importers.contains(&importer_key),
        "Go package import should apply to every non-test file in the package, got: {:?}",
        sibling_importers
    );
    assert!(
        !test_importers.contains(&importer_key),
        "Go package import should not target _test.go files, got: {:?}",
        test_importers
    );
}

#[test]
fn build_reverse_deps_ignores_go_standard_library_and_external_imports() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "services/api/go.mod",
        "module github.com/acme/api\n\ngo 1.23.0\n",
    );
    let importer = write_file(
        tmp.path(),
        "services/api/handler/handler.go",
        "package handler\n",
    );

    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_packages.insert(
        "github.com/acme/api".into(),
        tmp.path().join("services/api"),
    );
    manifest.files.insert(
        importer_key,
        crate::manifest::FileEntry {
            imports: vec![
                "fmt".to_string(),
                "net/http".to_string(),
                "golang.org/x/net/context".to_string(),
            ],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);

    assert!(
        reverse_deps.is_empty(),
        "Go stdlib and external imports should not produce reverse deps, got: {:?}",
        reverse_deps
    );
}
