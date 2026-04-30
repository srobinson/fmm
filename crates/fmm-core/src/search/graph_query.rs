use std::collections::{HashSet, VecDeque};

use crate::graph::{GraphError, GraphIndex, NodeId};
use crate::manifest::Manifest;

/// Path shaped dependency traversal over the internal graph index.
pub struct DependencyGraphQuery<'a> {
    manifest: &'a Manifest,
    graph: GraphIndex,
}

impl<'a> DependencyGraphQuery<'a> {
    pub fn new(manifest: &'a Manifest) -> Result<Self, GraphError> {
        Ok(Self {
            manifest,
            graph: GraphIndex::from_manifest(manifest)?,
        })
    }

    pub fn direct_upstream(&self, file: &str) -> Vec<String> {
        let Some(node) = self.node_for_path(file) else {
            return Vec::new();
        };
        let mut paths: Vec<String> = self
            .graph
            .downstream_edges(node)
            .iter()
            .filter_map(|edge| self.path_for_node(edge.target).map(str::to_string))
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    pub fn direct_downstream(&self, file: &str) -> Vec<&'a String> {
        let Some(node) = self.node_for_path(file) else {
            return Vec::new();
        };
        let mut paths: Vec<&String> = self
            .graph
            .upstream_edges(node)
            .iter()
            .filter_map(|edge| self.manifest_path_for_node(edge.source))
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    pub fn transitive_downstream(&self, file: &str, depth: i32) -> Vec<(String, i32)> {
        let Some(start) = self.node_for_path(file) else {
            return Vec::new();
        };
        let mut downstream = Vec::new();
        let mut visited = HashSet::from([start]);
        let mut queue = VecDeque::new();

        for edge in self.graph.upstream_edges(start) {
            if !visited.contains(&edge.source) {
                queue.push_back((edge.source, 1));
            }
        }

        while let Some((current, current_depth)) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }
            if let Some(path) = self.path_for_node(current) {
                downstream.push((path.to_string(), current_depth));
            }

            if depth == -1 || current_depth < depth {
                for edge in self.graph.upstream_edges(current) {
                    if !visited.contains(&edge.source) {
                        queue.push_back((edge.source, current_depth + 1));
                    }
                }
            }
        }

        downstream.sort_by(|a, b| a.0.cmp(&b.0));
        downstream
    }

    pub fn transitive_dependents(&self, targets: &HashSet<String>) -> HashSet<String> {
        let mut dependents = HashSet::new();
        for target in targets {
            for (path, _) in self.transitive_downstream(target, -1) {
                dependents.insert(path);
            }
        }
        dependents
    }

    pub fn downstream_count(&self, file: &str) -> usize {
        self.direct_downstream(file).len()
    }

    fn node_for_path(&self, path: &str) -> Option<NodeId> {
        let file_id = self.graph.file_id_for_path(path)?;
        self.graph.node_for_file_id(file_id)
    }

    fn path_for_node(&self, node: NodeId) -> Option<&str> {
        let file_id = self.graph.file_id_for_node(node)?;
        self.graph.path_for_file_id(file_id)
    }

    fn manifest_path_for_node(&self, node: NodeId) -> Option<&'a String> {
        let path = self.path_for_node(node)?;
        self.manifest
            .files
            .get_key_value(path)
            .map(|(path, _)| path)
    }
}
