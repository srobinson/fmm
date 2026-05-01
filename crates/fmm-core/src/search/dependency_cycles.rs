use crate::graph::{CycleEdgeMode, GraphError, GraphIndex};
use crate::manifest::Manifest;

pub fn dependency_cycles(
    manifest: &Manifest,
    file: Option<&str>,
    edge_mode: CycleEdgeMode,
) -> Result<Vec<Vec<String>>, GraphError> {
    dependency_cycles_with_path_filter(manifest, file, edge_mode, |_| true)
}

pub fn dependency_cycles_with_path_filter(
    manifest: &Manifest,
    file: Option<&str>,
    edge_mode: CycleEdgeMode,
    keep_path: impl Fn(&str) -> bool,
) -> Result<Vec<Vec<String>>, GraphError> {
    let graph = GraphIndex::from_manifest(manifest)?;
    let mut cycles = Vec::new();

    for component in crate::graph::dependency_cycles_with_node_filter(&graph, edge_mode, |node| {
        let Some(file_id) = graph.file_id_for_node(node) else {
            return false;
        };
        let Some(path) = graph.path_for_file_id(file_id) else {
            return false;
        };
        keep_path(path)
    }) {
        let mut members = component
            .into_iter()
            .filter_map(|node| {
                let file_id = graph.file_id_for_node(node)?;
                graph.path_for_file_id(file_id).map(str::to_string)
            })
            .collect::<Vec<_>>();
        members.sort();
        if file.is_none_or(|scoped_file| members.iter().any(|member| member == scoped_file)) {
            cycles.push(members);
        }
    }

    cycles.sort();
    Ok(cycles)
}
