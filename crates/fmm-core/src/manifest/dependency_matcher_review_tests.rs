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

#[test]
fn rust_reexport_chain_resolves_same_crate_bare_module() {
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
        "crates/rtm-cli/Cargo.toml",
        r#"
[package]
name = "rtm-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
rtm-core = { path = "../rtm-core" }
"#,
    );
    let core_root = tmp.path().join("crates/rtm-core");
    let cli_root = tmp.path().join("crates/rtm-cli");
    let proto = write_file(
        tmp.path(),
        "crates/rtm-core/src/proto.rs",
        "pub struct RuntimeRpc;",
    );
    let lifecycle = write_file(
        tmp.path(),
        "crates/rtm-core/src/types/lifecycle.rs",
        "pub struct Lifecycle;",
    );
    let types = write_file(
        tmp.path(),
        "crates/rtm-core/src/types.rs",
        "pub mod lifecycle; pub use lifecycle::Lifecycle;",
    );
    let core_lib = write_file(
        tmp.path(),
        "crates/rtm-core/src/lib.rs",
        "pub mod proto; pub mod types; pub use proto::RuntimeRpc; pub use types::Lifecycle;",
    );
    let cli_main = write_file(
        tmp.path(),
        "crates/rtm-cli/src/main.rs",
        "use rtm_core::RuntimeRpc;",
    );

    let proto_key = proto.to_string_lossy().into_owned();
    let lifecycle_key = lifecycle.to_string_lossy().into_owned();
    let types_key = types.to_string_lossy().into_owned();
    let core_lib_key = core_lib.to_string_lossy().into_owned();
    let cli_main_key = cli_main.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_packages = HashMap::from([
        ("rtm_core".to_string(), core_root),
        ("rtm_cli".to_string(), cli_root),
    ]);
    manifest
        .files
        .insert(proto_key.clone(), crate::manifest::FileEntry::default());
    manifest
        .files
        .insert(lifecycle_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        types_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["lifecycle".to_string()],
            named_imports: HashMap::from([(
                "lifecycle".to_string(),
                vec!["Lifecycle".to_string()],
            )]),
            ..Default::default()
        },
    );
    manifest.files.insert(
        core_lib_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["proto".to_string(), "types".to_string()],
            named_imports: HashMap::from([
                ("proto".to_string(), vec!["RuntimeRpc".to_string()]),
                ("types".to_string(), vec!["Lifecycle".to_string()]),
            ]),
            ..Default::default()
        },
    );
    manifest.files.insert(
        cli_main_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["rtm_core".to_string()],
            named_imports: HashMap::from([(
                "rtm_core".to_string(),
                vec!["RuntimeRpc".to_string()],
            )]),
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let proto_importers = reverse_deps.get(&proto_key).cloned().unwrap_or_default();
    let lifecycle_importers = reverse_deps
        .get(&lifecycle_key)
        .cloned()
        .unwrap_or_default();
    let types_importers = reverse_deps.get(&types_key).cloned().unwrap_or_default();
    let lib_importers = reverse_deps.get(&core_lib_key).cloned().unwrap_or_default();

    assert!(
        proto_importers.contains(&core_lib_key),
        "{proto_importers:?}"
    );
    assert!(
        lifecycle_importers.contains(&types_key),
        "{lifecycle_importers:?}"
    );
    assert!(
        types_importers.contains(&core_lib_key),
        "{types_importers:?}"
    );
    assert!(lib_importers.contains(&cli_main_key), "{lib_importers:?}");
}
