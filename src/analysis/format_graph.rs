use crate::graph::{ChainGraph, Edge, Graph};

pub fn format(graph: &Graph) -> ChainGraph {
    split_chains(graph)
}

fn split_chains(graph: &Graph) -> ChainGraph {
    let mut new_graph = ChainGraph::new(graph.crate_name.clone());

    // Loop over all nodes (e.g. functions)
    for edge in &graph.edges {
        // Start of a chain
        if edge.is_error && !edge.propagates {
            for chain in find_chains(graph, edge) {
                let mut prev = new_graph.add_node(graph.nodes[edge.from].label.clone());
                for chain_edge in chain {
                    let new = new_graph.add_node(graph.nodes[chain_edge.to].label.clone());
                    new_graph.add_edge(prev, new, chain_edge.ty);
                    prev = new;
                }
            }
        }
    }

    new_graph
}

fn find_chains(graph: &Graph, start_edge: &Edge) -> Vec<Vec<Edge>> {
    let mut res: Vec<Vec<Edge>> = vec![];

    for edge in graph.get_outgoing_edges(start_edge.to) {
        if edge.is_error && edge.propagates {
            let vec = vec![start_edge.clone()];
            let chains = find_chains(graph, edge);
            for chain in chains {
                let mut new = vec.clone();
                new.extend(chain);
                res.push(new);
            }
        }
    }

    if res.is_empty() {
        res.push(vec![start_edge.clone()]);
    }

    res
}
