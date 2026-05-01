use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::dependency_graph;
use crate::manifest::Manifest;
use crate::parser::Metadata;
use crate::resolver::workspace::WorkspaceEcosystem;

fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
    let path = base.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn js_workspace_import_resolved_through_graph_index_is_not_external() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(tmp.path(), "packages/shared/foo.ts", "");
    write_file(tmp.path(), "apps/web/index.ts", "");

    let target = "packages/shared/foo.ts".to_string();
    let importer = "apps/web/index.ts".to_string();
    let package_root = tmp.path().join("packages/shared");

    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("shared".into(), package_root.clone());
    manifest.workspace_packages_by_ecosystem.insert(
        WorkspaceEcosystem::Js,
        HashMap::from([("shared".to_string(), package_root)]),
    );
    manifest.add_file(&target, Metadata::default());
    manifest.add_file(
        &importer,
        Metadata {
            imports: vec!["shared/foo".to_string()],
            ..Default::default()
        },
    );

    let entry = &manifest.files[&importer];
    let (local, external, _) = dependency_graph(&manifest, &importer, entry);

    assert_eq!(local, vec![target]);
    assert!(
        !external.contains(&"shared/foo".to_string()),
        "{external:?}"
    );
}
