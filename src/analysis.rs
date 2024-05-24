use crate::graph::{Edge, Graph, Node, NodeKind};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{
    Block, Expr, ExprKind, HirId, Item, ItemKind, Pat, PatKind, QPath, StmtKind, TyKind,
};
use rustc_middle::ty::{Ty, TyCtxt};

/// Analysis steps:
///
/// Step 1: Create call graph (directional)
/// Step 1.1: Node for each function (store def id and/or body id)
/// Step 1.2: Edge for each function call
/// Step 1.3: Look into how concurrency plays into all this
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
pub fn analyze(context: TyCtxt) -> Option<Graph> {
    // Get the entry point of the program
    let entry_node = get_entry_node(context);

    // Create call graph
    let mut graph = create_call_graph_from_root(context, entry_node.expect_item());

    // Attach return type info
    for node in &graph.nodes.clone() {
        let ret_ty = get_return_type(context, node);
        if let Some(ty) = ret_ty {
            for edge in graph.incoming_edges(node) {
                edge.set_label(&format!("{ty:?}"));
                println!("{}", is_result_type(ty));
            }
        }
    }

    // TODO: Error propagation chains

    // TODO: Attach panic info

    // TODO: Remove redundant nodes/edges

    Some(graph)
}

fn get_entry_node(context: TyCtxt) -> rustc_hir::Node {
    let (def_id, _entry_type) = context
        .entry_fn(())
        .expect("Could not find entry function!");
    let id = context
        .local_def_id_to_hir_id(def_id.as_local().expect("Entry function def id not local!"));
    context.hir_node(id)
}

/// Create a call graph starting from the provided root node.
fn create_call_graph_from_root(context: TyCtxt, item: &Item) -> Graph {
    let mut graph = Graph::new();

    // Access the function
    if let ItemKind::Fn(_sig, _gen, id) = item.kind {
        // Create a node for the function
        let node = NodeKind::local_fn(item.hir_id());
        let node_id = graph.add_node(&get_path_string(context, node.def_id()), node);

        // Add edges/nodes for all functions called from within this function (and recursively do it for those functions as well)
        graph = add_calls_from_function(context, node_id, id.hir_id, graph);
    }

    return graph;
}

/// Retrieve all function calls within a function, and add the nodes and edges to the graph.
fn add_calls_from_function(
    context: TyCtxt,
    from_node: usize,
    fn_id: HirId,
    mut graph: Graph,
) -> Graph {
    let node = context.hir_node(fn_id);

    // Access the code block of the function (might be wrapped in expr)
    match node {
        rustc_hir::Node::Expr(expr) => {
            if let ExprKind::Block(block, _) = expr.kind {
                graph = add_calls_from_block(context, from_node, block, graph);
            }
        }
        rustc_hir::Node::Block(block) => {
            graph = add_calls_from_block(context, from_node, block, graph);
        }
        rustc_hir::Node::Item(item) => {
            if let ItemKind::Fn(_sig, _gen, id) = item.kind {
                graph = add_calls_from_function(context, from_node, id.hir_id, graph);
            }
        }
        _ => {}
    }

    return graph;
}

/// Retrieve all function calls within a block, and add the nodes and edges to the graph.
fn add_calls_from_block(context: TyCtxt, from: usize, block: &Block, mut graph: Graph) -> Graph {
    // Get the function calls from within this block
    let calls = get_function_calls_in_block(context, block);

    // Add edges for all function calls
    for (node_kind, call_id) in calls {
        match node_kind {
            NodeKind::LocalFn(hir_id) => {
                if let Some(node) = graph.find_local_fn_node(hir_id) {
                    // We have already encountered this local function, so just add the edge
                    graph.add_edge(Edge::new(from, node.id(), call_id));
                } else {
                    // We have not yet explored this local function, so add new node and edge,
                    // and explore it.
                    let id = graph.add_node(&get_path_string(context, node_kind.def_id()), node_kind);

                    graph.add_edge(Edge::new(from, id, call_id));

                    graph = add_calls_from_function(context, id, hir_id, graph);
                }
            }
            NodeKind::NonLocalFn(def_id) => {
                if let Some(node) = graph.find_non_local_fn_node(def_id) {
                    // We have already encountered this non-local function, so just add the edge
                    graph.add_edge(Edge::new(from, node.id(), call_id));
                } else {
                    // We have not yet explored this non-local function, so add new node and edge
                    let id = graph.add_node(&get_path_string(context, node_kind.def_id()), node_kind);

                    graph.add_edge(Edge::new(from, id, call_id));
                }
            }
        }
    }

    return graph;
}

