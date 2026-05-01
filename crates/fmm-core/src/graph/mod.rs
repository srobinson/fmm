use std::collections::HashMap;
use std::ops::Range;

use crate::identity::{EdgeKind, FileId, FileIdentityMap};
use crate::manifest::{Manifest, build_dependency_edges};

mod cycles;

pub(crate) use cycles::dependency_cycles_with_node_filter;
pub use cycles::{CycleEdgeMode, dependency_cycles};

/// Dense graph node identity used inside `GraphIndex`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

const _: () = assert!(std::mem::size_of::<NodeId>() == 4);

/// Contiguous edge range owned by one node in one direction.
pub type EdgeRange = Range<u32>;

/// One resolved dependency edge between two graph nodes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
}

const _: () = assert!(std::mem::size_of::<Edge>() <= 12);

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub file_id: FileId,
    pub downstream: EdgeRange,
    pub upstream: EdgeRange,
}

const _: () = assert!(std::mem::size_of::<Node>() <= 20);

/// Flat in-memory dependency graph.
///
/// This is an internal storage primitive. CLI and MCP surfaces continue to use
/// paths, and `to_reverse_deps` exists only as a migration adapter while query
/// APIs move from `Manifest.reverse_deps` to `GraphIndex`.
#[derive(Debug, Clone)]
pub struct GraphIndex {
    file_identity: FileIdentityMap,
    nodes: Vec<Node>,
    file_to_node: Vec<Option<NodeId>>,
    downstream_edges: Vec<Edge>,
    upstream_edges: Vec<Edge>,
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("manifest path {0} has no FileId")]
    MissingFileId(String),

    #[error("graph has more than u32::MAX {0}")]
    TooLarge(&'static str),
}

type Result<T> = std::result::Result<T, GraphError>;

impl GraphIndex {
    pub fn from_manifest(manifest: &Manifest) -> Result<Self> {
        let file_identity = manifest.file_identity().clone();
        let mut file_ids = manifest
            .files
            .keys()
            .map(|path| {
                file_identity
                    .id_for_path(path)
                    .ok_or_else(|| GraphError::MissingFileId(path.clone()))
            })
            .collect::<Result<Vec<_>>>()?;
        file_ids.sort();

        let mut file_to_node = Vec::new();
        let mut nodes = Vec::with_capacity(file_ids.len());
        for file_id in file_ids {
            let node_id = NodeId(u32_len(nodes.len(), "nodes")?);
            let slot = file_id.0 as usize;
            if file_to_node.len() <= slot {
                file_to_node.resize_with(slot + 1, || None);
            }
            file_to_node[slot] = Some(node_id);
            nodes.push(Node {
                file_id,
                downstream: 0..0,
                upstream: 0..0,
            });
        }

        let mut raw_edges = Vec::new();
        for edge in build_dependency_edges(manifest) {
            let Some(source_id) = file_identity.id_for_path(&edge.source) else {
                continue;
            };
            let Some(target_id) = file_identity.id_for_path(&edge.target) else {
                continue;
            };
            let Some(source) = node_for_file_id(&file_to_node, source_id) else {
                continue;
            };
            let Some(target) = node_for_file_id(&file_to_node, target_id) else {
                continue;
            };
            raw_edges.push(Edge {
                source,
                target,
                kind: edge.kind,
            });
        }
        raw_edges.sort_by_key(|edge| (edge.source, edge.target));
        raw_edges.dedup_by_key(|edge| (edge.source, edge.target));

        let downstream_edges = raw_edges.clone();
        set_ranges(&mut nodes, &downstream_edges, Direction::Downstream)?;

        let mut upstream_edges = raw_edges;
        upstream_edges.sort_by_key(|edge| (edge.target, edge.source));
        set_ranges(&mut nodes, &upstream_edges, Direction::Upstream)?;

        Ok(Self {
            file_identity,
            nodes,
            file_to_node,
            downstream_edges,
            upstream_edges,
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.downstream_edges.len()
    }

    pub fn node_for_file_id(&self, file_id: FileId) -> Option<NodeId> {
        node_for_file_id(&self.file_to_node, file_id)
    }

    pub fn file_id_for_node(&self, node: NodeId) -> Option<FileId> {
        self.nodes.get(node.0 as usize).map(|node| node.file_id)
    }

    pub fn file_id_for_path(&self, path: &str) -> Option<FileId> {
        self.file_identity.id_for_path(path)
    }

    pub fn path_for_file_id(&self, file_id: FileId) -> Option<&str> {
        self.file_identity
            .path_for_id(file_id)
            .map(|path| path.as_str())
    }

    pub fn downstream_edges(&self, node: NodeId) -> &[Edge] {
        let Some(node) = self.nodes.get(node.0 as usize) else {
            return &[];
        };
        edge_slice(&self.downstream_edges, &node.downstream)
    }

    pub fn upstream_edges(&self, node: NodeId) -> &[Edge] {
        let Some(node) = self.nodes.get(node.0 as usize) else {
            return &[];
        };
        edge_slice(&self.upstream_edges, &node.upstream)
    }

    /// Temporary path keyed adapter for compatibility during ALP-2120.
    pub fn to_reverse_deps(&self) -> HashMap<String, Vec<String>> {
        let mut reverse_deps: HashMap<String, Vec<String>> = HashMap::new();
        for edge in &self.upstream_edges {
            let Some(target_id) = self.file_id_for_node(edge.target) else {
                continue;
            };
            let Some(source_id) = self.file_id_for_node(edge.source) else {
                continue;
            };
            let Some(target) = self.path_for_file_id(target_id) else {
                continue;
            };
            let Some(source) = self.path_for_file_id(source_id) else {
                continue;
            };
            reverse_deps
                .entry(target.to_string())
                .or_default()
                .push(source.to_string());
        }
        for sources in reverse_deps.values_mut() {
            sources.sort();
            sources.dedup();
        }
        reverse_deps
    }
}

enum Direction {
    Downstream,
    Upstream,
}

fn set_ranges(nodes: &mut [Node], edges: &[Edge], direction: Direction) -> Result<()> {
    let mut cursor = 0;
    while cursor < edges.len() {
        let owner = match direction {
            Direction::Downstream => edges[cursor].source,
            Direction::Upstream => edges[cursor].target,
        };
        let start = cursor;
        cursor += 1;
        while cursor < edges.len() {
            let next_owner = match direction {
                Direction::Downstream => edges[cursor].source,
                Direction::Upstream => edges[cursor].target,
            };
            if next_owner != owner {
                break;
            }
            cursor += 1;
        }
        let range_start = u32_len(start, "edges")?;
        let range_end = u32_len(cursor, "edges")?;
        let range = range_start..range_end;
        let Some(node) = nodes.get_mut(owner.0 as usize) else {
            continue;
        };
        match direction {
            Direction::Downstream => node.downstream = range,
            Direction::Upstream => node.upstream = range,
        }
    }
    Ok(())
}

fn node_for_file_id(file_to_node: &[Option<NodeId>], file_id: FileId) -> Option<NodeId> {
    file_to_node.get(file_id.0 as usize).copied().flatten()
}

fn edge_slice<'a>(edges: &'a [Edge], range: &EdgeRange) -> &'a [Edge] {
    &edges[range.start as usize..range.end as usize]
}

fn u32_len(value: usize, label: &'static str) -> Result<u32> {
    u32::try_from(value).map_err(|_| GraphError::TooLarge(label))
}
