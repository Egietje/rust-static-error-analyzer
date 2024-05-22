use crate::graph::{Edge, Graph, Node, NodeKind};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Block, Expr, ExprKind, Item, ItemKind, Pat, PatKind, QPath, StmtKind, TyKind, HirId, FnRetTy, Ty};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

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
    let (def_id, _entry_type) = context.entry_fn(())?;
    let id = context.local_def_id_to_hir_id(def_id.as_local()?);
    let entry_node = context.hir_node(id);

    // Create call graph
    let mut graph = create_call_graph_from_root(context, entry_node.expect_item());

    // TODO: Attach return type info
    for edge in &mut graph.edges {
        //if let Some(ret) = get_return_type(context, &graph.nodes[edge.to]) {
            //edge.set_label(&return_type_to_label(context, ret));
        //}
    }

    // TODO: Error propagation chains

    // TODO: Attach panic info

    // TODO: Remove redundant nodes/edges

    Some(graph)
}

/// Create a call graph starting from the provided root node.
fn create_call_graph_from_root(context: TyCtxt, item: &Item) -> Graph {
    let mut graph = Graph::new();

    // Access the function
    if let ItemKind::Fn(_sig, _gen, id) = item.kind {
        // Create a node for the function
        let node = NodeKind::local_fn(item.hir_id());
        let node_id = graph.add_node(&get_full_name(context, &node), node);

        // Add edges/nodes for all functions called from within this function (and recursively do it for those functions as well)
        graph = add_calls_from_function(context, node_id, id.hir_id, graph);
    }

    return graph;
}

