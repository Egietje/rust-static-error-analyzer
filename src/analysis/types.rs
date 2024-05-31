use rustc_hir::def_id::DefId;
use rustc_hir::HirId;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::{Interner, Ty, TyCtxt};

/// Get the return type of a called function.
pub fn get_call_type(context: TyCtxt, call_id: HirId, caller_id: DefId, called_id: DefId) -> Ty {
    if let Some(ty) = get_call_type_using_mir(context, call_id, caller_id) {
        ty
    } else {
        get_call_type_using_context(context, called_id)
    }
}

/// Extracts the return type of a called function using just the functions `DefId`.
/// Should always succeed.
fn get_call_type_using_context(context: TyCtxt, called_id: DefId) -> Ty {
    context.type_of(called_id).skip_binder()
}

/// Extracts the return type of a called function using its call's `HirId`, as well as the caller's `DefId`.
/// Returns `None` if no MIR is available or the call was not found (e.g. due to desugaring/optimizations).
fn get_call_type_using_mir(context: TyCtxt, call_id: HirId, caller_id: DefId) -> Option<Ty> {
    if !context.is_mir_available(caller_id) {
        return None;
    }

    let mir = context.optimized_mir(caller_id);
    let call_expr = context.hir_node(call_id).expect_expr();

    for block in mir.basic_blocks.iter() {
        if let Some(terminator) = &block.terminator {
            if let TerminatorKind::Call { func, fn_span, .. } = &terminator.kind {
                if call_expr.span.hi() == fn_span.hi() {
                    if let Some((def_id, args)) = func.const_fn_def() {
                        return Some(context
                            .type_of_instantiated(def_id, args)
                            .fn_sig(context)
                            .output()
                            .skip_binder()
                        );
                    }
                }
            }
        }
    }

    None
}
