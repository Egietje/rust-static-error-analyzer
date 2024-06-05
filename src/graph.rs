use dot::{Edges, Id, Kind, LabelText, Nodes, Style};
use rustc_hir::def_id::DefId;
use rustc_hir::HirId;
use std::borrow::Cow;
use std::cmp::PartialEq;

#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub crate_name: String,
}

#[derive(Debug, Clone)]
pub struct Node {
    id: usize,
    pub label: String,
    pub kind: NodeKind,
    pub panics: bool,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    LocalFn(DefId, HirId),
    NonLocalFn(DefId),
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub call_id: HirId,
    pub ty: Option<String>,
    pub propagates: bool,
    pub is_error: bool,
}

impl<'a> dot::Labeller<'a, Node, Edge> for Graph {
    fn graph_id(&self) -> Id<'a> {
        let mut name: String = self.crate_name.clone();
        name.retain(|e| e.is_ascii_alphanumeric() || e == '_');
        Id::new(format!("error_propagation_{name}")).unwrap()
    }

    fn node_id(&self, n: &Node) -> Id<'a> {
        Id::new(format!("n{:?}", n.id)).unwrap()
    }

    fn node_label(&self, n: &Node) -> LabelText<'a> {
        LabelText::label(n.label.clone())
    }

    fn edge_label(&self, e: &Edge) -> LabelText<'a> {
        LabelText::label(e.ty.clone().unwrap_or(String::from("unknown")))
    }

    fn node_color(&'a self, n: &Node) -> Option<LabelText<'a>> {
        if n.panics {
            Some(LabelText::label("red"))
        } else {
            None
        }
    }

    fn edge_color(&'a self, e: &Edge) -> Option<LabelText<'a>> {
        if e.is_error && e.propagates {
            Some(LabelText::label("purple"))
        } else if e.is_error {
            Some(LabelText::label("red"))
        } else if e.propagates {
            Some(LabelText::label("blue"))
        } else {
            None
        }
    }

    fn edge_style(&'a self, e: &Edge) -> Style {
        if e.is_error || e.propagates {
            Style::None
        } else {
            Style::Dotted
        }
    }

    fn kind(&self) -> Kind {
        Kind::Digraph
    }
}

impl<'a> dot::GraphWalk<'a, Node, Edge> for Graph {
    fn nodes(&'a self) -> Nodes<'a, Node> {
        let mut nodes = vec![];
        for edge in &self.edges {
            if !nodes.contains(&self.nodes[edge.from]) {
                nodes.push(self.nodes[edge.from].clone());
            }
            if !nodes.contains(&self.nodes[edge.to]) {
                nodes.push(self.nodes[edge.to].clone());
            }
        }
        Cow::Owned(nodes)
    }

    fn edges(&'a self) -> Edges<'a, Edge> {
        Cow::Owned(self.edges.clone())
    }

    fn source(&'a self, edge: &Edge) -> Node {
        self.nodes[edge.from].clone()
    }

    fn target(&'a self, edge: &Edge) -> Node {
        self.nodes[edge.to].clone()
    }
}

#[derive(Debug, Clone)]
pub struct ChainGraph {
    pub nodes: Vec<ChainNode>,
    pub edges: Vec<ChainEdge>,
    pub crate_name: String,
}

#[derive(Debug, Clone)]
pub struct ChainNode {
    id: usize,
    label: String,
}

#[derive(Debug, Clone)]
pub struct ChainEdge {
    from: usize,
    to: usize,
    label: Option<String>,
}

impl<'a> dot::Labeller<'a, ChainNode, ChainEdge> for ChainGraph {
    fn graph_id(&'a self) -> Id<'a> {
        let mut name: String = self.crate_name.clone();
        name.retain(|e| e.is_ascii_alphanumeric() || e == '_');
        Id::new(format!("error_propagation_{name}_chains")).unwrap()
    }

    fn node_id(&'a self, n: &ChainNode) -> Id<'a> {
        Id::new(format!("n{:?}", n.id)).unwrap()
    }

    fn node_label(&self, n: &ChainNode) -> LabelText<'a> {
        LabelText::label(n.label.clone())
    }

    fn edge_label(&self, e: &ChainEdge) -> LabelText<'a> {
        LabelText::label(e.label.clone().unwrap_or(String::from("unknown")))
    }
}

impl<'a> dot::GraphWalk<'a, ChainNode, ChainEdge> for ChainGraph {
    fn nodes(&'a self) -> Nodes<'a, ChainNode> {
        let mut nodes = vec![];
        for edge in &self.edges {
            if !nodes.contains(&self.nodes[edge.from]) {
                nodes.push(self.nodes[edge.from].clone());
            }
            if !nodes.contains(&self.nodes[edge.to]) {
                nodes.push(self.nodes[edge.to].clone());
            }
        }
        Cow::Owned(nodes)
    }

    fn edges(&'a self) -> Edges<'a, ChainEdge> {
        Cow::Owned(self.edges.clone())
    }

    fn source(&'a self, edge: &ChainEdge) -> ChainNode {
        self.nodes[edge.to].clone()
    }

    fn target(&'a self, edge: &ChainEdge) -> ChainNode {
        self.nodes[edge.from].clone()
    }
}

impl Graph {
    /// Create a new, empty graph.
    pub fn new(crate_name: String) -> Self {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            crate_name,
        }
    }

    /// Add a node to this graph, returning its id.
    pub fn add_node(&mut self, label: &str, node_kind: NodeKind) -> usize {
        let node = Node::new(self.nodes.len(), label, node_kind);
        let id = node.id();
        self.nodes.push(node);
        id
    }

    /// Add an edge between two nodes to this graph.
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Find a node of `LocalFn` kind.
    pub fn find_local_fn_node(&self, id: HirId) -> Option<Node> {
        for node in &self.nodes {
            if let NodeKind::LocalFn(_def_id, hir_id) = node.kind {
                if hir_id == id {
                    return Some(node.clone());
                }
            }
        }

        None
    }

    /// Find a node of `NonLocalFn` kind.
    pub fn find_non_local_fn_node(&self, id: DefId) -> Option<Node> {
        for node in &self.nodes {
            if let NodeKind::NonLocalFn(def_id) = node.kind {
                if def_id == id {
                    return Some(node.clone());
                }
            }
        }

        None
    }

    pub fn get_outgoing_edges(&self, node_id: usize) -> Vec<&Edge> {
        let mut res = vec![];

        for edge in &self.edges {
            if edge.from == node_id {
                res.push(edge);
            }
        }

        res
    }

    /// Convert this graph to dot representation.
    pub fn to_dot(&self) -> String {
        let mut buf = Vec::new();

        dot::render(self, &mut buf).unwrap();

        String::from_utf8(buf).unwrap()
    }
}

impl Node {
    /// Create a new node.
    fn new(node_id: usize, label: &str, node_type: NodeKind) -> Self {
        Node {
            id: node_id,
            label: String::from(label),
            kind: node_type,
            panics: false,
        }
    }

    /// Get the id of this node.
    pub fn id(&self) -> usize {
        self.id
    }
}

impl NodeKind {
    /// Get a new `LocalFn`.
    pub fn local_fn(def_id: DefId, hir_id: HirId) -> Self {
        NodeKind::LocalFn(def_id, hir_id)
    }

    /// Get a new `NonLocalFn`.
    pub fn non_local_fn(id: DefId) -> Self {
        NodeKind::NonLocalFn(id)
    }

    /// Extract the `DefId` from this node.
    pub fn def_id(&self) -> DefId {
        match self {
            NodeKind::LocalFn(def_id, _hir_id) => *def_id,
            NodeKind::NonLocalFn(def_id) => *def_id,
        }
    }
}

impl Edge {
    /// Create a new edge.
    pub fn new(from: usize, to: usize, call_id: HirId, propagates: bool) -> Self {
        Edge {
            from,
            to,
            call_id,
            ty: None,
            propagates,
            is_error: false,
        }
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
            (NodeKind::LocalFn(def_id1, hir_id1), NodeKind::LocalFn(def_id2, hir_id2)) => {
                def_id1 == def_id2 && hir_id1 == hir_id2
            }
            (NodeKind::NonLocalFn(id1), NodeKind::NonLocalFn(id2)) => id1 == id2,
            _ => false,
        }
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.from == other.from
    }
}

impl ChainGraph {
    /// Create a new, empty graph.
    pub fn new(crate_name: String) -> Self {
        ChainGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            crate_name,
        }
    }

    pub fn add_node(&mut self, label: String) -> usize {
        let id = self.nodes.len();

        self.nodes.push(ChainNode::new(id, label));

        id
    }

    pub fn add_edge(&mut self, from: usize, to: usize, label: Option<String>) {
        self.edges.push(ChainEdge::new(from, to, label));
    }

    /// Convert this graph to dot representation.
    pub fn to_dot(&self) -> String {
        let mut buf = Vec::new();

        dot::render(self, &mut buf).unwrap();

        String::from_utf8(buf).unwrap()
    }
}

impl ChainNode {
    /// Create a new node.
    fn new(id: usize, label: String) -> Self {
        ChainNode { id, label }
    }
}

impl ChainEdge {
    /// Create a new edge.
    pub fn new(from: usize, to: usize, label: Option<String>) -> Self {
        ChainEdge { from, to, label }
    }
}

impl PartialEq for ChainNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq for ChainEdge {
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.from == other.from
    }
}
