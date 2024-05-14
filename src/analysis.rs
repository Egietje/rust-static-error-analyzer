use crate::graph::{Edge, Graph, Node};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{
    Block, Expr, ExprKind, Item, ItemKind, Pat, PatKind, PathSegment, QPath, StmtKind, CRATE_HIR_ID,
};
use rustc_middle::ty::TyCtxt;

// Analysis steps:

// Step 1: Create call graph (directional)
// Step 1.1: Node for each function (store def id and/or body id)
// Step 1.2: Edge for each function call

// Step 2: Attach return type info to functions in call graph (only if it's of type Result?)
// Step 2.1: Loop over each function/node in call graph
// Step 2.2: Label incoming edges of this node (e.g. calls to this function) with return type retrieved using def id

// Step 3: Investigate functions that call error functions (whether it handles or propagates)
// Step 3.1: Basic version: if calls error function and returns error, assume propagates
// Step 3.2: Basic version: if calls error function and doesn't return error, assume handles
// Step 3.3: Advanced version: not sure

// Step 4: Attach panic info to functions in call graph

// Step 5: Remove functions that don't error/panic from graph

pub fn analyze(context: TyCtxt) -> Option<Graph> {
    // Get the crate's root node
    let root_node = context.hir_node(CRATE_HIR_ID).expect_crate();

    // Go over all items defined in the crate's root
    for item_id in root_node.item_ids {
        // Get the node of each item
        let item_node = context.hir_node(item_id.hir_id());

        // Start analyzing at the main function
        if !item_node
            .ident()
            .is_some_and(|ident| ident.name.as_str().eq("main"))
        {
            continue;
        }

        // Create call graph
        let graph = create_call_graph_from_root(context, item_node.expect_item());

        // Attach return type info

        // Error propagation chains

        // Attach panic info

        // Remove redundant nodes

        return Some(graph);
    }

    None
}

fn create_call_graph_from_root(context: TyCtxt, item: &Item) -> Graph {
    let mut graph = Graph::new();

    if let ItemKind::Fn(_sig, _gen, id) = item.kind {
        let node = Node::new(id.hir_id, item.ident.as_str());

        graph = add_calls_from_function(context, node, graph);
    }

    return graph;
}

fn add_calls_from_function(context: TyCtxt, from: Node, mut graph: Graph) -> Graph {
    let node = context.hir_node(from.id());

    match node {
        rustc_hir::Node::Expr(expr) => {
            if let ExprKind::Block(block, _) = expr.kind {
                graph = add_calls_from_block(context, from, block, graph);
            }
        }
        rustc_hir::Node::Block(block) => {
            graph = add_calls_from_block(context, from, block, graph);
        }
        _ => {}
    }

    return graph;
}

fn add_calls_from_block(context: TyCtxt, from: Node, block: &Block, mut graph: Graph) -> Graph {
    let calls = get_function_calls_in_block(context, block);

    for node in calls {
        graph.add_edge(Edge::new(&from, &node));

        if graph.get_node(node.id()).is_none() {
            graph.add_node(node.clone());

            graph = add_calls_from_function(context, node, graph);
        }
    }

    return graph;
}

/// Retrieve a vec of all function calls made within the body of a block.
fn get_function_calls_in_block(context: TyCtxt, block: &Block) -> Vec<Node> {
    let mut res: Vec<Node> = vec![];

    for statement in block.stmts {
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
fn get_function_calls_in_expression(context: TyCtxt, expr: &Expr) -> Vec<Node> {
    let mut res: Vec<Node> = vec![];
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
            // TODO: verify this is correct/covers every case (probably not?)
            if let ExprKind::Path(qpath) = func.kind {
                if let QPath::Resolved(_ty, path) = qpath {
                    if let Res::Def(kind, id) = path.res {
                        if let DefKind::Fn = kind {
                            if let Some(local_id) = id.as_local() {
                                let hir_id = context.local_def_id_to_hir_id(local_id);
                                let item = context.hir_node(hir_id).expect_item();
                                if let ItemKind::Fn(_sig, _gen, body) = item.kind {
                                    res.push(Node::new(body.hir_id, &name(context, path.segments)))
                                }
                            }
                        }
                    }
                }
            }
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::MethodCall(path, method, args, span) => {
            // TODO: add node
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
            let block = context.hir_node(closure.body.hir_id).expect_block();
            res.extend(get_function_calls_in_block(context, block));
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

fn get_function_calls_in_pattern(context: TyCtxt, pat: &Pat) -> Vec<Node> {
    let mut res: Vec<Node> = vec![];

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

fn name(context: TyCtxt, segments: &[PathSegment]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let crate_num = segments
        .first()
        .unwrap()
        .hir_id
        .owner
        .def_id
        .to_def_id()
        .krate;

    let mut res = context.crate_name(crate_num).to_ident_string();

    for segment in segments {
        res.push_str("::");
        res.push_str(segment.ident.name.as_str());
    }

    res
}
