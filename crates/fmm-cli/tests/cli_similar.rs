use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn setup_similarity_project() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/a.ts",
        "export function extractImports(src: string): string[] {\n  return [];\n}\n",
    );
    write_file(
        root,
        "src/b.ts",
        "export function collectImports(src: string): string[] {\n  return [];\n}\n",
    );
    write_file(
        root,
        "src/extractImports.spec.ts",
        "export function collectImportsFromSpec(src: string): string[] {\n  return [];\n}\n",
    );
    write_file(
        root,
        "packages/other/imports.ts",
        "export function mergeImports(src: string): string[] {\n  return [];\n}\n",
    );

    run_fmm(root, &["generate", "--quiet"]);
    tmp
}

fn run_fmm(root: &Path, args: &[&str]) -> Output {
    Command::cargo_bin("fmm")
        .unwrap()
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn similar_cli_text_excludes_tests_by_default() {
    let tmp = setup_similarity_project();
    let text = stdout(&run_fmm(tmp.path(), &["similar", "extractImports"]));

    assert!(text.contains("collectImports"), "got:\n{text}");
    assert!(!text.contains("collectImportsFromSpec"), "got:\n{text}");
}

#[test]
fn similar_cli_json_returns_matches() {
    let tmp = setup_similarity_project();
    let text = stdout(&run_fmm(
        tmp.path(),
        &["similar", "extractImports", "--json"],
    ));

    let json: Value = serde_json::from_str(&text).unwrap();
    let matches = json.as_array().unwrap();
    assert!(
        matches.iter().any(|m| m["name"] == "collectImports"),
        "got: {json:#}"
    );
    assert!(
        matches
            .iter()
            .all(|m| m["name"] != "collectImportsFromSpec"),
        "got: {json:#}"
    );
}

#[test]
fn similar_cli_directory_scopes_candidates() {
    let tmp = setup_similarity_project();
    let text = stdout(&run_fmm(
        tmp.path(),
        &["similar", "extractImports", "--directory", "packages/other"],
    ));

    assert!(text.contains("mergeImports"), "got:\n{text}");
    assert!(!text.contains("collectImports"), "got:\n{text}");
}

#[test]
fn similar_cli_include_tests_restores_test_candidates() {
    let tmp = setup_similarity_project();
    let text = stdout(&run_fmm(
        tmp.path(),
        &["similar", "extractImports", "--include-tests"],
    ));

    assert!(text.contains("collectImportsFromSpec"), "got:\n{text}");
}
