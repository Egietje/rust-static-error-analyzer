use std::borrow::Cow;
use dot::{Edges, Nodes};
use rustc_hir::HirId;

#[derive(Debug, Clone)]
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl<'a> dot::Labeller<'a, Node, Edge> for Graph {
    fn graph_id(&'a self) -> dot::Id<'a> {
        dot::Id::new("call_graph").unwrap()
    }

    fn node_id(&'a self, n: &Node) -> dot::Id<'a> {
        let mut id = n.id.to_string().replace(":", "_");
        id.retain(|c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_".contains(c.clone()));
        dot::Id::new(id).unwrap()
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
        self.get_node(edge.from).expect("Node from edge not added to nodes list!").clone()
    }

    fn target(&'a self, edge: &Edge) -> Node {
        self.get_node(edge.to).expect("Node from edge not added to nodes list!").clone()
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    id: HirId,
    label: String,
}

#[derive(Debug, Clone)]
pub struct Edge {
    from: HirId,
    to: HirId,
    label: String,
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        if !self.nodes.contains(&node) {
            self.nodes.push(node);
        }
    }

    pub fn get_node(&self, id: HirId) -> Option<&Node> {
        for node in &self.nodes {
            if node.id == id {
                return Some(node);
            }
        }

        None
    }

    pub fn add_edge(&mut self, edge: Edge) {
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
    }

    pub fn to_dot(self) -> String {
        let mut buf = Vec::new();

        dot::render(&self, &mut buf).unwrap();

        String::from_utf8(buf).unwrap()
    }
}

impl Node {
    pub fn new(id: HirId, label: &str) -> Self {
        Node {
            id,
            label: String::from(label),
        }
    }

    pub fn id(&self) -> HirId {
        self.id
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.from == other.from
    }
}

impl Edge {
    pub fn new(from: &Node, to: &Node) -> Self {
        Edge {
            from: from.id(),
            to: to.id(),
            label: String::new(),
        }
    }

    pub fn label(mut self, label: &str) {
        self.label = String::from(label);
    }
}