/// Retrieve a vec of all function calls made within the body of a block.
fn get_function_calls_in_block(context: TyCtxt, block: &Block) -> Vec<(NodeKind, HirId)> {
    let mut res: Vec<(NodeKind, HirId)> = vec![];

    // If the block has an ending expression add calls from there
    if let Some(exp) = block.expr {
        res.extend(get_function_calls_in_expression(context, exp));
    }

    // Go over all statements in the block
    for statement in block.stmts {
        // Match the kind of statement
        match statement.kind {
            StmtKind::Let(stmt) => {
                if let Some(exp) = stmt.init {
                    res.extend(get_function_calls_in_expression(context, exp));
                }
            }
            StmtKind::Item(_id) => {
                // No function calls here
            }
            StmtKind::Expr(exp) => {
                res.extend(get_function_calls_in_expression(context, exp));
            }
            StmtKind::Semi(exp) => {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
    }

    res
}

/// Retrieve a vec of all function calls made within an expression.
fn get_function_calls_in_expression(context: TyCtxt, expr: &Expr) -> Vec<(NodeKind, HirId)> {
    let mut res: Vec<(NodeKind, HirId)> = vec![];
    // Match the kind of expression
    match expr.kind {
        ExprKind::ConstBlock(block) => {
            let node = context.hir_node(block.body.hir_id);
            res.extend(get_function_calls_in_block(context, node.expect_block()));
        }
        ExprKind::Array(args) => {
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Call(func, args) => {
            if let ExprKind::Path(qpath) = func.kind {
                if let Some(node) = get_node_kind_from_path(context, qpath) {
                    res.push((node, expr.hir_id));
                }
            }
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::MethodCall(_path, exp, args, _span) => {
            if let Some(def_id) = context
                .typeck(expr.hir_id.owner.def_id)
                .type_dependent_def_id(expr.hir_id)
            {
                res.push((NodeKind::non_local_fn(def_id), expr.hir_id));
            }
            res.extend(get_function_calls_in_expression(context, exp));
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Tup(args) => {
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Binary(_op, a, b) => {
            res.extend(get_function_calls_in_expression(context, a));
            res.extend(get_function_calls_in_expression(context, b));
        }
        ExprKind::Unary(_op, exp) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Lit(_lit) => {
            // No function calls here
        }
        ExprKind::Cast(exp, _ty) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Type(exp, _ty) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::DropTemps(exp) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Let(exp) => {
            res.extend(get_function_calls_in_expression(context, exp.init));
        }
        ExprKind::If(a, b, c) => {
            res.extend(get_function_calls_in_expression(context, a));
            res.extend(get_function_calls_in_expression(context, b));
            if let Some(exp) = c {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Loop(block, _lbl, _src, _span) => {
            res.extend(get_function_calls_in_block(context, block));
        }
        ExprKind::Match(exp, arms, _src) => {
            // TODO: this is the result of try op (?)
            res.extend(get_function_calls_in_expression(context, exp));
            for arm in arms {
                res.extend(get_function_calls_in_expression(context, arm.body));
                if let Some(guard) = arm.guard {
                    res.extend(get_function_calls_in_expression(context, guard));
                }
                res.extend(get_function_calls_in_pattern(context, arm.pat));
            }
        }
        ExprKind::Closure(closure) => {
            // TODO verify this is correct
            let exp = context.hir_node(closure.body.hir_id).expect_expr();
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Block(block, _lbl) => {
            res.extend(get_function_calls_in_block(context, block));
        }
        ExprKind::Assign(a, b, _span) => {
            res.extend(get_function_calls_in_expression(context, a));
            res.extend(get_function_calls_in_expression(context, b));
        }
        ExprKind::AssignOp(_op, a, b) => {
            res.extend(get_function_calls_in_expression(context, a));
            res.extend(get_function_calls_in_expression(context, b));
        }
        ExprKind::Field(exp, _ident) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Index(a, b, _span) => {
            res.extend(get_function_calls_in_expression(context, a));
            res.extend(get_function_calls_in_expression(context, b));
        }
        ExprKind::Path(_path) => {
            // No function calls here
        }
        ExprKind::AddrOf(_borrow, _mut, exp) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Break(_dest, opt) => {
            if let Some(exp) = opt {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Continue(_dest) => {
            // No function calls here
        }
        ExprKind::Ret(opt) => {
            if let Some(exp) = opt {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Become(exp) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::InlineAsm(_asm) => {
            // No function calls here
        }
        ExprKind::OffsetOf(_ty, _ids) => {
            // No function calls here
        }
        ExprKind::Struct(_path, args, base) => {
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp.expr));
            }
            if let Some(exp) = base {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Repeat(exp, _len) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Yield(exp, _src) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::Err(_err) => {
            // No function calls here
        }
    }

    res
}

/// Retrieve a vec of all function calls made from within a pattern (although I think it can never contain one).
fn get_function_calls_in_pattern(context: TyCtxt, pat: &Pat) -> Vec<(NodeKind, HirId)> {
    let mut res: Vec<(NodeKind, HirId)> = vec![];

    match pat.kind {
        PatKind::Wild => {
            // No function calls here
        }
        PatKind::Binding(_mode, _hir_id, _ident, opt_pat) => {
            if let Some(p) = opt_pat {
                res.extend(get_function_calls_in_pattern(context, p));
            }
        }
        PatKind::Struct(_path, fields, _other) => {
            for field in fields {
                res.extend(get_function_calls_in_pattern(context, field.pat));
            }
        }
        PatKind::TupleStruct(_path, pats, _pos) => {
            for p in pats {
                res.extend(get_function_calls_in_pattern(context, p));
            }
        }
        PatKind::Or(pats) => {
            for p in pats {
                res.extend(get_function_calls_in_pattern(context, p));
            }
        }
        PatKind::Never => {
            // No function calls here
        }
        PatKind::Path(_path) => {
            // No function calls here
        }
        PatKind::Tuple(pats, _pos) => {
            for p in pats {
                res.extend(get_function_calls_in_pattern(context, p));
            }
        }
        PatKind::Box(p) => {
            res.extend(get_function_calls_in_pattern(context, p));
        }
        PatKind::Deref(p) => {
            res.extend(get_function_calls_in_pattern(context, p));
        }
        PatKind::Ref(p, _mut) => {
            res.extend(get_function_calls_in_pattern(context, p));
        }
        PatKind::Lit(exp) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        PatKind::Range(a, b, _end) => {
            if let Some(exp) = a {
                res.extend(get_function_calls_in_expression(context, exp));
            }
            if let Some(exp) = b {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        PatKind::Slice(pats1, opt_pat, pats2) => {
            for p in pats1 {
                res.extend(get_function_calls_in_pattern(context, p));
            }
            if let Some(p) = opt_pat {
                res.extend(get_function_calls_in_pattern(context, p));
            }
            for p in pats2 {
                res.extend(get_function_calls_in_pattern(context, p));
            }
        }
        PatKind::Err(_err) => {
            // No function calls here
        }
    }

    res
}

fn get_node_kind_from_path(context: TyCtxt, qpath: QPath) -> Option<NodeKind> {
    match qpath {
        QPath::Resolved(_ty, path) => {
            if let Res::Def(kind, id) = path.res {
                if let DefKind::Fn = kind {
                    return Some(get_node_kind_from_def_id(context, id));
                }
            }
        }
        QPath::TypeRelative(ty, _segment) => {
            if let TyKind::Path(path) = ty.kind {
                if let QPath::Resolved(_ty, pat) = path {
                    if let Res::Def(_kind, id) = pat.res {
                        return Some(get_node_kind_from_def_id(context, id));
                    }
                }
            }
        }
        QPath::LangItem(_, _) => {}
    }

    None
}

fn get_node_kind_from_def_id(context: TyCtxt, def_id: DefId) -> NodeKind {
    return if let Some(local_id) = def_id.as_local() {
        let hir_id = context.local_def_id_to_hir_id(local_id);
        NodeKind::local_fn(hir_id)
    } else {
        NodeKind::non_local_fn(def_id)
    }
}

fn get_path_string(context: TyCtxt, def_id: DefId) -> String {
    format!(
        "{}{}",
        context.crate_name(context.def_path(def_id).krate),
        context.def_path(def_id).to_string_no_crate_verbose()
    )
}

fn is_result_type(ty: Ty) -> bool {
    format!("{}", ty).starts_with("std::result::Result<")
}

fn get_return_type<'a>(context: TyCtxt<'a>, node: &Node) -> Option<Ty<'a>> {
    if !context.is_mir_available(node.def_id()) {
        return None;
    }

    Some(context
        .optimized_mir(node.def_id())
        .bound_return_ty()
        .skip_binder()
    )
}
