use rustc_hir::def_id::DefId;
use rustc_hir::HirId;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::{GenericArg, Interner, Ty, TyCtxt, TyKind};

/// Get the return type of a called function.
#[allow(clippy::similar_names)]
fn get_call_type(context: TyCtxt, call_id: HirId, caller_id: DefId, called_id: DefId) -> Ty {
    if let Some(ty) = get_call_type_using_mir(context, call_id, caller_id) {
        ty
    } else {
        get_call_type_using_context(context, called_id)
    }
}

/// Extracts the return type of a called function using just the function's `DefId`.
/// Should always succeed.
fn get_call_type_using_context(context: TyCtxt, called_id: DefId) -> Ty {
    if context.type_of(called_id).instantiate_identity().is_fn() {
        context
            .fn_sig(called_id)
            .instantiate_identity()
            .output()
            .skip_binder()
    } else {
        context.type_of(called_id).instantiate_identity()
    }
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

/// Extract the error type from Result, or return the full type if it doesn't contain a Result (along with a flag of whether it is an extract error).
#[allow(clippy::similar_names)]
pub fn get_error_or_type(
    context: TyCtxt,
    call_id: HirId,
    caller_id: DefId,
    called_id: DefId,
) -> (String, bool) {
    let ret_ty = get_call_type(context, call_id, caller_id, called_id);

    let result = if context.ty_is_opaque_future(ret_ty) {
        extract_result_from_future(context, ret_ty)
    } else {
        extract_result(ret_ty)
    };

    let res = extract_error_from_result(result);

    (res.clone().unwrap_or(format!("{ret_ty}")), res.is_some())
}

/// Extract the Result type from any type.
fn extract_result(ty: Ty) -> Option<GenericArg> {
    for arg in ty.walk() {
        let format = format!("{arg}");
        if format.starts_with("std::result::Result<") && format.ends_with('>') {
            return Some(arg);
        }
    }

    None
}

/// Extract the Result type from any future.
fn extract_result_from_future<'a>(context: TyCtxt<'a>, ty: Ty<'a>) -> Option<GenericArg<'a>> {
    for t in ty.walk() {
        if let Some(typ) = t.as_type() {
            if let TyKind::Alias(_kind, alias) = typ.kind() {
                if let TyKind::Coroutine(_def_id, args) =
                    context.type_of(alias.def_id).instantiate_identity().kind()
                {
                    for arg in *args {
                        let format = format!("{arg}");
                        if format.starts_with("std::result::Result<") && format.ends_with('>') {
                            return Some(arg);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Extract the error from a Result type.
fn extract_error_from_result(opt: Option<GenericArg>) -> Option<String> {
    let t = opt?;
    for arg in t.walk() {
        let f = format!("{arg}");
        if format!("{t}").ends_with(&format!(", {f}>")) {
            return Some(f);
        }
    }

    None
}
