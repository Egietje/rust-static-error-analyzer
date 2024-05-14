use rustc_hir::HirId;

#[derive(Debug, Clone)]
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
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
}

impl Node {
    pub fn new(id: HirId, label: &str) -> Self{
        Node { id, label: String::from(label) }
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
        Edge { from: from.id(), to: to.id(), label: String::new() }
    }

    pub fn label(mut self, label: &str) {
        self.label = String::from(label);
    }
}
