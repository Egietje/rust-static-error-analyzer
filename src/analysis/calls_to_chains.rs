use crate::graph::{CallEdge, CallGraph, ChainGraph};
use std::collections::HashMap;

pub fn format(graph: &CallGraph) -> ChainGraph {
    split_chains(graph)
}

fn split_chains(graph: &CallGraph) -> ChainGraph {
    let mut new_graph = ChainGraph::new(graph.crate_name.clone());

    // Loop over all nodes (e.g. functions)
    for edge in &graph.edges {
        // Start of a chain
        if edge.is_error && !edge.propagates {
            let mut node_map: HashMap<usize, usize> = HashMap::new();
            for call in get_chain_from_edge(graph, edge) {
                let from = if node_map.contains_key(&call.from) {
                    node_map.get(&call.from).unwrap().clone()
                } else {
                    let id = new_graph.add_node(graph.nodes[call.from].label.clone());
                    node_map.insert(call.from, id);
                    id
                };

                let to = if node_map.contains_key(&call.to) {
                    node_map.get(&call.to).unwrap().clone()
                } else {
                    let id = new_graph.add_node(graph.nodes[call.to].label.clone());
                    node_map.insert(call.to, id);
                    id
                };

                new_graph.add_edge(from, to, call.ty);
            }
        }
    }

    new_graph
}

fn get_chain_from_edge(graph: &CallGraph, from: &CallEdge) -> Vec<CallEdge> {
    let mut res: Vec<CallEdge> = vec![];

    res.push(from.clone());

    for edge in graph.get_outgoing_edges(from.to) {
        if edge.is_error && edge.propagates {
            res.extend(get_chain_from_edge(graph, edge));
        }
    }

    res
}
