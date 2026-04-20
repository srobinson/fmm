use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn run_fmm(root: &std::path::Path, args: &[&str]) -> Output {
    Command::cargo_bin("fmm")
        .unwrap()
        .args(args)
        .current_dir(root)
        .output()
        .expect("failed to run fmm")
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "fmm glossary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn setup_precision_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/WorkLoop.ts",
        "export function scheduleUpdate(root: any) {}\nexport function requestLane(root: any) { return 0; }\nexport function peekLane() { return 0; }\n",
    );
    write_file(
        root,
        "src/HooksModule.ts",
        "import { scheduleUpdate } from './WorkLoop';\nexport function mountEffect(fiber: any) { scheduleUpdate(fiber); }\n",
    );
    write_file(
        root,
        "src/ProfilerTimer.ts",
        "import { requestLane } from './WorkLoop';\nexport function getTimerLane() { return requestLane(null); }\n",
    );
    write_file(
        root,
        "src/ClassComponent.ts",
        "import { scheduleUpdate, requestLane } from './WorkLoop';\nexport function updateComponent(comp: any) {\n  const lane = requestLane(comp);\n  scheduleUpdate(comp);\n}\n",
    );
    write_file(
        root,
        "src/ReExporter.ts",
        "export { scheduleUpdate } from './WorkLoop';\n",
    );
    write_file(
        root,
        "src/DeadImport.ts",
        "import { scheduleUpdate } from './WorkLoop';\nexport const MARKER = 42;\n",
    );
    write_file(
        root,
        "src/NamespaceUser.ts",
        "import * as WL from './WorkLoop';\nexport function debugSchedule(root: any) { WL.scheduleUpdate(root); }\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn setup_many_exports_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    for i in 1..=55 {
        write_file(
            root,
            &format!("src/items/item{i:02}.ts"),
            &format!("export const item{i:02} = {i};\n"),
        );
    }

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn setup_large_glossary_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/api/massive.ts",
        "export function massiveSymbol() { return 1; }\n",
    );
    for i in 0..400 {
        write_file(
            root,
            &format!("src/callers/caller_{i:03}_with_long_descriptive_name.ts"),
            "import { massiveSymbol } from '../api/massive';\nexport function run() { return massiveSymbol(); }\n",
        );
    }

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

#[test]
fn glossary_named_precision_filters_to_named_importers() {
    let tmp = setup_precision_project();
    let output = run_fmm(
        tmp.path(),
        &["glossary", "scheduleUpdate", "--precision", "named"],
    );
    let text = stdout(&output);

    assert!(text.contains("HooksModule.ts"), "got:\n{text}");
    assert!(text.contains("ClassComponent.ts"), "got:\n{text}");
    assert!(text.contains("DeadImport.ts"), "got:\n{text}");
    assert!(text.contains("ReExporter.ts"), "got:\n{text}");
    assert!(text.contains("NamespaceUser.ts"), "got:\n{text}");
    assert!(text.contains("via namespace import"), "got:\n{text}");
    assert!(
        !text.contains("ProfilerTimer.ts"),
        "different named import should be excluded; got:\n{text}"
    );
}

#[test]
fn glossary_call_site_precision_filters_dead_imports_and_annotates_edges() {
    let tmp = setup_precision_project();
    let output = run_fmm(
        tmp.path(),
        &["glossary", "scheduleUpdate", "--precision", "call-site"],
    );
    let text = stdout(&output);

    assert!(text.contains("HooksModule.ts"), "got:\n{text}");
    assert!(text.contains("ClassComponent.ts"), "got:\n{text}");
    assert!(!text.contains("DeadImport.ts"), "got:\n{text}");
    assert!(!text.contains("ProfilerTimer.ts"), "got:\n{text}");
    assert!(text.contains("ReExporter.ts"), "got:\n{text}");
    assert!(text.contains("re-exports only"), "got:\n{text}");
    assert!(text.contains("NamespaceUser.ts"), "got:\n{text}");
    assert!(text.contains("via namespace import"), "got:\n{text}");
}

#[test]
fn glossary_default_limit_and_hard_cap_match_mcp() {
    let tmp = setup_many_exports_project();

    let default_output = run_fmm(tmp.path(), &["glossary", "item"]);
    let default_text = stdout(&default_output);
    assert!(
        default_text.contains("showing 10/55 matches"),
        "got:\n{default_text}"
    );

    let capped_output = run_fmm(tmp.path(), &["glossary", "item", "--limit", "200"]);
    let capped_text = stdout(&capped_output);
    assert!(
        capped_text.contains("showing 50/55 matches"),
        "got:\n{capped_text}"
    );
}

#[test]
fn glossary_dotted_no_match_uses_standard_empty_output() {
    let tmp = setup_precision_project();
    let output = run_fmm(tmp.path(), &["glossary", "Missing.symbol"]);
    let text = stdout(&output);

    assert!(text.contains("(no matching exports)"), "got:\n{text}");
    assert_ne!(text.trim(), "---");
}

#[test]
fn glossary_no_truncate_bypasses_cli_response_cap() {
    let tmp = setup_large_glossary_project();

    let capped_output = run_fmm(tmp.path(), &["glossary", "massiveSymbol"]);
    let capped_text = stdout(&capped_output);
    assert!(capped_text.contains("[Truncated;"), "got:\n{capped_text}");
    assert!(capped_text.contains("--no-truncate"), "got:\n{capped_text}");

    let full_output = run_fmm(tmp.path(), &["glossary", "massiveSymbol", "--no-truncate"]);
    let full_text = stdout(&full_output);
    assert!(!full_text.contains("[Truncated;"), "got:\n{full_text}");
    assert!(
        full_text.contains("caller_399_with_long_descriptive_name.ts"),
        "got:\n{full_text}"
    );
}

#[test]
fn glossary_json_preserves_precision_annotations() {
    let tmp = setup_precision_project();
    let output = run_fmm(
        tmp.path(),
        &[
            "glossary",
            "scheduleUpdate",
            "--precision",
            "call-site",
            "--json",
        ],
    );
    let text = stdout(&output);
    let json: Value = serde_json::from_str(&text).unwrap();
    let entries = json.as_array().unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry["name"] == "scheduleUpdate")
        .unwrap();
    let source = entry["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["file"] == "src/WorkLoop.ts")
        .unwrap();
    let used_by: Vec<&str> = source["used_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();

    assert!(
        used_by.iter().any(|file| file.ends_with("HooksModule.ts")),
        "got: {json:#}"
    );
    assert!(
        used_by
            .iter()
            .any(|file| file.ends_with("ClassComponent.ts")),
        "got: {json:#}"
    );
    assert!(
        !used_by.iter().any(|file| file.ends_with("DeadImport.ts")),
        "got: {json:#}"
    );
    assert!(
        source["reexport_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file.as_str().unwrap().ends_with("ReExporter.ts")),
        "got: {json:#}"
    );
    assert!(
        source["layer2_namespace_callers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file.as_str().unwrap().ends_with("NamespaceUser.ts")),
        "got: {json:#}"
    );
}

#[test]
fn glossary_help_documents_precision_and_truncation() {
    let output = Command::cargo_bin("fmm")
        .unwrap()
        .args(["glossary", "--help"])
        .output()
        .expect("failed to run fmm glossary --help");

    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("--precision <PRECISION>"), "got: {text}");
    assert!(text.contains("call-site"), "got: {text}");
    assert!(text.contains("--no-truncate"), "got: {text}");
}
