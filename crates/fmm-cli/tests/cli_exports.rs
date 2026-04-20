use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn setup_export_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/app.ts",
        "export function createApp() {\n  return {};\n}\n\nexport const APP_VERSION = '1.0';\n",
    );
    write_file(
        root,
        "src/other.ts",
        "export function createOther() {\n  return {};\n}\n",
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
fn exports_file_text_lists_exports_for_one_file() {
    let tmp = setup_export_project();
    let output = run_fmm(tmp.path(), &["exports", "--file", "src/app.ts"]);

    assert!(
        output.status.success(),
        "fmm exports failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.starts_with("---"), "got: {stdout}");
    assert!(stdout.contains("file: src/app.ts"), "got: {stdout}");
    assert!(stdout.contains("createApp: [1, 3]"), "got: {stdout}");
    assert!(stdout.contains("APP_VERSION: [5, 5]"), "got: {stdout}");
    assert!(!stdout.contains("createOther"), "got: {stdout}");
}

#[test]
fn exports_file_json_lists_exports_for_one_file() {
    let tmp = setup_export_project();
    let output = run_fmm(tmp.path(), &["exports", "--file", "src/app.ts", "--json"]);

    assert!(
        output.status.success(),
        "fmm exports failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let exports = json["exports"].as_array().unwrap();

    assert_eq!(json["file"], "src/app.ts");
    assert_eq!(exports.len(), 2);
    let create_app = exports
        .iter()
        .find(|entry| entry["name"] == "createApp")
        .unwrap();
    let app_version = exports
        .iter()
        .find(|entry| entry["name"] == "APP_VERSION")
        .unwrap();

    assert_eq!(create_app["file"], "src/app.ts");
    assert_eq!(create_app["lines"], serde_json::json!([1, 3]));
    assert_eq!(app_version["file"], "src/app.ts");
    assert_eq!(app_version["lines"], serde_json::json!([5, 5]));
}

#[test]
fn exports_file_missing_file_returns_clear_error() {
    let tmp = setup_export_project();
    let output = run_fmm(tmp.path(), &["exports", "--file", "src/missing.ts"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("File 'src/missing.ts' not found in manifest"),
        "got: {stderr}"
    );
}

#[test]
fn exports_file_rejects_pattern_conflict() {
    let tmp = setup_export_project();
    let output = run_fmm(tmp.path(), &["exports", "App", "--file", "src/app.ts"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--file cannot be combined with a pattern"),
        "got: {stderr}"
    );
}

#[test]
fn exports_file_rejects_directory_conflict() {
    let tmp = setup_export_project();
    let output = run_fmm(
        tmp.path(),
        &["exports", "--file", "src/app.ts", "--dir", "src/"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--file cannot be combined with --dir"),
        "got: {stderr}"
    );
}

#[test]
fn exports_help_documents_file_flag() {
    let output = Command::cargo_bin("fmm")
        .unwrap()
        .args(["exports", "--help"])
        .output()
        .expect("failed to run fmm exports --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--file <FILE>"), "got: {stdout}");
    assert!(
        stdout.contains("returns all exports from this file"),
        "got: {stdout}"
    );
}
