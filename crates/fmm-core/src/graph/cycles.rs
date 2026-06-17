use crate::identity::EdgeKind;

use super::{Edge, GraphIndex, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleEdgeMode {
    Runtime,
    All,
}

impl CycleEdgeMode {
    fn keeps(self, kind: EdgeKind) -> bool {
        match self {
            Self::Runtime => kind.is_runtime(),
            Self::All => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleOptions {
    pub edge_mode: CycleEdgeMode,
    pub include_mod_hierarchy: bool,
}

impl CycleOptions {
    pub fn new(edge_mode: CycleEdgeMode) -> Self {
        Self {
            edge_mode,
            include_mod_hierarchy: false,
        }
    }

    pub fn include_mod_hierarchy(mut self, include_mod_hierarchy: bool) -> Self {
        self.include_mod_hierarchy = include_mod_hierarchy;
        self
    }

    pub(crate) fn keeps(self, edge: Edge) -> bool {
        self.edge_mode.keeps(edge.kind)
            && (self.include_mod_hierarchy || !edge.kind.is_module_hierarchy())
    }
}

#[derive(Debug, Clone, Copy)]
struct DfsFrame {
    node: NodeId,
    next_edge: usize,
}

#[derive(Debug)]
struct TarjanState {
    next_index: u32,
    indices: Vec<Option<u32>>,
    lowlinks: Vec<u32>,
    on_stack: Vec<bool>,
    stack: Vec<NodeId>,
    frames: Vec<DfsFrame>,
    components: Vec<Vec<NodeId>>,
}

pub fn dependency_cycles(graph: &GraphIndex, edge_mode: CycleEdgeMode) -> Vec<Vec<NodeId>> {
    dependency_cycles_with_options(graph, CycleOptions::new(edge_mode))
}

pub fn dependency_cycles_with_options(
    graph: &GraphIndex,
    options: CycleOptions,
) -> Vec<Vec<NodeId>> {
    dependency_cycles_with_node_filter(graph, options, |_| true)
}

pub(crate) fn dependency_cycles_with_node_filter(
    graph: &GraphIndex,
    options: CycleOptions,
    keep_node: impl Fn(NodeId) -> bool,
) -> Vec<Vec<NodeId>> {
    let kept_nodes = (0..graph.nodes.len())
        .map(|node_index| keep_node(NodeId(node_index as u32)))
        .collect::<Vec<_>>();
    let mut state = TarjanState::new(graph.nodes.len());
    for node_index in 0..graph.nodes.len() {
        if !kept_nodes[node_index] {
            continue;
        }
        let start = NodeId(node_index as u32);
        if state.indices[node_index].is_none() {
            state.traverse_from(graph, options, &kept_nodes, start);
        }
    }
    state.sort_components();
    state.components
}

impl TarjanState {
    fn new(node_count: usize) -> Self {
        Self {
            next_index: 0,
            indices: vec![None; node_count],
            lowlinks: vec![0; node_count],
            on_stack: vec![false; node_count],
            stack: Vec::new(),
            frames: Vec::new(),
            components: Vec::new(),
        }
    }

    fn traverse_from(
        &mut self,
        graph: &GraphIndex,
        options: CycleOptions,
        kept_nodes: &[bool],
        start: NodeId,
    ) {
        self.push_node(start);

        while !self.frames.is_empty() {
            let Some(edge) = self.next_edge(graph, options, kept_nodes) else {
                self.finish_node(graph, options);
                continue;
            };

            let target = edge.target;
            let target_index = target.0 as usize;
            if self.indices[target_index].is_none() {
                self.push_node(target);
            } else if self.on_stack[target_index] {
                let current = self.frames.last().expect("current frame exists").node;
                self.update_lowlink(current, self.indices[target_index].unwrap());
            }
        }
    }

    fn push_node(&mut self, node: NodeId) {
        let node_index = node.0 as usize;
        self.indices[node_index] = Some(self.next_index);
        self.lowlinks[node_index] = self.next_index;
        self.next_index += 1;
        self.stack.push(node);
        self.on_stack[node_index] = true;
        self.frames.push(DfsFrame { node, next_edge: 0 });
    }

    fn next_edge(
        &mut self,
        graph: &GraphIndex,
        options: CycleOptions,
        kept_nodes: &[bool],
    ) -> Option<Edge> {
        let frame = self.frames.last_mut()?;
        let edges = graph.downstream_edges(frame.node);
        while frame.next_edge < edges.len() {
            let edge = edges[frame.next_edge];
            frame.next_edge += 1;
            if options.keeps(edge) && kept_nodes[edge.target.0 as usize] {
                return Some(edge);
            }
        }
        None
    }

    fn finish_node(&mut self, graph: &GraphIndex, options: CycleOptions) {
        let frame = self.frames.pop().expect("frame exists");
        let node = frame.node;
        if self.lowlinks[node.0 as usize] == self.indices[node.0 as usize].unwrap() {
            self.collect_component(graph, options, node);
        }
        if let Some(parent) = self.frames.last() {
            let child_lowlink = self.lowlinks[node.0 as usize];
            self.update_lowlink(parent.node, child_lowlink);
        }
    }

    fn collect_component(&mut self, graph: &GraphIndex, options: CycleOptions, root: NodeId) {
        let mut component = Vec::new();
        while let Some(node) = self.stack.pop() {
            self.on_stack[node.0 as usize] = false;
            component.push(node);
            if node == root {
                break;
            }
        }
        if component.len() > 1 || has_self_loop(graph, options, root) {
            component.sort();
            self.components.push(component);
        }
    }

    fn update_lowlink(&mut self, node: NodeId, candidate: u32) {
        let lowlink = &mut self.lowlinks[node.0 as usize];
        *lowlink = (*lowlink).min(candidate);
    }

    fn sort_components(&mut self) {
        self.components
            .sort_by(|left, right| left.first().cmp(&right.first()));
    }
}

fn has_self_loop(graph: &GraphIndex, options: CycleOptions, node: NodeId) -> bool {
    graph
        .downstream_edges(node)
        .iter()
        .any(|edge| edge.target == node && options.keeps(*edge))
}
