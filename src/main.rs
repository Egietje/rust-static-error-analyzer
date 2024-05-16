#![feature(rustc_private)]

mod analysis;
mod graph;

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;

use std::path::PathBuf;
use cargo_options::CommonOptions;

/// Entry point, first sets up the compiler, and then runs it using the provided arguments.
fn main() {
    // Create a wrapper around an DiagCtxt that is used for early error emissions.
    let early_dcx =
        rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());

    // Gets (raw) command-line args
    let args = rustc_driver::args::raw_args(&early_dcx)
        .unwrap_or_else(|_| std::process::exit(rustc_driver::EXIT_FAILURE));

    let manifest_path = get_manifest_path(args);

    let compiler_args = get_compiler_args(manifest_path).expect("Could not get arguments from cargo build!");

    // Enables CTRL + C
    rustc_driver::install_ctrlc_handler();

    // Installs a panic hook that will print the ICE message on unexpected panics.
    let using_internal_features =
        rustc_driver::install_ice_hook(rustc_driver::DEFAULT_BUG_REPORT_URL, |_| ());

    // This allows tools to enable rust logging without having to magically match rustc’s tracing crate version.
    rustc_driver::init_rustc_env_logger(&early_dcx);

    println!("{:?}", compiler_args);

    // Run the compiler using the retrieved args.
    let exit_code = run_compiler(compiler_args, &mut AnalysisCallback, using_internal_features);

    println!("Ran compiler, exit code: {exit_code}");
}

fn get_manifest_path(args: Vec<String>) -> PathBuf {
    let mut manifest_path = std::env::current_dir().unwrap();
    if args.len() < 2 {
        manifest_path = manifest_path.join("Cargo.toml");
    } else {
        let arg = args.get(1).unwrap();
        if arg.ends_with("Cargo.toml") {
            manifest_path = manifest_path.join(arg);
        } else {
            manifest_path = manifest_path.join("Cargo.toml");
        }
    }
    return manifest_path;
}

fn get_compiler_args(manifest_path: PathBuf) -> Option<Vec<String>> {
    let command = get_cargo_build_rustc_invocation(manifest_path)?.trim_end_matches('`').to_string();

    let mut res = vec![];
    // Split on ' ', but leave ' ' when enclosed in '"'
    let mut temp = String::new();
    let mut first = true;
    for arg in command.split(' ') {
        if first {
            first = false;
            continue;
        }
        if arg.starts_with('"') {
            temp = String::new();
            temp.push_str(arg);
            temp.push(' ');
        } else if arg.ends_with('"') {
            temp.push_str(arg);
            res.push(temp.clone());
        } else {
            res.push(String::from(arg));
        }
    }
    Some(res)
}

fn get_cargo_build_rustc_invocation(manifest_path: PathBuf) -> Option<String> {
    let mut options = CommonOptions::default();
    options.verbose = 2;

    let build = cargo_options::Build {
        common: options,
        manifest_path: Some(manifest_path),
        release: false,
        ignore_rust_version: false,
        unit_graph: false,
        packages: vec![],
        workspace: false,
        exclude: vec![],
        all: false,
        lib: false,
        bin: vec![],
        bins: false,
        example: vec![],
        examples: false,
        test: vec![],
        tests: false,
        bench: vec![],
        benches: false,
        all_targets: true,
        out_dir: None,
        build_plan: false,
        future_incompat_report: false,
    };
    let mut command = cargo_options::Build::command(&build);
    let output = command.output().expect("Could not build!");
    let stderr_output = String::from_utf8(output.stderr).expect("Invalid UTF8!");
    for line in stderr_output.split("\n") {
        for command in line.split("&& ") {
            if command.contains("rustc") {
                return Some(String::from(command));
            }
        }
    }
    return None;
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
            let graph = analysis::analyze(context).expect("No graph was made!");

            println!("{}", graph.to_dot());
        });

        // No need to compile further
        rustc_driver::Compilation::Stop
    }
}
