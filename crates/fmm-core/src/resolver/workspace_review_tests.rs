use super::*;
use std::fs;
use tempfile::TempDir;

fn write_file(base: &Path, rel: &str, content: &str) {
    let path = base.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn discover_partitions_same_package_name_by_ecosystem() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"workspaces":["packages/*"]}"#,
    );
    write_file(
        tmp.path(),
        "packages/shared/package.json",
        r#"{"name":"shared"}"#,
    );
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"[workspace]
members = ["crates/*"]
"#,
    );
    write_file(
        tmp.path(),
        "crates/shared/Cargo.toml",
        r#"[package]
name = "shared"
version = "0.1.0"
"#,
    );

    let info = discover(tmp.path());

    assert_eq!(
        info.packages_for(WorkspaceEcosystem::Js).get("shared"),
        Some(&tmp.path().join("packages/shared"))
    );
    assert_eq!(
        info.packages_for(WorkspaceEcosystem::Rust).get("shared"),
        Some(&tmp.path().join("crates/shared"))
    );
}

#[test]
fn discover_missing_ecosystem_partition_does_not_fall_back_to_global() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"[workspace]
members = ["crates/*"]
"#,
    );
    write_file(
        tmp.path(),
        "crates/shared/Cargo.toml",
        r#"[package]
name = "shared"
version = "0.1.0"
"#,
    );

    let info = discover(tmp.path());

    assert!(
        info.packages_for(WorkspaceEcosystem::Rust)
            .contains_key("shared")
    );
    assert!(info.packages_for(WorkspaceEcosystem::Js).is_empty());
    assert!(info.roots_for(WorkspaceEcosystem::Js).is_empty());
}

#[test]
fn go_work_local_replace_discovers_module() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.work",
        r#"go 1.23.0
use ./services/api
replace example.com/shared v1.2.3 => ./libs/shared
"#,
    );
    write_file(
        tmp.path(),
        "services/api/go.mod",
        "module example.com/api\n\ngo 1.23.0\n",
    );
    write_file(
        tmp.path(),
        "libs/shared/go.mod",
        "module example.com/shared\n\ngo 1.23.0\n",
    );

    let info = discover(tmp.path());

    assert_eq!(
        info.packages_for(WorkspaceEcosystem::Go)
            .get("example.com/shared"),
        Some(&tmp.path().join("libs/shared"))
    );
}
