use std::collections::HashMap;
use std::path::Path;

use fmm_core::identity::EdgeKind;
use fmm_core::manifest::Manifest;
use fmm_core::parser::builtin::typescript::TypeScriptParser;
use fmm_core::parser::{Metadata, Parser};
use fmm_core::search::{CycleEdgeMode, dependency_cycles};

fn add_file(
    manifest: &mut Manifest,
    path: &str,
    dependencies: Vec<&str>,
    dependency_kinds: HashMap<String, EdgeKind>,
) {
    manifest.add_file(
        path,
        Metadata {
            dependencies: dependencies.into_iter().map(str::to_string).collect(),
            dependency_kinds,
            loc: 10,
            ..Default::default()
        },
    );
}

fn runtime_manifest(edges: &[(&str, Vec<&str>)]) -> Manifest {
    let mut manifest = Manifest::new();
    for (path, dependencies) in edges {
        add_file(&mut manifest, path, dependencies.clone(), HashMap::new());
    }
    manifest.rebuild_file_identity().unwrap();
    manifest
}

fn write_file(base: &Path, rel: &str, content: &str) {
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn add_ts_file(manifest: &mut Manifest, parser: &mut TypeScriptParser, path: &str, source: &str) {
    manifest.add_file(path, parser.parse(source).unwrap().metadata);
}

fn workspace_package_cycle_manifest(root: &Path, a_source: &str, b_source: &str) -> Manifest {
    write_file(
        root,
        "packages/a/package.json",
        r#"{"name":"@scope/a","main":"src/index.ts"}"#,
    );
    write_file(root, "packages/a/src/index.ts", a_source);
    write_file(
        root,
        "packages/b/package.json",
        r#"{"name":"@scope/b","main":"src/index.ts"}"#,
    );
    write_file(root, "packages/b/src/index.ts", b_source);

    let mut parser = TypeScriptParser::new().unwrap();
    let mut manifest = Manifest::new();
    manifest
        .workspace_packages
        .insert("@scope/a".into(), root.join("packages/a"));
    manifest
        .workspace_packages
        .insert("@scope/b".into(), root.join("packages/b"));
    add_ts_file(
        &mut manifest,
        &mut parser,
        "packages/a/src/index.ts",
        a_source,
    );
    add_ts_file(
        &mut manifest,
        &mut parser,
        "packages/b/src/index.ts",
        b_source,
    );
    manifest.rebuild_file_identity().unwrap();
    manifest
}

#[test]
fn dependency_cycles_groups_runtime_scc_paths_deterministically() {
    let manifest = runtime_manifest(&[
        ("src/c.ts", vec![]),
        ("src/b.ts", vec!["./a"]),
        ("src/a.ts", vec!["./b"]),
    ]);

    let cycles = dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap();

    assert_eq!(
        cycles,
        vec![vec!["src/a.ts".to_string(), "src/b.ts".to_string()]]
    );
}

#[test]
fn dependency_cycles_excludes_type_only_workspace_package_cycles_in_runtime_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manifest = workspace_package_cycle_manifest(
        tmp.path(),
        "import type { B } from '@scope/b';\nexport type A = B;\n",
        "import type { A } from '@scope/a';\nexport type B = A;\n",
    );

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        Vec::<Vec<String>>::new()
    );
    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::All).unwrap(),
        vec![vec![
            "packages/a/src/index.ts".to_string(),
            "packages/b/src/index.ts".to_string()
        ]]
    );
}

#[test]
fn dependency_cycles_reports_mixed_workspace_package_cycles_in_runtime_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manifest = workspace_package_cycle_manifest(
        tmp.path(),
        "import type { B } from '@scope/b';\nimport { b } from '@scope/b';\nexport type A = B;\nexport const a = b;\n",
        "import type { A } from '@scope/a';\nimport { a } from '@scope/a';\nexport type B = A;\nexport const b = a;\n",
    );

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        vec![vec![
            "packages/a/src/index.ts".to_string(),
            "packages/b/src/index.ts".to_string()
        ]]
    );
}

