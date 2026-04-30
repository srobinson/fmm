use crate::graph::{CycleEdgeMode, GraphError, GraphIndex};
use crate::manifest::Manifest;

pub fn dependency_cycles(
    manifest: &Manifest,
    file: Option<&str>,
    edge_mode: CycleEdgeMode,
) -> Result<Vec<Vec<String>>, GraphError> {
    let graph = GraphIndex::from_manifest(manifest)?;
    let mut cycles = Vec::new();

    for component in crate::graph::dependency_cycles(&graph, edge_mode) {
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
