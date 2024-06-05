use crate::graph::{Edge, Graph, NodeKind};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::{
    Block, Expr, ExprKind, HirId, ImplItemKind, Item, ItemKind, MatchSource, Pat, PatKind, QPath,
    StmtKind, TyKind,
};
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::TyCtxt;

/// Create a call graph starting from the provided root node.
pub fn create_call_graph_from_root(context: TyCtxt, item: &Item) -> Graph {
    let mut graph = Graph::new(context.crate_name(LOCAL_CRATE).to_ident_string());

    // Access the function
    if let ItemKind::Fn(_sig, _gen, id) = item.kind {
        // Create a node for the function
        let node = NodeKind::local_fn(item.hir_id().owner.to_def_id(), item.hir_id());
        let node_id = graph.add_node(&context.def_path_str(node.def_id()), node);

        // Add edges/nodes for all functions called from within this function (and recursively do it for those functions as well)
        graph = add_calls_from_function(context, node_id, id.hir_id, graph);
    }

    graph
}

/// Retrieve all function calls within a function, and add the nodes and edges to the graph.
fn add_calls_from_function(
    context: TyCtxt,
    from_node: usize,
    fn_id: HirId,
    mut graph: Graph,
) -> Graph {
    let node = context.hir_node(fn_id);

    // Access the code block of the function
    match node {
        rustc_hir::Node::Expr(expr) => {
            if let ExprKind::Block(block, _) = expr.kind {
                graph = add_calls_from_block(context, from_node, block, graph);
            } else if let ExprKind::Closure(closure) = expr.kind {
                graph = add_calls_from_function(context, from_node, closure.body.hir_id, graph);
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
        rustc_hir::Node::ImplItem(item) => {
            if let ImplItemKind::Fn(_sig, id) = item.kind {
                graph = add_calls_from_function(context, from_node, id.hir_id, graph);
            }
        }
        _ => {}
    }

    graph
}

/// Retrieve all function calls within a block, and add the nodes and edges to the graph.
fn add_calls_from_block(context: TyCtxt, from: usize, block: &Block, mut graph: Graph) -> Graph {
    // Get the function calls from within this block
    let calls = get_function_calls_in_block(context, block, true);

    // Add edges for all function calls
    for (node_kind, call_id, add_edge, propagates) in calls {
        match node_kind {
            NodeKind::LocalFn(def_id, hir_id) => {
                if let Some(node) = graph.find_local_fn_node(hir_id) {
                    // We have already encountered this local function, so just add the edge
                    if add_edge {
                        graph.add_edge(Edge::new(from, node.id(), call_id, propagates));
                    }
                } else {
                    // We have not yet explored this local function, so add new node and edge,
                    // and explore it.
                    let id = graph.add_node(&context.def_path_str(def_id), node_kind);

                    if add_edge {
                        graph.add_edge(Edge::new(from, id, call_id, propagates));
                    }

                    graph = add_calls_from_function(context, id, hir_id, graph);
                }
            }
            NodeKind::NonLocalFn(def_id) => {
                if let Some(node) = graph.find_non_local_fn_node(def_id) {
                    // We have already encountered this non-local function, so just add the edge
                    if add_edge {
                        graph.add_edge(Edge::new(from, node.id(), call_id, propagates));
                    }
                } else {
                    // We have not yet explored this non-local function, so add new node and edge
                    let id = graph.add_node(&context.def_path_str(node_kind.def_id()), node_kind);

                    if add_edge {
                        graph.add_edge(Edge::new(from, id, call_id, propagates));
                    }
                }
            }
        }
    }

    graph
}

/// Retrieve a vec of all function calls made within the body of a block.
fn get_function_calls_in_block(
    context: TyCtxt,
    block: &Block,
    is_fn: bool,
) -> Vec<(NodeKind, HirId, bool, bool)> {
    let mut res: Vec<(NodeKind, HirId, bool, bool)> = vec![];

    // If the block has an ending expression add calls from there
    // If this block is that of a function, this is a return statement
    if let Some(exp) = block.expr {
        if let ExprKind::DropTemps(ex) = exp.kind {
            if let ExprKind::Block(b, _lbl) = ex.kind {
                return get_function_calls_in_block(context, b, is_fn);
            }
        } else {
            if is_fn {
                for (kind, id, add_edge, _) in get_function_calls_in_expression(context, exp) {
                    res.push((kind.clone(), id, add_edge, true));
                }
            } else {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
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
            StmtKind::Expr(exp) | StmtKind::Semi(exp) => {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
    }

    res
}

/// Retrieve a vec of all function calls made within an expression.
#[allow(clippy::too_many_lines)]
fn get_function_calls_in_expression(
    context: TyCtxt,
    expr: &Expr,
) -> Vec<(NodeKind, HirId, bool, bool)> {
    let mut res: Vec<(NodeKind, HirId, bool, bool)> = vec![];

    // Match the kind of expression
    match expr.kind {
        ExprKind::Call(func, args) => {
            if let Some(def_id) = get_call_def_id(context, expr.hir_id) {
                let node_kind = get_node_kind_from_def_id(context, def_id);
                res.push((node_kind, expr.hir_id, true, false));
            } else if let ExprKind::Path(qpath) = func.kind {
                if let Some((node_kind, _add_edge)) = get_node_kind_from_path(context, qpath) {
                    res.push((node_kind, expr.hir_id, true, false));
                }
            }
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::MethodCall(_path, exp, args, _span) => {
            if let Some(def_id) = get_call_def_id(context, expr.hir_id) {
                let node_kind = get_node_kind_from_def_id(context, def_id);
                res.push((node_kind, expr.hir_id, true, false));
            } else if let Some(def_id) = context
                .typeck(expr.hir_id.owner.def_id)
                .type_dependent_def_id(expr.hir_id)
            {
                if let Some(local_id) = def_id.as_local() {
                    res.push((
                        NodeKind::local_fn(def_id, context.local_def_id_to_hir_id(local_id)),
                        expr.hir_id,
                        true,
                        false,
                    ));
                } else {
                    res.push((NodeKind::non_local_fn(def_id), expr.hir_id, true, false));
                }
            }
            res.extend(get_function_calls_in_expression(context, exp));
            for exp in args {
                res.extend(get_function_calls_in_expression(context, exp));
            }
        }
        ExprKind::Match(exp, arms, src) => {
            match src {
                MatchSource::TryDesugar(_hir) => {
                    for (kind, id, add_edge, _) in get_function_calls_in_expression(context, exp) {
                        res.push((kind, id, add_edge, true));
                    }

                    return res;
                }
                _ => {
                    res.extend(get_function_calls_in_expression(context, exp));
                }
            }
            for arm in arms {
                res.extend(get_function_calls_in_expression(context, arm.body));
                if let Some(guard) = arm.guard {
                    res.extend(get_function_calls_in_expression(context, guard));
                }
                res.extend(get_function_calls_in_pattern(context, arm.pat));
            }
        }
        ExprKind::Closure(closure) => {
            let node_kind = NodeKind::local_fn(
                closure.def_id.to_def_id(),
                context.local_def_id_to_hir_id(closure.def_id),
            );
            res.push((node_kind, expr.hir_id, false, false));
        }
        ExprKind::ConstBlock(block) => {
            let node = context.hir_node(block.hir_id);
            res.extend(get_function_calls_in_block(
                context,
                node.expect_block(),
                false,
            ));
        }
        ExprKind::Array(args) | ExprKind::Tup(args) => {
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
        ExprKind::Cast(exp, _ty) | ExprKind::Type(exp, _ty) => {
            res.extend(get_function_calls_in_expression(context, exp));
        }
        ExprKind::DropTemps(exp) | ExprKind::Become(exp) => {
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
            res.extend(get_function_calls_in_block(context, block, false));
        }
        ExprKind::Block(block, _lbl) => {
            res.extend(get_function_calls_in_block(context, block, false));
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
        ExprKind::Path(path) => {
            if let Some((node_kind, add_edge)) = get_node_kind_from_path(context, path) {
                res.push((node_kind, expr.hir_id, add_edge, false));
            }
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
                for (kind, id, add_edge, _) in get_function_calls_in_expression(context, exp) {
                    res.push((kind, id, add_edge, true));
                }
            }
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
fn get_function_calls_in_pattern(context: TyCtxt, pat: &Pat) -> Vec<(NodeKind, HirId, bool, bool)> {
    let mut res: Vec<(NodeKind, HirId, bool, bool)> = vec![];

    match pat.kind {
        PatKind::Wild | PatKind::Never => {
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
        PatKind::Path(_path) => {
            // No function calls here
        }
        PatKind::Tuple(pats, _pos) => {
            for p in pats {
                res.extend(get_function_calls_in_pattern(context, p));
            }
        }
        PatKind::Box(p) | PatKind::Deref(p) => {
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

/// Get the node kind from a given `QPath`.
fn get_node_kind_from_path(context: TyCtxt, qpath: QPath) -> Option<(NodeKind, bool)> {
    match qpath {
        QPath::Resolved(_ty, path) => {
            if let Res::Def(kind, id) = path.res {
                let add_edge: bool = matches!(
                    kind,
                    DefKind::Fn | DefKind::Ctor(_, _) | DefKind::AssocFn | DefKind::Closure
                );
                return Some((get_node_kind_from_def_id(context, id), add_edge));
            }
        }
        QPath::TypeRelative(ty, _segment) => {
            if let TyKind::Path(path) = ty.kind {
                return get_node_kind_from_path(context, path);
            }
        }
        QPath::LangItem(_, _) => {}
    }

    None
}

/// Get the `NodeKind` from a given `DefId`.
fn get_node_kind_from_def_id(context: TyCtxt, def_id: DefId) -> NodeKind {
    if let Some(local_id) = def_id.as_local() {
        let hir_id = context.local_def_id_to_hir_id(local_id);
        NodeKind::local_fn(def_id, hir_id)
    } else {
        NodeKind::non_local_fn(def_id)
    }
}

/// Get the `DefId` of the called function using the `HirId` of the call.
pub fn get_call_def_id(context: TyCtxt, call_id: HirId) -> Option<DefId> {
    if !context.is_mir_available(call_id.owner.to_def_id()) {
        return None;
    }

    let mir = context.optimized_mir(call_id.owner.to_def_id());

    for block in mir.basic_blocks.iter() {
        if let Some(terminator) = &block.terminator {
            if let TerminatorKind::Call { func, fn_span, .. } = &terminator.kind {
                if context.hir_node(call_id).expect_expr().span.hi() == fn_span.hi() {
                    if let Some((def_id, _)) = func.const_fn_def() {
                        return Some(def_id);
                    }
                }
            }
        }
    }

    None
}
