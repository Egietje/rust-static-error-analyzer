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

/// Entry point, first sets up the compiler, and then runs it using the provided arguments.
fn main() {
    // Create a wrapper around an DiagCtxt that is used for early error emissions.
    let early_dcx =
        rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());

    // Gets (raw) command-line args
    let args = rustc_driver::args::raw_args(&early_dcx)
        .unwrap_or_else(|_| std::process::exit(rustc_driver::EXIT_FAILURE));

    let cargo_path = get_relative_manifest_path(args);

    let manifest_path = get_manifest_path(&cargo_path);

    let compiler_args = get_compiler_args(&cargo_path, &manifest_path).expect("Could not get arguments from cargo build!");

    // Enables CTRL + C
    rustc_driver::install_ctrlc_handler();

    // Installs a panic hook that will print the ICE message on unexpected panics.
    let using_internal_features =
        rustc_driver::install_ice_hook(rustc_driver::DEFAULT_BUG_REPORT_URL, |_| ());

    // This allows tools to enable rust logging without having to magically match rustc’s tracing crate version.
    rustc_driver::init_rustc_env_logger(&early_dcx);

    // Run the compiler using the retrieved args.
    let exit_code = run_compiler(compiler_args, &mut AnalysisCallback, using_internal_features);

    println!("Ran compiler, exit code: {exit_code}");
}

fn get_manifest_path(cargo_path: &str) -> PathBuf {
    return std::env::current_dir().unwrap().join(cargo_path);
}

fn get_relative_manifest_path<'a>(args: Vec<String>) -> String {
    if args.len() < 2 {
        String::from("Cargo.toml")
    } else {
        let arg = args.get(1).unwrap();
        if arg.ends_with("Cargo.toml") {
            arg.clone()
        } else {
            String::from("Cargo.toml")
        }
    }
}

fn get_compiler_args(relative_manifest_path: &str, manifest_path: &PathBuf) -> Option<Vec<String>> {
    cargo_clean(manifest_path);

    let build_output = cargo_build_verbose(&manifest_path);

    let command = get_rustc_invocation(&build_output)?;

    Some(split_args(relative_manifest_path, command))
}

fn split_args(relative_manifest_path: &str, command: String) -> Vec<String> {
    let mut res = vec![];
    let mut temp = String::new();

    // Split on ' '
    for arg in command.split(' ') {
        let mut arg = arg.to_owned();

        // If this is the path to main.rs, prepend the relative path to the manifest, stripping away Cargo.toml
        if arg.contains("main.rs") {
            let mut new_arg = String::from(relative_manifest_path.trim_end_matches("Cargo.toml"));
            new_arg.push_str(&arg);
            arg = new_arg;
        }

        // Leave ' ' when enclosed in '"', removing the enclosing '"'
        if arg.ends_with('"') {
            temp.push_str(arg.trim_end_matches('"'));
            res.push(temp);
            temp = String::new();
        } else if arg.starts_with('"') || !temp.is_empty() {
            temp.push_str(arg.trim_start_matches('"'));
            temp.push(' ');
        } else {
            res.push(arg);
        }
    }

    // Overwrite error format args
    for i in 0..res.len() {
        if i >= res.len() {
            break;
        }
        if res[i].starts_with("--error-format=") {
            res[i] = String::from("--error-format=short");
        }
        if res[i].starts_with("--json=") {
            res.remove(i);
        }
    }

    res
}

fn cargo_clean(manifest_path: &PathBuf) -> String {
    // TODO: auto clean proper package
    println!("Cleaning package...");
    let mut clean_command = std::process::Command::new("cargo");
    clean_command.arg("clean");
    //clean_command.arg("-p");
    //clean_command.arg("crate1");

    clean_command.current_dir(manifest_path.parent().expect("Could not get manifest directory!"));

    let output = clean_command.output().expect("Could not clean!");

    String::from_utf8(output.stderr).expect("Invalid UTF8!")
}

fn cargo_build_verbose(manifest_path: &PathBuf) -> String {
    // TODO: interrupt build as to not compile the program twice
    println!("Building package...");
    let mut build_command = std::process::Command::new("cargo");
    build_command.arg("build");
    build_command.arg("-vv");
    build_command.arg("--manifest-path");
    build_command.arg(manifest_path.as_os_str());
    let output = build_command.output().expect("Could not build!");

    String::from_utf8(output.stderr).expect("Invalid UTF8!")
}

fn get_rustc_invocation(build_output: &str) -> Option<String> {
    for line in build_output.split("\n") {
        for command in line.split("&& ") {
            if command.contains("rustc") && command.contains("--crate-type bin") && command.contains("main.rs") {
                return Some(String::from(command.trim_end_matches('`')));
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
    println!("Running compiler...");

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
            println!("Analyzing output...");
            // Analyze the type context
            let graph = analysis::analyze(context).expect("No graph was made!");

            println!("Done!");
            println!();
            println!("{}", graph.to_dot());
        });

        // No need to compile further
        rustc_driver::Compilation::Stop
    }
}
