use std::collections::HashMap;

use fmm_core::graph::{GraphError, GraphIndex};
use fmm_core::identity::{EdgeKind, FileId, FileIdentityMap};
use fmm_core::manifest::Manifest;
use fmm_core::parser::Metadata;

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

#[test]
fn graph_index_builds_flat_ranges_from_file_identity() {
    let mut manifest = Manifest::new();
    add_file(&mut manifest, "src/types.ts", vec![], HashMap::new());
    add_file(&mut manifest, "src/c.ts", vec![], HashMap::new());
    add_file(&mut manifest, "src/b.ts", vec!["./c"], HashMap::new());
    add_file(
        &mut manifest,
        "src/a.ts",
        vec!["./b", "./types"],
        HashMap::from([("./types".to_string(), EdgeKind::TypeOnly)]),
    );
    manifest.rebuild_file_identity().unwrap();

    let graph = GraphIndex::from_manifest(&manifest).unwrap();

    let a = graph.node_for_file_id(FileId(0)).unwrap();
    let b = graph.node_for_file_id(FileId(1)).unwrap();
    let c = graph.node_for_file_id(FileId(2)).unwrap();
    let types = graph.node_for_file_id(FileId(3)).unwrap();

    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(graph.file_id_for_node(a), Some(FileId(0)));
    assert_eq!(graph.file_id_for_path("src/types.ts"), Some(FileId(3)));
    assert_eq!(graph.path_for_file_id(FileId(1)), Some("src/b.ts"));

    let downstream = graph.downstream_edges(a);
    assert_eq!(downstream.len(), 2);
    assert_eq!(downstream[0].target, b);
    assert_eq!(downstream[0].kind, EdgeKind::Runtime);
    assert_eq!(downstream[1].target, types);
    assert_eq!(downstream[1].kind, EdgeKind::TypeOnly);

    let upstream = graph.upstream_edges(c);
    assert_eq!(upstream.len(), 1);
    assert_eq!(upstream[0].source, b);
    assert_eq!(upstream[0].kind, EdgeKind::Runtime);
}

#[test]
fn graph_reverse_deps_adapter_matches_existing_path_index() {
    let mut manifest = Manifest::new();
    add_file(&mut manifest, "src/leaf.ts", vec![], HashMap::new());
    add_file(
        &mut manifest,
        "src/branch.ts",
        vec!["./leaf"],
        HashMap::new(),
    );
    add_file(
        &mut manifest,
        "src/root.ts",
        vec!["./branch"],
        HashMap::new(),
    );
    manifest.rebuild_file_identity().unwrap();
    manifest.rebuild_reverse_deps();

    let graph = GraphIndex::from_manifest(&manifest).unwrap();

    assert_eq!(graph.to_reverse_deps(), manifest.reverse_deps);
}

#[test]
fn graph_merges_duplicate_edges_with_runtime_winning_over_type_only() {
    // ALP-2090 design: when a single source-target pair is contributed by both
    // a runtime and a type-only dependency specifier, the merged edge must be
    // Runtime so downstream runtime traversal stays correct.
    let mut manifest = Manifest::new();
    add_file(&mut manifest, "src/types.ts", vec![], HashMap::new());
    add_file(
        &mut manifest,
        "src/a.ts",
        vec!["./types", "./types.ts"],
        HashMap::from([
            ("./types".to_string(), EdgeKind::TypeOnly),
            ("./types.ts".to_string(), EdgeKind::Runtime),
        ]),
    );
    manifest.rebuild_file_identity().unwrap();

    let graph = GraphIndex::from_manifest(&manifest).unwrap();

    let a = graph
        .node_for_file_id(graph.file_id_for_path("src/a.ts").unwrap())
        .unwrap();
    let downstream = graph.downstream_edges(a);
    assert_eq!(downstream.len(), 1, "duplicates must merge into one edge");
    assert_eq!(downstream[0].kind, EdgeKind::Runtime);
}

#[test]
fn graph_index_handles_empty_manifest() {
    let mut manifest = Manifest::new();
    manifest.rebuild_file_identity().unwrap();

    let graph = GraphIndex::from_manifest(&manifest).unwrap();

    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.file_id_for_path("src/missing.ts"), None);
    assert_eq!(graph.to_reverse_deps(), HashMap::new());
}

#[test]
fn graph_index_errors_when_manifest_path_has_no_file_identity() {
    // The strict construction check is the contract that callers must rebuild
    // file identity before building the graph. Any manifest path missing from
    // file_identity is a programmer error, not a recoverable state.
    let mut manifest = Manifest::new();
    add_file(&mut manifest, "src/a.ts", vec![], HashMap::new());
    add_file(&mut manifest, "src/b.ts", vec![], HashMap::new());

    let identity = FileIdentityMap::from_relative_paths(["src/a.ts"]).unwrap();
    manifest.set_file_identity(identity);

    match GraphIndex::from_manifest(&manifest) {
        Err(GraphError::MissingFileId(path)) => assert_eq!(path, "src/b.ts"),
        other => panic!("expected MissingFileId(\"src/b.ts\"), got {other:?}"),
    }
}