/// Retrieve all function calls within a function, and add the nodes and edges to the graph.
fn add_calls_from_function(context: TyCtxt, from_node: usize, fn_id: HirId, mut graph: Graph) -> Graph {
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
            NodeKind::LocalFn { hir_id } => {
                if let Some(node) = graph.find_local_fn_node(hir_id) {
                    // We have already encountered this local function, so just add the edge
                    graph.add_edge(Edge::new(from, node.id(), call_id));
                } else {
                    // We have not yet explored this local function, so add new node and edge,
                    // and explore it
                    let id = graph.add_node(&get_full_name(context, &node_kind), node_kind);

                    graph.add_edge(Edge::new(from, id, call_id));

                    graph = add_calls_from_function(context, id, hir_id, graph);
                }
            }
            NodeKind::NonLocalFn { def_id } => {
                if let Some(node) = graph.find_non_local_fn_node(def_id) {
                    // We have already encountered this non-local function, so just add the edge
                    graph.add_edge(Edge::new(from, node.id(), call_id));
                } else {
                    // We have not yet explored this non-local function, so add new node and edge
                    let id = graph.add_node(&get_full_name(context, &node_kind), node_kind);

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
            if let Some(def_id) = context.typeck(expr.hir_id.owner.def_id).type_dependent_def_id(expr.hir_id) {
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
            println!("{:?}", closure);
            let exp = context.hir_node(closure.body.hir_id).expect_expr();
            println!("{:?}", exp);
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

/// Retrieve a vec of all function calls made from within a pattern.
fn get_function_calls_in_pattern(context: TyCtxt, pat: &Pat) -> Vec<(NodeKind, HirId)> {
    let mut res: Vec<(NodeKind, HirId)> = vec![];

    match pat.kind {
        PatKind::Wild => {}
        PatKind::Binding(a, b, c, d) => {}
        PatKind::Struct(a, b, c) => {}
        PatKind::TupleStruct(a, b, c) => {}
        PatKind::Or(a) => {}
        PatKind::Never => {}
        PatKind::Path(a) => {}
        PatKind::Tuple(a, b) => {}
        PatKind::Box(a) => {}
        PatKind::Deref(a) => {}
        PatKind::Ref(a, b) => {}
        PatKind::Lit(a) => {}
        PatKind::Range(a, b, _end) => {
            if let Some(exp) = a {
                res.extend(get_function_calls_in_expression(context, exp));
            }
            if let Some(exp) = b {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        PatKind::Slice(a, b, c) => {}
        PatKind::Err(_err) => {}
    }

    res
}

fn get_node_kind_from_path(context: TyCtxt, qpath: QPath) -> Option<NodeKind> {
    match qpath {
        QPath::Resolved(_ty, path) => {
            if let Res::Def(kind, id) = path.res {
                if let DefKind::Fn = kind {
                    if let Some(local_id) = id.as_local() {
                        // Local function
                        let hir_id = context.local_def_id_to_hir_id(local_id);
                        let item = context.hir_node(hir_id).expect_item();
                        if let ItemKind::Fn(_sig, _gen, body) = item.kind {
                            return Some(NodeKind::local_fn(hir_id));
                        }
                    } else {
                        // Non-local function
                        return Some(NodeKind::non_local_fn(id));
                    }
                }
            }
        }
        QPath::TypeRelative(ty, segment) => {
            if let TyKind::Path(path) = ty.kind {
                if let QPath::Resolved(_ty, pat) = path {
                    if let Res::Def(_kind, id) = pat.res {
                        if let Some(local_id) = id.as_local() {
                            let hir_id = context.local_def_id_to_hir_id(local_id);
                            let item = context.hir_node(hir_id).expect_item();
                            if let ItemKind::Fn(_sig, _gen, body) = item.kind {
                                return Some(NodeKind::local_fn(hir_id));
                            }
                        }
                    }
                }
            }
        }
        QPath::LangItem(_, _) => {}
    }

    None
}

/// Get a string of a path from its path segments, including the crate name (e.g. crate::main)
fn get_full_name(context: TyCtxt, node_kind: &NodeKind) -> String {
    match node_kind {
        NodeKind::LocalFn { hir_id } => {
            get_fn_path(context, hir_id.owner.def_id.to_def_id())
        }
        NodeKind::NonLocalFn { def_id } => {
            get_fn_path(context, def_id.clone())
        }
    }
}

fn get_fn_path(context: TyCtxt, def_id: DefId) -> String {
    let path: Vec<String> = get_fn_path_rec(context, def_id).into_iter().rev().collect();

    let mut res = String::new();

    for i in 0..path.len() {
        res.push_str(&path[i]);
        if i != path.len() - 1 {
            res.push_str("::");
        }
    }

    res
}

fn get_fn_path_rec(context: TyCtxt, def_id: DefId) -> Vec<String> {
    let mut res = Vec::new();

    let ident = get_identifier(context, def_id);
    if let Some(id) = ident {
        res.push(id);
    }

    if let Some(parent) = context.opt_parent(def_id) {
        res.extend(get_fn_path_rec(context, parent));
    }

    res
}

fn get_identifier(context: TyCtxt, def_id: DefId) -> Option<String> {
    context.opt_item_name(def_id).map(|s| s.to_ident_string())
}

fn return_type_to_label(context: TyCtxt, ret_type: FnRetTy) -> String {
    match ret_type {
        FnRetTy::DefaultReturn(_) => { String::from("()") }
        FnRetTy::Return(ty) => {
            if let Some(def_id) = get_type_def(context, ty) {
                format!("{:?}", context.type_of(def_id))
            } else {
                String::new()
            }
        }
    }
}

fn get_type_def(context: TyCtxt, ty: &Ty) -> Option<DefId> {
    match ty.kind {
        TyKind::Path(qpath) => {
            match qpath {
                QPath::Resolved(_ty, path) => {
                    match path.res {
                        Res::Def(_kind, def_id) => {Some(def_id)}
                        Res::SelfTyParam { trait_ } => {Some(trait_)}
                        Res::SelfTyAlias { alias_to, .. } => {Some(alias_to)}
                        Res::SelfCtor(def_id) => {Some(def_id)}
                        Res::Local(hir_id) => {Some(hir_id.owner.to_def_id())}
                        _ => None
                    }
                }
                QPath::TypeRelative(_, _) => {None}
                QPath::LangItem(_, _) => {None}
            }
        }
        _ => None
    }
}

fn get_return_type<'a>(context: TyCtxt<'a>, node: &Node) -> Option<FnRetTy<'a>> {
    match node.kind {
        NodeKind::LocalFn { hir_id } => {
            let item = context.hir_node(hir_id).expect_item();
            if let ItemKind::Fn(sig, gen, body) = item.kind {
                println!("{:?}", sig.decl.output);
                Some(sig.decl.output)
            } else {
                None
            }
        }
        NodeKind::NonLocalFn { def_id } => {
            println!("{:?}", context.type_of(def_id));
            None
        }
    }
}
