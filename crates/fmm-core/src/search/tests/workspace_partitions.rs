use std::collections::HashMap;
use std::path::PathBuf;

use super::super::dependency_graph;
use crate::manifest::{FileEntry, Manifest};
use crate::resolver::workspace::WorkspaceEcosystem;

#[test]
fn js_workspace_import_resolved_by_reverse_deps_is_not_external() {
    let target = "packages/shared/foo.ts".to_string();
    let importer = "apps/web/index.ts".to_string();
    let entry = FileEntry {
        imports: vec!["shared/foo".to_string()],
        ..Default::default()
    };

    let mut manifest = Manifest::new();
    manifest.workspace_packages_by_ecosystem.insert(
        WorkspaceEcosystem::Js,
        HashMap::from([("shared".to_string(), PathBuf::from("packages/shared"))]),
    );
    manifest.files.insert(target.clone(), FileEntry::default());
    manifest.files.insert(importer.clone(), entry.clone());
    manifest
        .reverse_deps
        .insert(target.clone(), vec![importer.clone()]);

    let (local, external, _) = dependency_graph(&manifest, &importer, &entry);

    assert_eq!(local, vec![target]);
    assert!(
        !external.contains(&"shared/foo".to_string()),
        "{external:?}"
    );
}
