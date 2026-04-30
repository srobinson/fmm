use super::super::{SearchFilters, dependency_graph, dependency_graph_transitive, filter_search};
use super::support::manifest_with_graph_only;

#[test]
fn direct_downstream_uses_graph_index_without_reverse_deps() {
    let manifest = manifest_with_graph_only(vec![
        ("src/base.ts", vec![]),
        ("src/feature.ts", vec!["./base"]),
    ]);
    assert!(manifest.reverse_deps.is_empty());

    let entry = manifest.files["src/base.ts"].clone();
    let (_, _, downstream) = dependency_graph(&manifest, "src/base.ts", &entry);

    assert_eq!(
        downstream
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>(),
        ["src/feature.ts"]
    );
}

#[test]
fn transitive_downstream_uses_graph_index_without_reverse_deps() {
    let manifest = manifest_with_graph_only(vec![
        ("src/root.ts", vec![]),
        ("src/middle.ts", vec!["./root"]),
        ("src/leaf.ts", vec!["./middle"]),
    ]);
    assert!(manifest.reverse_deps.is_empty());

    let entry = manifest.files["src/root.ts"].clone();
    let (_, _, downstream) = dependency_graph_transitive(&manifest, "src/root.ts", &entry, -1);

    assert_eq!(
        downstream,
        vec![
            ("src/leaf.ts".to_string(), 2),
            ("src/middle.ts".to_string(), 1),
        ]
    );
}

#[test]
fn depends_on_search_uses_graph_index_for_transitive_matches() {
    let manifest = manifest_with_graph_only(vec![
        ("src/root.ts", vec![]),
        ("src/middle.ts", vec!["./root"]),
        ("src/leaf.ts", vec!["./middle"]),
    ]);
    assert!(manifest.reverse_deps.is_empty());

    let results = filter_search(
        &manifest,
        &SearchFilters {
            export: None,
            imports: None,
            depends_on: Some("src/root.ts".to_string()),
            min_loc: None,
            max_loc: None,
        },
    );
    let files: Vec<&str> = results.iter().map(|result| result.file.as_str()).collect();

    assert_eq!(files, ["src/leaf.ts", "src/middle.ts"]);
}
