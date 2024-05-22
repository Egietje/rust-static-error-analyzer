use dot::{Edges, Nodes};
use rustc_hir::HirId;
use rustc_hir::def_id::DefId;
use std::borrow::Cow;
use std::cmp::PartialEq;

#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: usize,
    pub label: String,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    LocalFn {
        hir_id: HirId,
    },
    NonLocalFn {
        def_id: DefId,
    },
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub call_id: HirId,
    label: String,
}

impl<'a> dot::Labeller<'a, Node, Edge> for Graph {
    fn graph_id(&'a self) -> dot::Id<'a> {
        dot::Id::new("call_graph").unwrap()
    }

    fn node_id(&'a self, n: &Node) -> dot::Id<'a> {
        dot::Id::new(format!("node{:?}", n.id)).unwrap()
    }

    fn node_label(&'a self, n: &Node) -> dot::LabelText<'a> {
        dot::LabelText::label(n.label.clone())
    }

    fn edge_label(&'a self, e: &Edge) -> dot::LabelText<'a> {
        dot::LabelText::label(e.label.clone())
    }
}

impl<'a> dot::GraphWalk<'a, Node, Edge> for Graph {
    fn nodes(&'a self) -> Nodes<'a, Node> {
        Cow::Owned(self.nodes.clone())
    }

    fn edges(&'a self) -> Edges<'a, Edge> {
        Cow::Owned(self.edges.clone())
    }

    fn source(&'a self, edge: &Edge) -> Node {
        self.get_node(edge.from)
            .expect("Node at edge's start does not exist!")
            .clone()
    }

    fn target(&'a self, edge: &Edge) -> Node {
        self.get_node(edge.to)
            .expect("Node at edge's end does not exist!")
            .clone()
    }
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, label: &str, node_kind: NodeKind) -> usize {
        let node = Node::new(self.nodes.len(), label, node_kind);
        let id = node.id();
        self.nodes.push(node);
        id
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn get_node(&self, id: usize) -> Option<Node> {
        return if id < self.nodes.len() {
            Some(self.nodes[id].clone())
        } else {
            None
        };
    }

    pub fn find_local_fn_node(&self, id: HirId) -> Option<Node> {
        for node in &self.nodes {
            if let NodeKind::LocalFn {hir_id} = node.kind {
                if hir_id == id {
                    return Some(node.clone())
                }
            }
        }

        None
    }

    pub fn find_non_local_fn_node(&self, id: DefId) -> Option<Node> {
        for node in &self.nodes {
            if let NodeKind::NonLocalFn {def_id} = node.kind {
                if def_id == id {
                    return Some(node.clone())
                }
            }
        }

        None
    }

    pub fn to_dot(self) -> String {
        let mut buf = Vec::new();

        dot::render(&self, &mut buf).unwrap();

        String::from_utf8(buf).unwrap()
    }
}

impl Node {
    fn new(node_id: usize, label: &str, node_type: NodeKind) -> Self {
        Node {
            id: node_id,
            label: String::from(label),
            kind: node_type,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

impl NodeKind {
    pub fn local_fn(id: HirId) -> Self {
        NodeKind::LocalFn { hir_id: id }
    }

    pub fn non_local_fn(id: DefId) -> Self {
        NodeKind::NonLocalFn { def_id: id }
    }
}

impl Edge {
    pub fn new(from: usize, to: usize, call_id: HirId) -> Self {
        Edge {
            from,
            to,
            call_id,
            label: String::new(),
        }
    }

    pub fn set_label(&mut self, label: &str) {
        self.label = String::from(label);
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.kind == other.kind
    }
}

impl PartialEq for NodeKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NodeKind::LocalFn { hir_id: id1 }, NodeKind::LocalFn { hir_id: id2 }) => {
                id1 == id2
            }
            (NodeKind::NonLocalFn { def_id: id1 }, NodeKind::NonLocalFn { def_id: id2 }) => {
                id1 == id2
            }
            _ => false
        }
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.from == other.from
    }
}
