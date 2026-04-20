use super::*;

fn exts() -> &'static HashSet<String> {
    builtin_source_extensions()
}

fn write_file(base: &Path, rel: &str, content: &str) -> std::path::PathBuf {
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn build_reverse_deps_dispatches_cross_package_resolution_by_source_extension() {
    use std::fs;

    let tmp = tempfile::TempDir::new().unwrap();
    let shared_dir = tmp.path().join("packages/shared");
    let target = shared_dir.join("util.ts");
    let ts_importer = tmp.path().join("packages/app/index.ts");
    let rs_importer = tmp.path().join("crates/app/src/lib.rs");

    for path in [&target, &ts_importer, &rs_importer] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    let target_key = target.to_string_lossy().into_owned();
    let ts_importer_key = ts_importer.to_string_lossy().into_owned();
    let rs_importer_key = rs_importer.to_string_lossy().into_owned();

    let mut manifest = Manifest::new();
    manifest.workspace_roots.push(shared_dir);
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        ts_importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["shared/util".to_string()],
            ..Default::default()
        },
    );
    manifest.files.insert(
        rs_importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["shared/util".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&ts_importer_key),
        "TS importer should resolve through the JS/TS cross-package path, got: {:?}",
        importers
    );
    assert!(
        !importers.contains(&rs_importer_key),
        "Rust importer should ignore JS-style slash specifiers, got: {:?}",
        importers
    );
}

