#![feature(rustc_private)]

mod analysis;
mod graph;

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;

use rustc_driver::Compilation;
use rustc_interface::interface::Compiler;
use rustc_interface::Queries;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::Table;

/// Entry point, first sets up the compiler, and then runs it using the provided arguments.
fn main() {
    // Create a wrapper around an DiagCtxt that is used for early error emissions.
    let early_dcx =
        rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());

    // Get command-line args
    let args = rustc_driver::args::raw_args(&early_dcx)
        .unwrap_or_else(|_| std::process::exit(rustc_driver::EXIT_FAILURE));

    // Extract the arguments
    let (relative_manifest_path, relative_output_path, remove_redundant) = extract_arguments(&args);

    let manifest_path = get_manifest_path(&relative_manifest_path);
    let output_path = get_output_path(&relative_output_path);

    // Extract the compiler arguments from running `cargo build`
    let compiler_args = get_compiler_args(&relative_manifest_path, &manifest_path)
        .expect("Could not get arguments from cargo build!");

    // Enable CTRL + C
    rustc_driver::install_ctrlc_handler();

    // Install a panic hook that will print the ICE message on unexpected panics.
    let using_internal_features =
        rustc_driver::install_ice_hook(rustc_driver::DEFAULT_BUG_REPORT_URL, |_| ());

    // This allows tools to enable rust logging without having to magically match rustc’s tracing crate version.
    rustc_driver::init_rustc_env_logger(&early_dcx);

    // Run the compiler using the retrieved args.
    let exit_code = run_compiler(
        compiler_args,
        &mut AnalysisCallback(output_path, remove_redundant),
        using_internal_features,
    );

    println!("Ran compiler, exit code: {exit_code}");
}

/// Extract the needed arguments from the provided arguments
fn extract_arguments(args: &[String]) -> (String, String, bool) {
    if args.len() < 3 {
        eprintln!("Usage:");
        eprintln!("static-result-analyzer.exe input output [--call]");
        eprintln!();
        eprintln!("Both the input and output path should be relative.");
        eprintln!("The call flag will output the call graph instead of the error chain graph if set.");
        std::process::exit(rustc_driver::EXIT_FAILURE);
    }

    (
        args.get(1).unwrap().clone(),
        args.get(2).unwrap().clone(),
        !args.get(3).is_some_and(|arg| arg == "--call"),
    )
}

/// Get the full path to the manifest.
fn get_output_path(output_path: &str) -> PathBuf {
    std::env::current_dir().unwrap().join(output_path)
}

/// Get the full path to the manifest.
fn get_manifest_path(cargo_path: &str) -> PathBuf {
    std::env::current_dir().unwrap().join(cargo_path)
}

/// Get the compiler arguments used to compile the package by first running `cargo clean` and then `cargo build -vv`.
fn get_compiler_args(relative_manifest_path: &str, manifest_path: &PathBuf) -> Option<Vec<String>> {
    println!("Using {}!", cargo_version().trim_end_matches('\n'));

    let (package_name, bin_name) = get_package_name(manifest_path);

    cargo_clean(manifest_path, &package_name);

    let build_output = cargo_build_verbose(manifest_path);

    let command = get_rustc_invocation(&build_output, &package_name, bin_name)?;

    Some(split_args(relative_manifest_path, &command))
}

/// Split up individual arguments from the command.
fn split_args(relative_manifest_path: &str, command: &str) -> Vec<String> {
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
        if arg.starts_with('"') && arg.ends_with('"') {
            temp.push_str(
                arg.strip_prefix('"')
                    .expect("Could not remove '\"' from start of string!")
                    .strip_suffix('"')
                    .expect("Could not remove '\"' from end of string!"),
            );
            res.push(temp);
            temp = String::new();
        } else if arg.ends_with('"') {
            temp.push_str(
                arg.strip_suffix('"')
                    .expect("Could not remove '\"' from end of string!"),
            );
            res.push(temp);
            temp = String::new();
        } else if arg.starts_with('"') {
            temp.push_str(
                arg.strip_prefix('"')
                    .expect("Could not remove '\"' from start of string!"),
            );
            temp.push(' ');
        } else if !temp.is_empty() {
            temp.push_str(&arg);
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
        res[i] = res[i].replace("\\\"", "\"");
        if res[i].starts_with("--error-format=") {
            res[i] = String::from("--error-format=short");
        }
        if res[i].starts_with("--json=") {
            res.remove(i);
        }
    }

    res
}

