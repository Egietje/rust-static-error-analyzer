#![feature(rustc_private)]

mod analysis;
mod graph;

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;

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
            let graph = analysis::analyze(context);

            println!("{graph:?}");
        });

        // No need to compile further
        rustc_driver::Compilation::Stop
    }
}
