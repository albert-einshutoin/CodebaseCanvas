mod cli;
mod output;

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = cli::command().get_matches();
    let Some(("analyze", arguments)) = arguments.subcommand() else {
        return ExitCode::FAILURE;
    };
    let Some(path) = arguments.get_one::<std::path::PathBuf>("repository") else {
        return ExitCode::FAILURE;
    };
    match cli::analyze(path, |_| {
        Err("repository analysis is not implemented yet (Issue #17); no graph was written".into())
    }) {
        Ok(summary) => {
            println!("Graph written: {:?}", summary.path);
            println!(
                "Diagnostics: {} warnings, {} errors",
                summary.warnings, summary.errors
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("CodebaseCanvas: {error}");
            ExitCode::FAILURE
        }
    }
}