/// Run `cargo clean -p PACKAGE`, where the package name is extracted from the given manifest.
fn cargo_clean(manifest_path: &PathBuf, package_name: &str) -> String {
    println!("Cleaning package...");
    let mut clean_command = create_cargo_command();
    clean_command.arg("clean");
    clean_command.arg("-p");
    clean_command.arg(package_name);

    clean_command.current_dir(
        manifest_path
            .parent()
            .expect("Could not get manifest directory!"),
    );

    let output = clean_command.output().expect("Could not clean!");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8!");

    if output.status.code() != Some(0) {
        eprintln!("Could not clean package!");
        println!("{:?}", stderr);
    }

    stderr
}

/// Extract the package name from the given manifest.
fn get_package_name(manifest_path: &PathBuf) -> (String, Option<String>) {
    let file = std::fs::read(manifest_path).expect("Could not read manifest!");
    let content = String::from_utf8(file).expect("Invalid UTF8!");
    let table = content
        .parse::<Table>()
        .expect("Could not parse manifest as TOML!");
    let package_table = table["package"]
        .as_table()
        .expect("'package' is not a table!");
    let package_name = package_table["name"]
        .as_str()
        .expect("No name found in package information!")
        .to_owned();
    if table.contains_key("bin") {
        let binary_table = table["bin"]
            .as_array()
            .expect("'bin' is not an array!")
            .get(0)
            .expect("'bin' contains no values!")
            .as_table()
            .expect("'bin' is not a table!");
        let binary_name = binary_table["name"]
            .as_str()
            .expect("No name found in binary information!")
            .to_owned();
        return (package_name, Some(binary_name));
    }

    (package_name, None)
}

/// Create a new cargo command.
fn create_cargo_command() -> Command {
    let command = Command::new("cargo");

    command
}

/// Run `cargo --version`.
fn cargo_version() -> String {
    let mut version_command = create_cargo_command();
    version_command.arg("--version");

    let output = version_command
        .output()
        .expect("Could not get cargo version!");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8!");

    stdout
}

/// Run `cargo build -v` on the given manifest.
fn cargo_build_verbose(manifest_path: &Path) -> String {
    // TODO: interrupt build as to not compile the program twice
    println!("Building package...");
    let mut build_command = create_cargo_command();
    build_command.arg("build");
    build_command.arg("-v");
    build_command.arg("--manifest-path");
    build_command.arg(manifest_path.as_os_str());

    let output = build_command.output().expect("Could not build!");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8!");

    if output.status.code() != Some(0) {
        eprintln!("Could not (fully) build package!");
        eprintln!();
        for line in stderr.split('\n') {
            if line.starts_with("error") {
                eprintln!("{}", line);
            }
        }
        eprintln!();
        eprintln!("Trying to continue...");
    }

    stderr
}

/// Gets the rustc invocation command from the output of `cargo build -vv`.
fn get_rustc_invocation(
    build_output: &str,
    package_name: &str,
    bin_name: Option<String>,
) -> Option<String> {
    let name = bin_name.unwrap_or(package_name.to_owned()).replace('-', "_");
    for line in build_output.split('\n') {
        for part in line.split('`') {
            for command in part.split("&& ") {
                if command.contains("rustc")
                    && command.contains("--crate-type bin")
                    && !command.contains("build.rs")
                    && command.contains("main.rs")
                    && command.contains(&format!("--crate-name {name}"))
                {
                    return Some(String::from(command));
                }
            }
        }
    }

    None
}

/// Run a compiler with the provided arguments and callbacks.
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

struct AnalysisCallback(PathBuf, bool);

impl rustc_driver::Callbacks for AnalysisCallback {
    fn after_crate_root_parsing<'tcx>(
        &mut self,
        _compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        // Access type context
        queries.global_ctxt().unwrap().enter(|context| {
            println!("Analyzing output...");
            // Analyze the program using the type context
            let (call_graph, chain_graph) = analysis::analyze(context);

            let dot = if self.1 {
                chain_graph.to_dot()
            } else {
                call_graph.to_dot()
            };

            println!("Writing graph...");

            match std::fs::write(&self.0, dot.clone()) {
                Ok(()) => {
                    println!("Done!");
                    println!("Wrote to {}", &self.0.display());
                }
                Err(e) => {
                    eprintln!("Could not write output!");
                    eprintln!("{e}");
                    eprintln!();
                    println!("{dot}");
                }
            }
        });

        // No need to compile further
        Compilation::Stop
    }
}
