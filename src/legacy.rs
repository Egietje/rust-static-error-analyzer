use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::CrateNum;
use rustc_hir::{FnRetTy, Item, Node, PathSegment, QPath, Ty, TyKind};
use rustc_middle::ty::TyCtxt;

/// Analyze the type context, starting from the root node.
fn analyze(context: TyCtxt) {
    let crate_node = context.hir_node(rustc_hir::hir_id::CRATE_HIR_ID);
    let mod_node = crate_node.expect_crate();

    for item_id in mod_node.item_ids {
        let node = context.hir_node(item_id.hir_id());

        if let Node::Item(item) = node {
            let type_opt = get_function_return_type(item);
            if let Some(_ret_type) = type_opt {
                println!("MIR\n{:?}", context.build_mir(item.owner_id.def_id));
                //println!("\n{:?}", item);
                //println!("{} {}", item.ident, is_result_type(ret_type, context));
                //println!();
            }
        }
    }
}

/// Checks whether the given type is (an alias of) `core::result::Result`.
/// It does this by getting the last entry of the PathSegments, which should be the actual definition.
fn is_result_type(ty: &Ty, context: TyCtxt) -> bool {
    return if let TyKind::Path(path) = ty.kind {
        if let QPath::Resolved(_ty, path) = path {
            if !path.segments.is_empty() {
                let segment = path.segments.last().unwrap();

                is_result(segment, context)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };
}

/// Check whether the path segment is that of `core::result::Result`, or an alias of it.
fn is_result(segment: &PathSegment, context: TyCtxt) -> bool {
    println!("{segment:?}");
    return if let Res::Def(kind, id) = segment.res {
        match kind {
            DefKind::Enum => {
                // Check whether it is core::result::Result

                // Check whether the crate is core or std
                if !is_core_crate(id.krate, context) {
                    return false;
                }

                // Check whether the enum's identifier is Result
                if segment.ident.name.as_str() != "Result" {
                    return false;
                }

                true
            }
            DefKind::TyAlias => {
                // TODO: Check whether it is an alias of core::result::Result

                false
            }
            _ => false,
        }
    } else {
        false
    };
}

/// Return whether the given crate is the core crate.
fn is_core_crate(krate: CrateNum, context: TyCtxt) -> bool {
    context.crate_name(krate).as_str() == "core"
}

/// Get the return type of the function item.
fn get_function_return_type<'a>(item: &'a Item<'a>) -> Option<&'a Ty<'a>> {
    return if let rustc_hir::ItemKind::Fn(sig, _generics, _body) = item.kind {
        match sig.decl.output {
            FnRetTy::DefaultReturn(_span) => None,
            FnRetTy::Return(ty) => Some(ty),
        }
    } else {
        None
    };
}