#[test]
fn dependency_cycles_excludes_type_only_workspace_package_reexport_cycles_in_runtime_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manifest = workspace_package_cycle_manifest(
        tmp.path(),
        "export type { B } from '@scope/b';\n",
        "export { type A } from '@scope/a';\n",
    );

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        Vec::<Vec<String>>::new()
    );
    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::All).unwrap(),
        vec![vec![
            "packages/a/src/index.ts".to_string(),
            "packages/b/src/index.ts".to_string()
        ]]
    );
}

#[test]
fn dependency_cycles_reports_value_workspace_package_reexport_cycles_in_runtime_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manifest = workspace_package_cycle_manifest(
        tmp.path(),
        "export { b } from '@scope/b';\nexport const a = 1;\n",
        "export { a } from '@scope/a';\nexport const b = 1;\n",
    );

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        vec![vec![
            "packages/a/src/index.ts".to_string(),
            "packages/b/src/index.ts".to_string()
        ]]
    );
}

#[test]
fn dependency_cycles_reports_mixed_workspace_package_reexport_cycles_in_runtime_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manifest = workspace_package_cycle_manifest(
        tmp.path(),
        "export { type B, b } from '@scope/b';\nexport type A = string;\nexport const a = 1;\n",
        "export { type A, a } from '@scope/a';\nexport type B = string;\nexport const b = 1;\n",
    );

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        vec![vec![
            "packages/a/src/index.ts".to_string(),
            "packages/b/src/index.ts".to_string()
        ]]
    );
}

#[test]
fn dependency_cycles_excludes_type_only_cycles_in_runtime_mode() {
    let mut manifest = Manifest::new();
    add_file(
        &mut manifest,
        "src/a.ts",
        vec!["./b"],
        HashMap::from([("./b".to_string(), EdgeKind::TypeOnly)]),
    );
    add_file(
        &mut manifest,
        "src/b.ts",
        vec!["./a"],
        HashMap::from([("./a".to_string(), EdgeKind::TypeOnly)]),
    );
    manifest.rebuild_file_identity().unwrap();

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        Vec::<Vec<String>>::new()
    );
    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::All).unwrap(),
        vec![vec!["src/a.ts".to_string(), "src/b.ts".to_string()]]
    );
}

#[test]
fn dependency_cycles_mixed_runtime_and_type_only_cycle_requires_all_mode() {
    let mut manifest = Manifest::new();
    add_file(&mut manifest, "src/a.ts", vec!["./b"], HashMap::new());
    add_file(
        &mut manifest,
        "src/b.ts",
        vec!["./a"],
        HashMap::from([("./a".to_string(), EdgeKind::TypeOnly)]),
    );
    manifest.rebuild_file_identity().unwrap();

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        Vec::<Vec<String>>::new()
    );
    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::All).unwrap(),
        vec![vec!["src/a.ts".to_string(), "src/b.ts".to_string()]]
    );
}

#[test]
fn dependency_cycles_reports_true_self_loop() {
    let manifest = runtime_manifest(&[("src/self.ts", vec!["./self"])]);

    let cycles = dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap();

    assert_eq!(cycles, vec![vec!["src/self.ts".to_string()]]);
}

#[test]
fn dependency_cycles_returns_empty_for_acyclic_graph() {
    let manifest = runtime_manifest(&[
        ("src/a.ts", vec!["./b"]),
        ("src/b.ts", vec!["./c"]),
        ("src/c.ts", vec![]),
    ]);

    assert_eq!(
        dependency_cycles(&manifest, None, CycleEdgeMode::Runtime).unwrap(),
        Vec::<Vec<String>>::new()
    );
}

#[test]
fn dependency_cycles_file_scope_keeps_only_matching_component() {
    let manifest = runtime_manifest(&[
        ("src/a.ts", vec!["./b"]),
        ("src/b.ts", vec!["./a"]),
        ("src/c.ts", vec!["./d"]),
        ("src/d.ts", vec!["./c"]),
    ]);

    let cycles = dependency_cycles(&manifest, Some("src/c.ts"), CycleEdgeMode::Runtime).unwrap();

    assert_eq!(
        cycles,
        vec![vec!["src/c.ts".to_string(), "src/d.ts".to_string()]]
    );
}
