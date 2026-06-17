use std::collections::HashSet;

use crate::graph::{CycleEdgeMode, CycleOptions, GraphError, GraphIndex, NodeId};
use crate::identity::EdgeKind;
use crate::manifest::Manifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyCycle {
    pub files: Vec<String>,
    pub edges: Vec<DependencyCycleEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyCycleEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

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
    Ok(dependency_cycle_reports_with_path_filter(
        manifest,
        file,
        CycleOptions::new(edge_mode),
        keep_path,
    )?
    .into_iter()
    .map(|cycle| cycle.files)
    .collect())
}

pub fn dependency_cycle_reports(
    manifest: &Manifest,
    file: Option<&str>,
    options: CycleOptions,
) -> Result<Vec<DependencyCycle>, GraphError> {
    dependency_cycle_reports_with_path_filter(manifest, file, options, |_| true)
}

pub fn dependency_cycle_reports_with_path_filter(
    manifest: &Manifest,
    file: Option<&str>,
    options: CycleOptions,
    keep_path: impl Fn(&str) -> bool,
) -> Result<Vec<DependencyCycle>, GraphError> {
    let graph = GraphIndex::from_manifest(manifest)?;
    let mut cycles = Vec::new();

    for component in crate::graph::dependency_cycles_with_node_filter(&graph, options, |node| {
        let Some(file_id) = graph.file_id_for_node(node) else {
            return false;
        };
        let Some(path) = graph.path_for_file_id(file_id) else {
            return false;
        };
        keep_path(path)
    }) {
        let mut members = component_paths(&graph, &component);
        members.sort();
        if file.is_none_or(|scoped_file| members.iter().any(|member| member == scoped_file)) {
            cycles.push(DependencyCycle {
                edges: component_edges(&graph, &component, options),
                files: members,
            });
        }
    }

    cycles.sort_by(|a, b| a.files.cmp(&b.files));
    Ok(cycles)
}

fn component_paths(graph: &GraphIndex, component: &[NodeId]) -> Vec<String> {
    component
        .iter()
        .filter_map(|node| {
            let file_id = graph.file_id_for_node(*node)?;
            graph.path_for_file_id(file_id).map(str::to_string)
        })
        .collect()
}

fn component_edges(
    graph: &GraphIndex,
    component: &[NodeId],
    options: CycleOptions,
) -> Vec<DependencyCycleEdge> {
    let members = component.iter().copied().collect::<HashSet<_>>();
    let mut edges = Vec::new();

    for source in component {
        for edge in graph.downstream_edges(*source) {
            if !options.keeps(*edge) || !members.contains(&edge.target) {
                continue;
            }
            let Some(source_path) = graph
                .file_id_for_node(edge.source)
                .and_then(|file_id| graph.path_for_file_id(file_id))
            else {
                continue;
            };
            let Some(target_path) = graph
                .file_id_for_node(edge.target)
                .and_then(|file_id| graph.path_for_file_id(file_id))
            else {
                continue;
            };
            edges.push(DependencyCycleEdge {
                source: source_path.to_string(),
                target: target_path.to_string(),
                kind: edge.kind,
            });
        }
    }

    edges.sort_by(|a, b| {
        (&a.source, &a.target, a.kind.as_str()).cmp(&(&b.source, &b.target, b.kind.as_str()))
    });
    edges
}
