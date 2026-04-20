use super::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
    let path = base.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, content).unwrap();
    path
}

fn package_map(tmp: &TempDir) -> HashMap<String, PathBuf> {
    HashMap::from([("app".to_string(), tmp.path().join("crates/app"))])
}

#[test]
fn super_from_top_level_module_resolves_custom_lib_root() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
version = "0.1.0"

[lib]
path = "src/custom_root.rs"
"#,
    );
    let root = write_file(tmp.path(), "crates/app/src/custom_root.rs", "");
    let importer = write_file(tmp.path(), "crates/app/src/foo.rs", "");
    let resolver = RustImportResolver::new(&package_map(&tmp));

    assert_eq!(resolver.resolve(&importer, "super::RootType"), Some(root));
}

#[test]
fn super_from_top_level_module_resolves_main_root() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
version = "0.1.0"
"#,
    );
    let root = write_file(tmp.path(), "crates/app/src/main.rs", "");
    let importer = write_file(tmp.path(), "crates/app/src/foo.rs", "");
    let resolver = RustImportResolver::new(&package_map(&tmp));

    assert_eq!(resolver.resolve(&importer, "super::RootType"), Some(root));
}
