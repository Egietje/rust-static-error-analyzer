use crate::graph::Graph;

pub fn format(graph: Graph) -> Graph {
    //split_chains(graph)
    graph
}

fn split_chains(graph: Graph) -> Graph {
    // TODO: implement properly
    let mut new_graph = Graph::new(graph.crate_name.clone());

    // Loop over all nodes (e.g. functions)
    for node in &graph.nodes {
        // Loop over calls to this function
        for edge in graph.get_incoming_edges(node.id()) {
            // We only care about error edges for the chains
            if edge.is_error {

            }
        }
    }

    new_graph
}