use codebasecanvas_analyzer::{NodeKind, Severity, SystemGraph};
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Repo(PathBuf);
impl Repo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "canvas-issue17-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn copy_fixture(&self) {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample");
        fn copy_sources(source: &Path, target: &Path) {
            fs::create_dir(target).unwrap();
            for entry in fs::read_dir(source).unwrap() {
                let entry = entry.unwrap();
                let source = entry.path();
                let target = target.join(entry.file_name());
                if source.is_dir() {
                    copy_sources(&source, &target);
                } else if matches!(
                    source.extension().and_then(|s| s.to_str()),
                    Some("ts" | "tsx" | "prisma")
                ) {
                    fs::copy(source, target).unwrap();
                }
            }
        }
        copy_sources(&fixture.join("src"), &self.0.join("src"));
        copy_sources(&fixture.join("prisma"), &self.0.join("prisma"));
        fs::copy(fixture.join("tsconfig.json"), self.0.join("tsconfig.json")).unwrap();
    }
    fn run(&self) -> std::process::Output {
        cli(&["analyze", self.0.to_str().unwrap()])
    }
    fn output(&self) -> PathBuf {
        self.0.join(".codebasecanvas/graph.json")
    }
    fn graph(&self) -> SystemGraph {
        SystemGraph::from_json(&fs::read_to_string(self.output()).unwrap()).unwrap()
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_codebasecanvas"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_and_argument_errors_are_distinct() {
    let help = cli(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("analyze"));
    for args in [&[][..], &["analyze"][..], &["unknown"][..]] {
        let failure = cli(args);
        assert!(!failure.status.success());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("Usage:"));
    }
}

#[test]
fn invalid_repository_is_a_fatal_error() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = cli(&["analyze", path.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("repository"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("not implemented"));
}

#[test]
fn real_cli_analyzes_fixture_and_replaces_graph_from_changed_sources() {
    let repo = Repo::new();
    repo.copy_fixture();
    let first = repo.run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let summary = String::from_utf8(first.stdout).unwrap();
    assert!(summary.contains("Read 13 TypeScript/TSX source files (including parse failures)"));
    assert!(summary.contains("Analyzed 1 selected Prisma schemas"));
    assert!(summary.contains("Generated 57 nodes / 90 edges"));
    assert!(summary.contains("Diagnostics: 0 info / 15 warnings / 0 errors"));
    assert!(summary.contains("Output: "));
    let first_graph = repo.graph();
    assert_eq!(first_graph.nodes.len(), 57);
    assert_eq!(
        first_graph.metadata.analyzer_version,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(first_graph.metadata.analyzed_at.len(), 20);
    assert!(first_graph.metadata.analyzed_at.ends_with('Z'));
    assert!(
        !fs::read_to_string(repo.output())
            .unwrap()
            .contains(repo.0.to_str().unwrap())
    );

    let second = repo.run();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let mut second_graph = repo.graph();
    second_graph.metadata.analyzed_at = first_graph.metadata.analyzed_at.clone();
    assert_eq!(
        first_graph.to_json().unwrap(),
        second_graph.to_json().unwrap()
    );

    fs::write(
        repo.0.join("src/new-feature.ts"),
        "export class NewFeature { run() {} }\n",
    )
    .unwrap();
    let third = repo.run();
    assert!(
        third.status.success(),
        "{}",
        String::from_utf8_lossy(&third.stderr)
    );
    assert!(repo.graph().nodes.iter().any(|n| n.name == "NewFeature"));
    assert_ne!(repo.graph().nodes.len(), first_graph.nodes.len());

    let schema = repo.0.join("prisma/schema.prisma");
    let mut source = fs::read_to_string(&schema).unwrap();
    source.push_str("\nmodel Extra { id Int }\n");
    fs::write(schema, source).unwrap();
    let fourth = repo.run();
    assert!(
        fourth.status.success(),
        "{}",
        String::from_utf8_lossy(&fourth.stderr)
    );
    assert!(
        repo.graph()
            .nodes
            .iter()
            .any(|n| n.name == "Extra" && n.kind == NodeKind::DatabaseModel)
    );
    assert_eq!(
        fs::read_dir(repo.0.join(".codebasecanvas"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn parse_error_is_partial_but_source_read_error_preserves_previous_graph() {
    let repo = Repo::new();
    fs::write(repo.0.join("good.ts"), "export class Good {}\n").unwrap();
    assert!(repo.run().status.success());
    fs::write(repo.0.join("bad.ts"), "class Broken {\n").unwrap();
    let partial = repo.run();
    assert!(partial.status.success());
    let summary = String::from_utf8(partial.stdout).unwrap();
    assert!(summary.contains("Partial analysis: error diagnostics are present"));
    let graph = repo.graph();
    assert!(graph.nodes.iter().any(|n| n.name == "Good"));
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|d| d.code == "TS_PARSE_ERROR" && d.severity == Severity::Error)
    );
    fs::remove_file(repo.0.join("good.ts")).unwrap();
    let all_bad = repo.run();
    assert!(all_bad.status.success());
    assert!(
        String::from_utf8(all_bad.stdout)
            .unwrap()
            .contains("Partial analysis")
    );
    assert!(repo.graph().nodes.is_empty());
    let previous = fs::read(repo.output()).unwrap();
    fs::write(repo.0.join("bad.ts"), [0xff]).unwrap();
    let fatal = repo.run();
    assert!(!fatal.status.success());
    assert!(fatal.stdout.is_empty());
    assert_eq!(fs::read(repo.output()).unwrap(), previous);
    assert_eq!(
        fs::read_dir(repo.0.join(".codebasecanvas"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn empty_repository_is_successfully_analyzed_without_placeholder() {
    let repo = Repo::new();
    let result = repo.run();
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("Generated 0 nodes / 0 edges")
    );
    let graph = repo.graph();
    assert!(graph.nodes.is_empty() && graph.edges.is_empty() && graph.diagnostics.is_empty());
    let missing = cli(&["analyze", repo.0.join("missing").to_str().unwrap()]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("repository"));
}

#[cfg(unix)]
#[test]
fn real_cli_rejects_unsafe_output_directory_and_target() {
    use std::os::unix::fs::symlink;
    let outside = Repo::new();
    fs::write(outside.0.join("graph.json"), b"external sentinel").unwrap();
    let directory_link = Repo::new();
    symlink(&outside.0, directory_link.0.join(".codebasecanvas")).unwrap();
    let failed = directory_link.run();
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(
        fs::read(outside.0.join("graph.json")).unwrap(),
        b"external sentinel"
    );

    let target_link = Repo::new();
    fs::create_dir(target_link.0.join(".codebasecanvas")).unwrap();
    symlink(outside.0.join("graph.json"), target_link.output()).unwrap();
    let failed = target_link.run();
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(
        fs::read(outside.0.join("graph.json")).unwrap(),
        b"external sentinel"
    );
    assert_eq!(
        fs::read_dir(target_link.0.join(".codebasecanvas"))
            .unwrap()
            .count(),
        1
    );
}
