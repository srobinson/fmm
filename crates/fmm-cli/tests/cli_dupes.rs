//! CLI integration tests for `fmm dupes`.

use assert_cmd::cargo::CommandCargoExt;
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

fn setup_dupes_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    for (file, name) in [
        ("src/a.rs", "format_symbol_signature_end_byte"),
        ("src/b.rs", "format_symbol_signature_end_byte_node"),
    ] {
        write_file(
            tmp.path(),
            file,
            &format!(
                r#"
pub fn {name}() -> String {{
    String::new()
}}
"#
            ),
        );
    }
    for (file, name) in [
        ("src/c.rs", "load_user"),
        ("src/d.rs", "save_order"),
        ("src/e.rs", "parse_config"),
    ] {
        write_file(
            tmp.path(),
            file,
            &format!(
                r#"
pub fn {name}() -> String {{
    String::new()
}}
"#
            ),
        );
    }

    let output = run_fmm(tmp.path(), &["generate"]);
    assert!(
        output.status.success(),
        "fmm generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    tmp
}

#[test]
fn dupes_text_output_matches_snapshot() {
    let tmp = setup_dupes_project();
    let output = run_fmm(tmp.path(), &["dupes"]);
    assert!(
        output.status.success(),
        "fmm dupes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    insta::assert_snapshot!(
        "dupes_text_output",
        String::from_utf8(output.stdout).unwrap()
    );
}

#[test]
fn dupes_json_output_is_byte_stable() {
    let tmp = setup_dupes_project();
    let first = run_fmm(tmp.path(), &["dupes", "--json"]);
    let second = run_fmm(tmp.path(), &["dupes", "--json"]);
    assert!(
        first.status.success(),
        "first fmm dupes --json failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "second fmm dupes --json failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert_eq!(first.stdout, second.stdout);
}
