use std::process::Command;

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
fn production_pipeline_never_writes_a_placeholder() {
    let path = std::env::temp_dir().join(format!("canvas-cli-process-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let output = cli(&["analyze", path.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not implemented"));
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    std::fs::remove_dir(&path).unwrap();
    let missing = cli(&["analyze", path.to_str().unwrap()]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("repository"));
}
