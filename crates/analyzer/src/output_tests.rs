use super::{Repository, unix};
use codebasecanvas_analyzer::SystemGraph;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "canvas-cli-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn target(&self) -> PathBuf {
        self.0.join(".codebasecanvas/graph.json")
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn graph() -> SystemGraph {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../contracts/cases.json")).unwrap();
    SystemGraph::from_json(&cases["graph"].to_string()).unwrap()
}
fn assert_only_graph(sandbox: &Sandbox) {
    let files: Vec<_> = fs::read_dir(sandbox.0.join(".codebasecanvas"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(files, ["graph.json"]);
}

#[test]
fn save_and_replace_use_validated_canonical_graph() {
    let sandbox = Sandbox::new();
    let repository = Repository::open(&sandbox.0).unwrap();
    let mut graph = graph();
    let path = repository.save(&graph).unwrap();
    assert_eq!(path, sandbox.target());
    assert_eq!(fs::read_to_string(&path).unwrap(), graph.to_json().unwrap());
    graph.metadata.root_name = Some("changed".into());
    repository.save(&graph).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), graph.to_json().unwrap());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_only_graph(&sandbox);
}

#[test]
fn invalid_graph_and_pipeline_failure_preserve_previous_graph() {
    let sandbox = Sandbox::new();
    let repository = Repository::open(&sandbox.0).unwrap();
    let mut graph = graph();
    repository.save(&graph).unwrap();
    let previous = fs::read(sandbox.target()).unwrap();
    graph.nodes[0].id = "invalid".into();
    assert!(repository.save(&graph).is_err());
    assert!(crate::cli::analyze(&sandbox.0, |_| Err("fatal pipeline".into())).is_err());
    assert_eq!(fs::read(sandbox.target()).unwrap(), previous);
    assert_only_graph(&sandbox);
}

#[test]
fn partial_write_failure_preserves_previous_graph_and_cleans_temporary_file() {
    let sandbox = Sandbox::new();
    let repository = Repository::open(&sandbox.0).unwrap();
    repository.save(&graph()).unwrap();
    let previous = fs::read(sandbox.target()).unwrap();
    let failure = unix::atomic_write(&repository.directory, |file| {
        file.write_all(b"partial")?;
        Err(io::Error::other("injected disk failure"))
    });
    assert!(failure.is_err());
    assert_eq!(fs::read(sandbox.target()).unwrap(), previous);
    assert_only_graph(&sandbox);
}

#[test]
fn symlink_directory_and_target_never_touch_external_files() {
    for directory_link in [true, false] {
        for dangling in [true, false] {
            let sandbox = Sandbox::new();
            let outside = Sandbox::new();
            let external = outside.0.join("graph.json");
            if !dangling {
                fs::write(&external, b"external sentinel").unwrap();
            }
            if directory_link {
                symlink(&outside.0, sandbox.0.join(".codebasecanvas")).unwrap();
            } else {
                fs::create_dir(sandbox.0.join(".codebasecanvas")).unwrap();
                symlink(&external, sandbox.target()).unwrap();
            }
            assert!(
                Repository::open(&sandbox.0)
                    .unwrap()
                    .save(&graph())
                    .is_err()
            );
            if dangling {
                assert!(!external.exists());
            } else {
                assert_eq!(fs::read(&external).unwrap(), b"external sentinel");
            }
            assert_eq!(
                fs::read_dir(&outside.0).unwrap().count(),
                usize::from(!dangling)
            );
        }
    }
}

#[test]
fn target_changed_to_symlink_before_commit_is_rejected() {
    let sandbox = Sandbox::new();
    let outside = Sandbox::new();
    let external = outside.0.join("sentinel");
    fs::write(&external, b"outside").unwrap();
    let repository = Repository::open(&sandbox.0).unwrap();
    let failure = unix::atomic_write(&repository.directory, |file| {
        file.write_all(b"new")?;
        symlink(&external, sandbox.target())
    });
    assert!(failure.is_err());
    assert_eq!(fs::read(&external).unwrap(), b"outside");
    assert!(sandbox.target().is_symlink());
    assert_only_graph(&sandbox);
}

#[test]
fn directory_swapped_to_symlink_before_commit_is_rejected() {
    let sandbox = Sandbox::new();
    let outside = Sandbox::new();
    let repository = Repository::open(&sandbox.0).unwrap();
    let moved = sandbox.0.join("old-output");
    let failure = unix::atomic_write(&repository.directory, |file| {
        file.write_all(b"new")?;
        fs::rename(sandbox.0.join(".codebasecanvas"), &moved)?;
        symlink(&outside.0, sandbox.0.join(".codebasecanvas"))
    });
    assert!(failure.is_err());
    assert_eq!(fs::read_dir(&moved).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
}

#[test]
fn root_changed_during_pipeline_does_not_create_output() {
    let sandbox = Sandbox::new();
    let root = sandbox.0.join("repository");
    fs::create_dir(&root).unwrap();
    let moved = sandbox.0.join("moved");
    assert!(
        crate::cli::analyze(&root, |_| {
            fs::rename(&root, &moved).unwrap();
            fs::create_dir(&root).unwrap();
            Ok(graph())
        })
        .is_err()
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&moved).unwrap().count(), 0);
}

#[test]
fn orchestration_accepts_warning_graph_and_passes_canonical_root() {
    let sandbox = Sandbox::new();
    let summary = crate::cli::analyze(&sandbox.0.join("."), |root| {
        assert_eq!(root.path(), sandbox.0);
        Ok(graph())
    })
    .unwrap();
    assert_eq!(summary.path, sandbox.target());
    assert_eq!(summary.warnings, 1);
    assert_eq!(summary.errors, 0);
    assert_eq!(
        summary.warnings,
        graph()
            .diagnostics
            .iter()
            .filter(|d| d.severity == codebasecanvas_analyzer::Severity::Warning)
            .count()
    );
    assert_eq!(
        fs::read_to_string(summary.path).unwrap(),
        graph().to_json().unwrap()
    );
}

#[test]
fn selected_root_identity_survives_canonicalization() {
    let sandbox = Sandbox::new();
    let outside = Sandbox::new();
    let selected = sandbox.0.join("selected");
    symlink(&sandbox.0, &selected).unwrap();
    let before = fs::metadata(&selected).unwrap();
    // A symlink input is valid when it still identifies the captured directory.
    assert_eq!(
        Repository::open_checked(&selected, &before)
            .unwrap()
            .root
            .path()
            .to_owned(),
        sandbox.0
    );
    fs::remove_file(&selected).unwrap();
    symlink(&outside.0, &selected).unwrap();
    assert!(Repository::open_checked(&selected, &before).is_err());
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
    assert!(!sandbox.0.join(".codebasecanvas").exists());
}
