use clap::{Arg, Command, value_parser};
use codebasecanvas_analyzer::{Severity, discovery::RepositoryRoot, pipeline::Analysis};
use std::path::PathBuf;

pub fn command() -> Command {
    Command::new("codebasecanvas")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Analyze a local repository into a validated SystemGraph")
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
    pub typescript_sources: usize,
    pub prisma_schemas: usize,
    pub nodes: usize,
    pub edges: usize,
    pub info: usize,
    pub warnings: usize,
    pub errors: usize,
}

pub fn analyze(
    path: &std::path::Path,
    pipeline: impl FnOnce(&RepositoryRoot) -> Result<Analysis, String>,
) -> Result<Summary, String> {
    let root = crate::output::Repository::open(path)?;
    let analysis = pipeline(&root.root)?;
    let graph = &analysis.graph;
    let path = root.save(graph)?;
    Ok(Summary {
        path,
        typescript_sources: analysis.typescript_sources,
        prisma_schemas: analysis.prisma_schemas,
        nodes: graph.nodes.len(),
        edges: graph.edges.len(),
        info: graph
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
            .count(),
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
