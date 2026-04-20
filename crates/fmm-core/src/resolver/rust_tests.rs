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

fn write_workspace(tmp: &TempDir) {
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.dependencies]
cm-core = { path = "crates/cm-core" }
"#,
    );
    write_file(
        tmp.path(),
        "crates/cm-core/Cargo.toml",
        r#"
[package]
name = "cm-core"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(tmp.path(), "crates/cm-core/src/lib.rs", "pub mod store;");
    write_file(
        tmp.path(),
        "crates/cm-core/src/store.rs",
        "pub struct CxStore;",
    );
    write_file(
        tmp.path(),
        "crates/cm-cli/Cargo.toml",
        r#"
[package]
name = "cm-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
cm-core.workspace = true
"#,
    );
    write_file(tmp.path(), "crates/cm-cli/src/main.rs", "fn main() {}");
}

fn workspace_packages(tmp: &TempDir) -> HashMap<String, PathBuf> {
    let mut packages = HashMap::new();
    packages.insert("cm_core".to_string(), tmp.path().join("crates/cm-core"));
    packages.insert("cm_cli".to_string(), tmp.path().join("crates/cm-cli"));
    packages
}

#[test]
fn cross_crate_path_resolves_workspace_dependency() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));
    let importer = tmp.path().join("crates/cm-cli/src/main.rs");

    assert_eq!(
        resolver.resolve(&importer, "cm_core::store::CxStore"),
        Some(tmp.path().join("crates/cm-core/src/store.rs"))
    );
}

#[test]
fn crate_path_resolves_from_importing_crate_root() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    write_file(
        tmp.path(),
        "crates/cm-cli/src/parser/builtin/mod.rs",
        "pub mod rust;",
    );
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));
    let importer = tmp.path().join("crates/cm-cli/src/parser/mod.rs");

    assert_eq!(
        resolver.resolve(&importer, "crate::parser::builtin"),
        Some(tmp.path().join("crates/cm-cli/src/parser/builtin/mod.rs"))
    );
}

#[test]
fn super_path_resolves_from_parent_module() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    write_file(
        tmp.path(),
        "crates/cm-cli/src/parser/builtin/query_helpers.rs",
        "",
    );
    let importer = write_file(
        tmp.path(),
        "crates/cm-cli/src/parser/builtin/rust/mod.rs",
        "",
    );
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));

    assert_eq!(
        resolver.resolve(&importer, "super::query_helpers"),
        Some(
            tmp.path()
                .join("crates/cm-cli/src/parser/builtin/query_helpers.rs")
        )
    );
}

#[test]
fn renamed_dependency_resolves_through_alias() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    write_file(
        tmp.path(),
        "crates/cm-cli/Cargo.toml",
        r#"
[package]
name = "cm-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
core_alias = { package = "cm-core", path = "../cm-core" }
"#,
    );
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));
    let importer = tmp.path().join("crates/cm-cli/src/main.rs");

    assert_eq!(
        resolver.resolve(&importer, "core_alias::store"),
        Some(tmp.path().join("crates/cm-core/src/store.rs"))
    );
}

#[test]
fn workspace_true_dependency_resolves_via_root_manifest() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));
    let importer = tmp.path().join("crates/cm-cli/src/main.rs");

    assert_eq!(
        resolver.resolve(&importer, "cm_core::store"),
        Some(tmp.path().join("crates/cm-core/src/store.rs"))
    );
}

#[test]
fn workspace_renamed_dependency_resolves_via_root_manifest_alias() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.dependencies]
core-alias = { package = "cm-core", path = "crates/cm-core" }
"#,
    );
    write_file(
        tmp.path(),
        "crates/cm-core/Cargo.toml",
        r#"
[package]
name = "cm-core"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(tmp.path(), "crates/cm-core/src/lib.rs", "pub mod store;");
    write_file(
        tmp.path(),
        "crates/cm-core/src/store.rs",
        "pub struct CxStore;",
    );
    write_file(
        tmp.path(),
        "crates/cm-cli/Cargo.toml",
        r#"
[package]
name = "cm-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
core-alias.workspace = true
"#,
    );
    write_file(tmp.path(), "crates/cm-cli/src/main.rs", "fn main() {}");
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));
    let importer = tmp.path().join("crates/cm-cli/src/main.rs");

    assert_eq!(
        resolver.resolve(&importer, "core_alias::store"),
        Some(tmp.path().join("crates/cm-core/src/store.rs"))
    );
}

#[test]
fn external_crate_returns_none() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = RustImportResolver::new(&workspace_packages(&tmp));
    let importer = tmp.path().join("crates/cm-cli/src/main.rs");

    assert_eq!(resolver.resolve(&importer, "anyhow::Result"), None);
}

#[test]
fn lib_path_override_resolves_crate_root() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
"#,
    );
    write_file(
        tmp.path(),
        "crates/custom/Cargo.toml",
        r#"
[package]
name = "custom-package"
version = "0.1.0"
edition = "2024"

[lib]
name = "custom_crate"
path = "src/custom_root.rs"
"#,
    );
    let lib_root = write_file(tmp.path(), "crates/custom/src/custom_root.rs", "");
    let mut packages = HashMap::new();
    packages.insert("custom_crate".to_string(), tmp.path().join("crates/custom"));
    let resolver = RustImportResolver::new(&packages);

    assert_eq!(resolver.resolve(&lib_root, "custom_crate"), Some(lib_root));
}
