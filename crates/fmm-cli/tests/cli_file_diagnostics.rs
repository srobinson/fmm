use assert_cmd::cargo::CommandCargoExt;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn setup_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/app.ts",
        "export function createApp() {\n  return {};\n}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn run_fmm(root: &std::path::Path, args: &[&str]) -> Output {
    Command::cargo_bin("fmm")
        .unwrap()
        .args(args)
        .current_dir(root)
        .output()
        .expect("failed to run fmm")
}

#[test]
fn outline_missing_file_reports_workspace_not_found() {
    let tmp = setup_project();
    let output = run_fmm(tmp.path(), &["outline", "src/missing.ts"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("File not found in workspace: src/missing.ts"),
        "got: {stderr}"
    );
    assert!(
        !stderr.contains("Run 'fmm generate'"),
        "missing disk files should not suggest regeneration; got: {stderr}"
    );
}

#[test]
fn outline_unindexed_file_reports_missing_from_index() {
    let tmp = setup_project();
    write_file(
        tmp.path(),
        "src/new.ts",
        "export function createNew() {\n  return {};\n}\n",
    );

    let output = run_fmm(tmp.path(), &["outline", "src/new.ts"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("File exists but is missing from the fmm index: src/new.ts"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("Run 'fmm generate'."),
        "unindexed disk files should suggest regeneration; got: {stderr}"
    );
}
