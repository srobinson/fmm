use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::{dependency_graph, dependency_graph_transitive};
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

#[test]
fn rust_same_crate_reexport_module_is_local_not_external() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "crates/rtm-core/Cargo.toml",
        r#"
[package]
name = "rtm-core"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(
        tmp.path(),
        "crates/rtm-core/src/proto.rs",
        "pub struct RuntimeRpc;",
    );
    write_file(
        tmp.path(),
        "crates/rtm-core/src/types/lifecycle.rs",
        "pub struct Lifecycle;",
    );
    write_file(
        tmp.path(),
        "crates/rtm-core/src/types.rs",
        "pub mod lifecycle; pub use lifecycle::Lifecycle;",
    );
    write_file(
        tmp.path(),
        "crates/rtm-core/src/lib.rs",
        "pub mod proto; pub mod types; pub use proto::RuntimeRpc; pub use types::Lifecycle;",
    );

    let proto_key = "crates/rtm-core/src/proto.rs".to_string();
    let lifecycle_key = "crates/rtm-core/src/types/lifecycle.rs".to_string();
    let types_key = "crates/rtm-core/src/types.rs".to_string();
    let lib_key = "crates/rtm-core/src/lib.rs".to_string();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("rtm_core".into(), tmp.path().join("crates/rtm-core"));
    manifest.add_file(&proto_key, Metadata::default());
    manifest.add_file(&lifecycle_key, Metadata::default());
    manifest.add_file(
        &types_key,
        Metadata {
            imports: vec!["lifecycle".to_string()],
            named_imports: HashMap::from([(
                "lifecycle".to_string(),
                vec!["Lifecycle".to_string()],
            )]),
            ..Default::default()
        },
    );
    manifest.add_file(
        &lib_key,
        Metadata {
            imports: vec!["proto".to_string(), "types".to_string()],
            named_imports: HashMap::from([
                ("proto".to_string(), vec!["RuntimeRpc".to_string()]),
                ("types".to_string(), vec!["Lifecycle".to_string()]),
            ]),
            ..Default::default()
        },
    );

    let entry = &manifest.files[&lib_key];
    let (local, external, _) = dependency_graph(&manifest, &lib_key, entry);
    let (upstream, transitive_external, _) =
        dependency_graph_transitive(&manifest, &lib_key, entry, -1);

    assert!(local.contains(&proto_key), "{local:?}");
    assert!(local.contains(&types_key), "{local:?}");
    assert!(!external.contains(&"proto".to_string()), "{external:?}");
    assert!(!external.contains(&"types".to_string()), "{external:?}");
    assert!(
        upstream.iter().any(|(path, _)| path == &proto_key),
        "{upstream:?}"
    );
    assert!(
        upstream.iter().any(|(path, _)| path == &lifecycle_key),
        "{upstream:?}"
    );
    assert!(
        !transitive_external.contains(&"proto".to_string()),
        "{transitive_external:?}"
    );
    assert!(
        !transitive_external.contains(&"lifecycle".to_string()),
        "{transitive_external:?}"
    );
}
