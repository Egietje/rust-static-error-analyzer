use crate::graph::{CallEdge, CallGraph, ChainGraph};
use std::collections::HashMap;

pub fn to_chains(graph: &CallGraph) -> ChainGraph {
    let mut new_graph = ChainGraph::new(graph.crate_name.clone());

    // Loop over all nodes (e.g. functions)
    for edge in &graph.edges {
        // Start of a chain
        if edge.is_error && !edge.propagates {
            let mut node_map: HashMap<usize, usize> = HashMap::new();

            let mut calls = get_chain_from_edge(graph, edge);
            calls.push(edge.clone());

            for call in calls {
                // If we've already added the node to the new graph, refer to that, otherwise, add a new node
                let from = if node_map.contains_key(&call.from) {
                    node_map.get(&call.from).unwrap().clone()
                } else {
                    let id = new_graph.add_node(graph.nodes[call.from].label.clone());
                    node_map.insert(call.from, id);
                    id
                };

                // Ditto
                let to = if node_map.contains_key(&call.to) {
                    node_map.get(&call.to).unwrap().clone()
                } else {
                    let id = new_graph.add_node(graph.nodes[call.to].label.clone());
                    node_map.insert(call.to, id);
                    id
                };

                // Add the edge
                new_graph.add_edge(from, to, call.ty);
            }
        }
    }

    new_graph
}

fn get_chain_from_edge(graph: &CallGraph, from: &CallEdge) -> Vec<CallEdge> {
    let mut res: Vec<CallEdge> = vec![];

    // Add all outgoing propagating error edges from the 'to' node to the list
    // And do the same once for each node this edge calls to
    for edge in graph.get_outgoing_edges(from.to) {
        if edge.is_error && edge.propagates {
            if !res.contains(edge) {
                // If we haven't had this edge yet, explore the node
                res.push(edge.clone());
                res.extend(get_chain_from_edge(graph, edge));
            } else {
                // Otherwise just add the edge
                res.push(edge.clone());
            }
        }
    }

    res
}
