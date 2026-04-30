use std::collections::HashMap;

use fmm_core::identity::EdgeKind;
use fmm_core::manifest::Manifest;
use fmm_core::parser::Metadata;
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
