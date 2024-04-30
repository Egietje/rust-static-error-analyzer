#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::CrateNum;
use rustc_hir::{FnRetTy, Item, Node, PathSegment, QPath, Ty, TyKind};
use rustc_middle::ty::TyCtxt;

/// Entry point, first sets up the compiler, and then runs it using the provided arguments.
fn main() {
    // Create a wrapper around an DiagCtxt that is used for early error emissions.
    let early_dcx =
        rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());

    // Gets (raw) command-line args
    let args = rustc_driver::args::raw_args(&early_dcx)
        .unwrap_or_else(|_| std::process::exit(rustc_driver::EXIT_FAILURE));

    // Enables CTRL + C
    rustc_driver::install_ctrlc_handler();

    // Installs a panic hook that will print the ICE message on unexpected panics.
    let using_internal_features =
        rustc_driver::install_ice_hook(rustc_driver::DEFAULT_BUG_REPORT_URL, |_| ());

    // This allows tools to enable rust logging without having to magically match rustc’s tracing crate version.
    rustc_driver::init_rustc_env_logger(&early_dcx);

    // Run the compiler using the command-line args.
    let exit_code = run_compiler(args, &mut AnalysisCallback, using_internal_features);

    println!("Ran compiler, exit code: {exit_code}");
}

struct AnalysisCallback;

impl rustc_driver::Callbacks for AnalysisCallback {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        // Access type context
        queries.global_ctxt().unwrap().enter(|context| {
            // Analyze the type context
            analyze(context);
        });

        // No need to compile further
        rustc_driver::Compilation::Stop
    }
}

/// Analyze the type context, starting from the root node.
fn analyze(context: TyCtxt) {
    let crate_node = context.hir_node(rustc_hir::hir_id::CRATE_HIR_ID);
    let mod_node = crate_node.expect_crate();

    for item_id in mod_node.item_ids {
        let node = context.hir_node(item_id.hir_id());

        if let Node::Item(item) = node {
            let type_opt = get_function_return_type(item);
            if let Some(ret_type) = type_opt {
                println!("{:?}", item);
                println!("{} {}", item.ident, is_result_type(ret_type, context));
                println!();
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

/// Run a compiler with the provided (command-line) arguments and callbacks.
/// Returns the exit code of the compiler.
fn run_compiler(
    args: Vec<String>,
    callbacks: &mut (dyn rustc_driver::Callbacks + Send),
    using_internal_features: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> i32 {
    println!("Running compiler.");

    // Invoke compiler, and return the exit code
    rustc_driver::catch_with_exit_code(move || {
        rustc_driver::RunCompiler::new(&args, callbacks)
            .set_using_internal_features(using_internal_features)
            .run()
    })
}
