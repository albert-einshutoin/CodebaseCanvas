use clap::{Arg, Command, value_parser};
use codebasecanvas_analyzer::{Severity, SystemGraph};
use std::path::{Path, PathBuf};

pub fn command() -> Command {
    Command::new("codebasecanvas")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Analyze a local repository (analysis pipeline is not implemented yet)")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("analyze")
                .about("Write .codebasecanvas/graph.json after successful analysis")
                .arg(
                    Arg::new("repository")
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                ),
        )
}

pub struct Summary {
    pub path: PathBuf,
    pub warnings: usize,
    pub errors: usize,
}

pub fn analyze(
    path: &Path,
    pipeline: impl FnOnce(&Path) -> Result<SystemGraph, String>,
) -> Result<Summary, String> {
    let root = crate::output::Repository::open(path)?;
    let graph = pipeline(&root.path)?;
    let path = root.save(&graph)?;
    Ok(Summary {
        path,
        warnings: graph
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count(),
        errors: graph
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count(),
    })
}
