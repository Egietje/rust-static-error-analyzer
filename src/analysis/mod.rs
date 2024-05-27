mod create_graph;
mod types;

use crate::graph::Graph;
use rustc_middle::ty::TyCtxt;

/// Analysis steps:
///
/// Step 1: Create call graph (directional)
/// Step 1.1: Node for each function (store def id and/or body id)
/// Step 1.2: Edge for each function call
///
/// Step 2: Attach return type info to functions in call graph (only if it's of type Result?)
/// Step 2.1: Loop over each function/node in call graph
/// Step 2.2: Label incoming edges of this node (e.g. calls to this function) with return type retrieved using def id
///
/// Step 3: Investigate functions that call error functions (whether it handles or propagates)
/// Step 3.1: Basic version: if calls error function and returns error, assume propagates
/// Step 3.2: Basic version: if calls error function and doesn't return error, assume handles
/// Step 3.3: Advanced version: not sure
///
/// Step 4: Attach panic info to functions in call graph
///
/// Step 5: Remove functions that don't error/panic from graph
pub fn analyze(context: TyCtxt) -> Graph {
    // Get the entry point of the program
    let entry_node = get_entry_node(context);

    // Create call graph
    let mut graph = create_graph::create_call_graph_from_root(context, entry_node.expect_item());

    // Attach return type info
    for edge in &mut graph.edges {
        let ret_ty = types::get_call_type(context, edge.call_id);
        edge.ty = ret_ty.map(|t| format!("{t}"));
    }

    // TODO: Investigate functions that call error functions

    // TODO: Attach panic info

    // TODO: Remove redundant nodes/edges
    for i in (0..graph.edges.len()).rev() {
        let edge = &graph.edges[i];
        if let Some(ty) = &edge.ty {
            if !ty.starts_with("std::result::Result<") && !graph.nodes[edge.to].panics {
                graph.edges.remove(i);
            }
        } else {
            graph.edges.remove(i);
        }
    }

    graph
}

fn get_entry_node(context: TyCtxt) -> rustc_hir::Node {
    let (def_id, _entry_type) = context
        .entry_fn(())
        .expect("Could not find entry function!");
    let id = context
        .local_def_id_to_hir_id(def_id.as_local().expect("Entry function def id not local!"));
    context.hir_node(id)
}
