use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn setup_reverse_cycle_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/core.ts",
        "export function core() { return 1; }\n",
    );
    write_file(
        root,
        "src/a.ts",
        "import { core } from './core';\nimport { c } from './c';\nexport function a() { return core() + c(); }\n",
    );
    write_file(
        root,
        "src/b.ts",
        "import { a } from './a';\nexport function b() { return a(); }\n",
    );
    write_file(
        root,
        "src/c.ts",
        "import { b } from './b';\nexport function c() { return b(); }\n",
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

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn deps_reverse_transitive_reports_cycle_safe_closure_and_count() {
    let tmp = setup_reverse_cycle_project();

    let text = stdout(&run_fmm(
        tmp.path(),
        &["deps", "src/core.ts", "--reverse", "--transitive"],
    ));

    assert!(text.contains("reverse: true"), "got:\n{text}");
    assert!(
        text.contains("depth: full (transitive closure)"),
        "got:\n{text}"
    );
    assert!(text.contains("reverse_deps_count: 3"), "got:\n{text}");
    assert!(text.contains("file: src/a.ts  depth: 1"), "got:\n{text}");
    assert!(text.contains("file: src/b.ts  depth: 2"), "got:\n{text}");
    assert!(text.contains("file: src/c.ts  depth: 3"), "got:\n{text}");

    let json_text = stdout(&run_fmm(
        tmp.path(),
        &["deps", "src/core.ts", "--reverse", "--transitive", "--json"],
    ));
    let json: Value = serde_json::from_str(&json_text).unwrap();
    assert_eq!(json["reverse_deps_count"], 3);
    assert_eq!(json["reverse_deps"][0]["file"], "src/a.ts");
    assert_eq!(json["reverse_deps"][0]["depth"], 1);
    assert_eq!(json["reverse_deps"][1]["file"], "src/b.ts");
    assert_eq!(json["reverse_deps"][1]["depth"], 2);
    assert_eq!(json["reverse_deps"][2]["file"], "src/c.ts");
    assert_eq!(json["reverse_deps"][2]["depth"], 3);
}
