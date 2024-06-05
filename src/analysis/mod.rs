mod create_graph;
mod format_graph;
mod types;

use crate::graph::{ChainGraph, Graph};
use rustc_middle::ty::TyCtxt;

/// Analysis steps:
///
/// Step 1: Create call graph
/// Step 1.1: Node for each function
/// Step 1.2: Edge for each function call
/// Step 1.3: Add function call information (e.g. whether it propagates using the try op)
///
/// Step 2: Attach return type info to functions in call graph
/// Step 2.1: Loop over each edge in call graph
/// Step 2.2: Label edge with type info extracted from MIR
///
/// Step 3: Attach panic info to functions in call graph
/// NOTE: skipped due to lack of time
///
/// Step 4: Parse the output graph to show individual propagation chains
pub fn analyze(context: TyCtxt) -> (Graph, ChainGraph) {
    // Get the entry point of the program
    let entry_node = get_entry_node(context);

    // Create call graph
    let mut graph = create_graph::create_call_graph_from_root(context, entry_node.expect_item());

    // Attach return type info
    for edge in &mut graph.edges {
        let (ty, error) = types::get_error_or_type(
            context,
            edge.call_id,
            graph.nodes[edge.from].kind.def_id(),
            graph.nodes[edge.to].kind.def_id(),
        );
        edge.ty = Some(ty);
        edge.is_error = error;
    }

    // Format graph
    let formatted = format_graph::format(&graph);
    println!("{}", formatted.to_dot());

    (graph, formatted)
}

/// Retrieve the entry node (aka main function) from the type context.
fn get_entry_node(context: TyCtxt) -> rustc_hir::Node {
    let (def_id, _entry_type) = context
        .entry_fn(())
        .expect("Could not find entry function!");
    let id = context
        .local_def_id_to_hir_id(def_id.as_local().expect("Entry function def id not local!"));
    context.hir_node(id)
}