#[test]
fn build_reverse_deps_resolves_deno_workspace_package_import() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.json",
        r#"{"workspace":["./app","./shared"]}"#,
    );
    write_file(tmp.path(), "app/deno.json", r#"{"name":"app"}"#);
    write_file(tmp.path(), "shared/deno.json", r#"{"name":"shared"}"#);
    let importer = write_file(
        tmp.path(),
        "app/main.ts",
        "import { value } from 'shared';\nconsole.log(value);\n",
    );
    let target = write_file(tmp.path(), "shared/mod.ts", "export const value = 1;\n");

    let importer_key = importer.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_roots.extend([
        tmp.path().to_path_buf(),
        tmp.path().join("app"),
        tmp.path().join("shared"),
    ]);
    manifest
        .workspace_packages
        .insert("app".into(), tmp.path().join("app"));
    manifest
        .workspace_packages
        .insert("shared".into(), tmp.path().join("shared"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["shared".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&importer_key),
        "Deno workspace package import should resolve through the Deno resolver, got: {:?}",
        importers
    );
}

#[test]
fn build_reverse_deps_resolves_deno_import_map_and_ignores_remote_imports() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.json",
        r#"{"workspace":["./app"],"imports":{"@/":"./app/src/","remote":"https://deno.land/std/mod.ts"}}"#,
    );
    write_file(tmp.path(), "app/deno.json", r#"{"name":"app"}"#);
    let importer = write_file(
        tmp.path(),
        "app/src/main.ts",
        "import { local } from '@/local.ts';\nimport 'https://deno.land/std/assert/mod.ts';\nimport 'jsr:@std/assert';\nimport 'remote';\nconsole.log(local);\n",
    );
    let target = write_file(tmp.path(), "app/src/local.ts", "export const local = 1;\n");

    let importer_key = importer.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_roots
        .extend([tmp.path().to_path_buf(), tmp.path().join("app")]);
    manifest
        .workspace_packages
        .insert("app".into(), tmp.path().join("app"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec![
                "@/local.ts".to_string(),
                "https://deno.land/std/assert/mod.ts".to_string(),
                "jsr:@std/assert".to_string(),
                "remote".to_string(),
            ],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert_eq!(importers, vec![importer_key]);
    assert_eq!(reverse_deps.len(), 1);
}

#[test]
fn build_reverse_deps_resolves_rust_cross_crate_named_import() {
    let tmp = tempfile::TempDir::new().unwrap();
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
    let target = write_file(
        tmp.path(),
        "crates/cm-core/src/store.rs",
        "pub struct CxStore;",
    );
    write_file(tmp.path(), "crates/cm-core/src/lib.rs", "pub mod store;");
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
    let importer = write_file(
        tmp.path(),
        "crates/cm-cli/src/main.rs",
        "use cm_core::store::CxStore;",
    );

    let target_key = target.to_string_lossy().into_owned();
    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("cm_core".into(), tmp.path().join("crates/cm-core"));
    manifest
        .workspace_packages
        .insert("cm_cli".into(), tmp.path().join("crates/cm-cli"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            named_imports: std::collections::HashMap::from([(
                "cm_core::store".to_string(),
                vec!["CxStore".to_string()],
            )]),
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&importer_key),
        "Rust named import should resolve to the workspace crate module, got: {:?}",
        importers
    );
}

#[test]
fn build_reverse_deps_resolves_rust_cross_crate_module_import() {
    let tmp = tempfile::TempDir::new().unwrap();
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
    let root = write_file(tmp.path(), "crates/cm-core/src/lib.rs", "pub mod store;");
    let target = write_file(
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
    let importer = write_file(
        tmp.path(),
        "crates/cm-cli/src/main.rs",
        "use cm_core::store;",
    );

    let root_key = root.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("cm_core".into(), tmp.path().join("crates/cm-core"));
    manifest
        .workspace_packages
        .insert("cm_cli".into(), tmp.path().join("crates/cm-cli"));
    manifest
        .files
        .insert(root_key.clone(), crate::manifest::FileEntry::default());
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            named_imports: std::collections::HashMap::from([(
                "cm_core".to_string(),
                vec!["store".to_string()],
            )]),
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();
    let root_importers = reverse_deps.get(&root_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&importer_key),
        "Rust module import should resolve to the module file, got: {:?}",
        importers
    );
    assert!(
        root_importers.contains(&importer_key),
        "Rust module import should also keep the crate root edge, got: {:?}",
        root_importers
    );
}

#[test]
fn build_reverse_deps_ignores_undeclared_rust_workspace_dependency() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"
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
    let target = write_file(
        tmp.path(),
        "crates/cm-core/src/store.rs",
        "pub struct CxStore;",
    );
    write_file(tmp.path(), "crates/cm-core/src/lib.rs", "pub mod store;");
    write_file(
        tmp.path(),
        "crates/cm-cli/Cargo.toml",
        r#"
[package]
name = "cm-cli"
version = "0.1.0"
edition = "2024"
"#,
    );
    let importer = write_file(
        tmp.path(),
        "crates/cm-cli/src/main.rs",
        "use cm_core::store::CxStore;",
    );

    let target_key = target.to_string_lossy().into_owned();
    let importer_key = importer.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("cm_core".into(), tmp.path().join("crates/cm-core"));
    manifest
        .workspace_packages
        .insert("cm_cli".into(), tmp.path().join("crates/cm-cli"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            named_imports: std::collections::HashMap::from([(
                "cm_core::store".to_string(),
                vec!["CxStore".to_string()],
            )]),
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        !importers.contains(&importer_key),
        "undeclared Rust workspace dependency should not resolve, got: {:?}",
        importers
    );
}

#[test]
fn build_reverse_deps_resolves_rust_crate_path_from_importer_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"
"#,
    );
    write_file(
        tmp.path(),
        "crates/app/Cargo.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(tmp.path(), "crates/app/src/lib.rs", "pub mod parser;");
    let importer = write_file(
        tmp.path(),
        "crates/app/src/parser/mod.rs",
        "use crate::parser::builtin;",
    );
    let target = write_file(tmp.path(), "crates/app/src/parser/builtin/mod.rs", "");

    let importer_key = importer.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("app".into(), tmp.path().join("crates/app"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            dependencies: vec!["crate::parser::builtin".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&importer_key),
        "Rust crate:: dependency should resolve from the importing crate root, got: {:?}",
        importers
    );
}

#[test]
fn build_reverse_deps_avoids_generic_relative_match_for_rust_super_paths() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"
"#,
    );
    write_file(
        tmp.path(),
        "crates/app/Cargo.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(tmp.path(), "crates/app/src/lib.rs", "pub mod parser;");
    let importer = write_file(
        tmp.path(),
        "crates/app/src/parser/builtin/rust.rs",
        "use super::query_helpers;",
    );
    let correct_target = write_file(
        tmp.path(),
        "crates/app/src/parser/builtin/query_helpers.rs",
        "",
    );
    let wrong_target = write_file(tmp.path(), "crates/app/src/parser/query_helpers.rs", "");

    let importer_key = importer.to_string_lossy().into_owned();
    let correct_key = correct_target.to_string_lossy().into_owned();
    let wrong_key = wrong_target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("app".into(), tmp.path().join("crates/app"));
    manifest
        .files
        .insert(correct_key.clone(), crate::manifest::FileEntry::default());
    manifest
        .files
        .insert(wrong_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            dependencies: vec!["../query_helpers".to_string()],
            named_imports: std::collections::HashMap::from([(
                "super".to_string(),
                vec!["query_helpers".to_string()],
            )]),
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let correct_importers = reverse_deps.get(&correct_key).cloned().unwrap_or_default();
    let wrong_importers = reverse_deps.get(&wrong_key).cloned().unwrap_or_default();

    assert!(
        correct_importers.contains(&importer_key),
        "Rust super path should resolve through module semantics, got: {:?}",
        correct_importers
    );
    assert!(
        !wrong_importers.contains(&importer_key),
        "Rust super path should not use generic filesystem relative matching, got: {:?}",
        wrong_importers
    );
}

#[test]
fn build_reverse_deps_keeps_generic_rust_deps_without_cargo_workspace() {
    let mut manifest = Manifest::new();
    manifest
        .workspace_roots
        .push(std::path::PathBuf::from("packages/ui"));
    manifest.files.insert(
        "src/config.rs".to_string(),
        crate::manifest::FileEntry::default(),
    );
    manifest.files.insert(
        "src/main.rs".to_string(),
        crate::manifest::FileEntry {
            dependencies: vec!["crate::config".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps
        .get("src/config.rs")
        .cloned()
        .unwrap_or_default();

    assert!(
        importers.contains(&"src/main.rs".to_string()),
        "Rust fallback deps should still work without Cargo metadata, got: {:?}",
        importers
    );
}

#[test]
fn dep_matches_relative_path() {
    assert!(dep_matches(
        "./types",
        "src/types.ts",
        "src/index.ts",
        exts()
    ));
    assert!(dep_matches(
        "./config",
        "src/config.ts",
        "src/index.ts",
        exts()
    ));
    assert!(!dep_matches(
        "./types",
        "src/other.ts",
        "src/index.ts",
        exts()
    ));
}

#[test]
fn dep_matches_compound_filename_with_dot() {
    assert!(dep_matches(
        "../errors/exceptions/runtime.exception",
        "packages/core/errors/exceptions/runtime.exception.ts",
        "packages/core/injector/injector.ts",
        exts(),
    ));
    assert!(dep_matches(
        "../errors/exceptions/undefined-dependency.exception",
        "packages/core/errors/exceptions/undefined-dependency.exception.ts",
        "packages/core/injector/injector.ts",
        exts(),
    ));
    assert!(dep_matches(
        "../utils/crypto.utils.js",
        "pkg/src/utils/crypto.utils.ts",
        "pkg/src/services/auth.service.ts",
        exts(),
    ));
}

#[test]
fn dep_matches_nested_path() {
    assert!(dep_matches(
        "./utils/helpers",
        "src/utils/helpers.ts",
        "src/index.ts",
        exts(),
    ));
    assert!(!dep_matches(
        "./utils/helpers",
        "src/utils/other.ts",
        "src/index.ts",
        exts(),
    ));
}

#[test]
fn dep_matches_parent_relative() {
    assert!(dep_matches(
        "../utils/crypto.utils.js",
        "pkg/src/utils/crypto.utils.ts",
        "pkg/src/services/auth.service.ts",
        exts(),
    ));
    assert!(!dep_matches(
        "../utils/crypto.utils.js",
        "pkg/src/services/other.ts",
        "pkg/src/services/auth.service.ts",
        exts(),
    ));
}

#[test]
fn dep_matches_deep_parent_relative() {
    assert!(dep_matches(
        "../../../utils/crypto.utils.js",
        "pkg/src/utils/crypto.utils.ts",
        "pkg/src/tests/unit/auth/test.ts",
        exts(),
    ));
}

#[test]
fn dep_matches_without_prefix() {
    assert!(dep_matches("types", "src/types.ts", "src/index.ts", exts()));
}

#[test]
fn dep_matches_python_package() {
    assert!(dep_matches(
        "./utils",
        "src/utils/__init__.py",
        "src/service.py",
        exts(),
    ));
    assert!(dep_matches(
        "../models",
        "models/__init__.py",
        "src/service.py",
        exts(),
    ));
    assert!(dep_matches(
        "./utils",
        "src/utils.py",
        "src/service.py",
        exts()
    ));
    assert!(!dep_matches(
        "./utils",
        "src/auth/__init__.py",
        "src/service.py",
        exts(),
    ));
}

#[test]
fn dep_matches_crate_path() {
    assert!(dep_matches(
        "crate::config",
        "src/config.rs",
        "src/main.rs",
        exts()
    ));
    assert!(dep_matches(
        "crate::parser::builtin",
        "src/parser/builtin.rs",
        "src/main.rs",
        exts(),
    ));
    assert!(!dep_matches(
        "crate::config",
        "src/other.rs",
        "src/main.rs",
        exts()
    ));
}

#[test]
fn dep_matches_go_module_path() {
    assert!(dep_matches(
        "github.com/user/project/internal/handler",
        "internal/handler/handler.go",
        "cmd/main.go",
        exts(),
    ));
    assert!(!dep_matches(
        "fmt",
        "internal/format/format.go",
        "cmd/main.go",
        exts(),
    ));
}
