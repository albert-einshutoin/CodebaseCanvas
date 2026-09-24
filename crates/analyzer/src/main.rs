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
    match cli::analyze(path, |root| {
        let analyzed_at = codebasecanvas_analyzer::pipeline::utc_now()?;
        codebasecanvas_analyzer::pipeline::analyze(root, &analyzed_at)
    }) {
        Ok(summary) => {
            println!(
                "Read {} TypeScript/TSX source files (including parse failures)",
                summary.typescript_sources
            );
            println!(
                "Analyzed {} selected Prisma schemas",
                summary.prisma_schemas
            );
            println!(
                "Generated {} nodes / {} edges",
                summary.nodes, summary.edges
            );
            println!(
                "Diagnostics: {} info / {} warnings / {} errors",
                summary.info, summary.warnings, summary.errors
            );
            if summary.errors > 0 {
                println!("Partial analysis: error diagnostics are present; see graph.json");
            }
            println!("Output: {}", summary.path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("CodebaseCanvas: {error}");
            ExitCode::FAILURE
        }
    }
}
