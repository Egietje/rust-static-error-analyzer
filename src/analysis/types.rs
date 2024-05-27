use rustc_hir::HirId;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::{Interner, Ty, TyCtxt};

pub fn get_call_type(context: TyCtxt, call_id: HirId) -> Option<Ty> {
    if !context.is_mir_available(call_id.owner.to_def_id()) {
        return None;
    }

    let mir = context.optimized_mir(call_id.owner.to_def_id());

    for block in mir.basic_blocks.iter() {
        if let Some(terminator) = &block.terminator {
            if let TerminatorKind::Call { func, fn_span, .. } = &terminator.kind {
                if context.hir_node(call_id).expect_expr().span.hi() == fn_span.hi() {
                    if let Some((def_id, args)) = func.const_fn_def() {
                        return Some(
                            context
                                .type_of_instantiated(def_id, args)
                                .fn_sig(context)
                                .output()
                                .skip_binder(),
                        );
                    }
                }
            }
        }
    }

    None
}
