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

fn setup_read_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/service.ts",
        "export class SecretService {\n  private computeSecret(seed: string): string {\n    return seed.toUpperCase();\n  }\n\n  public run(seed: string): string {\n    return this.computeSecret(seed);\n  }\n}\n\nexport function createSecretService(): SecretService {\n  return new SecretService();\n}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn setup_scopepath_collision_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "crates/cm-core/src/types.rs",
        "pub struct ScopePath(String);\n\nimpl ScopePath {\n    pub fn parse(input: &str) -> Result<Self, String> {\n        let path = Self(input.to_string());\n        path.validate()?;\n        Ok(path)\n    }\n\n    fn validate(&self) -> Result<(), String> {\n        if self.0.is_empty() {\n            return Err(\"empty\".to_string());\n        }\n        Ok(())\n    }\n}\n",
    );
    write_file(
        root,
        "crates/cm-web/frontend/src/api/generated/ScopePath.ts",
        "export type ScopePath = string;\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn setup_large_class_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let mut source = String::from("export class BigService {\n");
    for i in 0..150usize {
        source.push_str(&format!(
            "  doWork{i:03}(input: string): string {{\n    // perform operation {i:03}\n    return input;\n  }}\n"
        ));
    }
    source.push_str("}\n");

    assert!(
        source.len() > 10_240,
        "test source must exceed 10KB, got {} bytes",
        source.len()
    );

    write_file(root, "src/big-service.ts", &source);
    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn setup_unicode_truncation_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let prefix = "export function unicodeBlob(): string {\n  return \"";
    let marker = "\u{00e9}";
    let filler = "a".repeat(10_240 - prefix.len() - 1);
    let source = format!("{prefix}{filler}{marker}\";\n}}\n");

    write_file(root, "src/unicode.ts", &source);
    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

fn parse_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn read_missing_export_suggests_cli_commands() {
    let tmp = setup_read_project();
    let output = run_fmm(tmp.path(), &["read", "NoSuchSymbol"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for missing export"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Export 'NoSuchSymbol' not found"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("Use fmm exports or fmm search"),
        "got: {stderr}"
    );
    assert!(!stderr.contains("fmm_list_exports"), "got: {stderr}");
    assert!(!stderr.contains("fmm_search"), "got: {stderr}");
}

#[test]
fn read_missing_method_suggests_cli_outline() {
    let tmp = setup_read_project();
    let output = run_fmm(tmp.path(), &["read", "SecretService.missing"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for missing method"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Method 'SecretService.missing' not found"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("Use fmm outline src/service.ts --include-private"),
        "got: {stderr}"
    );
    assert!(!stderr.contains("fmm_file_outline"), "got: {stderr}");
}

#[test]
fn read_public_symbol_still_works_with_line_numbers() {
    let tmp = setup_read_project();
    let output = run_fmm(
        tmp.path(),
        &["read", "createSecretService", "--line-numbers"],
    );

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("symbol: createSecretService"),
        "got: {stdout}"
    );
    assert!(
        stdout.contains("11  export function createSecretService"),
        "got: {stdout}"
    );
    assert!(
        stdout.contains("12    return new SecretService()"),
        "got: {stdout}"
    );
}

#[test]
fn read_private_method_uses_outline_fallback() {
    let tmp = setup_read_project();
    let output = run_fmm(tmp.path(), &["read", "SecretService.computeSecret"]);

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("symbol: SecretService.computeSecret"),
        "got: {stdout}"
    );
    assert!(stdout.contains("private computeSecret"), "got: {stdout}");
    assert!(
        stdout.contains("return seed.toUpperCase()"),
        "got: {stdout}"
    );
    assert!(!stdout.contains("public run"), "got: {stdout}");
}

#[test]
fn read_public_method_still_uses_method_index() {
    let tmp = setup_read_project();
    let output = run_fmm(tmp.path(), &["read", "SecretService.run"]);

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("symbol: SecretService.run"),
        "got: {stdout}"
    );
    assert!(stdout.contains("public run"), "got: {stdout}");
    assert!(!stdout.contains("private computeSecret"), "got: {stdout}");
}

#[test]
fn read_file_qualified_public_method_uses_requested_file() {
    let tmp = setup_read_project();
    let output = run_fmm(tmp.path(), &["read", "src/service.ts:SecretService.run"]);

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("symbol: 'src/service.ts:SecretService.run'"),
        "got: {stdout}"
    );
    assert!(stdout.contains("public run"), "got: {stdout}");
    assert!(!stdout.contains("private computeSecret"), "got: {stdout}");
}

#[test]
fn read_file_qualified_private_rust_method_uses_requested_file() {
    let tmp = setup_scopepath_collision_project();
    let output = run_fmm(
        tmp.path(),
        &[
            "read",
            "crates/cm-core/src/types.rs:ScopePath.validate",
            "--line-numbers",
        ],
    );

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("symbol: 'crates/cm-core/src/types.rs:ScopePath.validate'"),
        "got: {stdout}"
    );
    assert!(stdout.contains("10      fn validate"), "got: {stdout}");
    assert!(!stdout.contains("pub fn parse"), "got: {stdout}");
}

#[test]
fn read_private_method_searches_duplicate_class_exports() {
    let tmp = setup_scopepath_collision_project();
    let output = run_fmm(
        tmp.path(),
        &["read", "ScopePath.validate", "--line-numbers"],
    );

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("symbol: ScopePath.validate"),
        "got: {stdout}"
    );
    assert!(
        stdout.contains("file: crates/cm-core/src/types.rs"),
        "got: {stdout}"
    );
    assert!(stdout.contains("10      fn validate"), "got: {stdout}");
    assert!(!stdout.contains("export type ScopePath"), "got: {stdout}");
}

#[test]
fn read_large_bare_class_returns_method_redirect() {
    let tmp = setup_large_class_project();
    let output = run_fmm(tmp.path(), &["read", "BigService"]);

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("would exceed the 10KB response cap"),
        "got: {stdout}"
    );
    assert!(stdout.contains("methods:"), "got: {stdout}");
    assert!(stdout.contains("doWork000"), "got: {stdout}");
    assert!(
        stdout.contains("fmm_read_symbol(\"BigService.doWork000\")"),
        "got: {stdout}"
    );
    assert!(!stdout.contains("return input"), "got: {stdout}");
}

#[test]
fn read_large_bare_class_no_truncate_returns_source() {
    let tmp = setup_large_class_project();
    let output = run_fmm(tmp.path(), &["read", "BigService", "--no-truncate"]);

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!stdout.contains("would exceed"), "got: {stdout}");
    assert!(stdout.contains("return input"), "got: {stdout}");
}

#[test]
fn read_truncates_unicode_source_at_char_boundary() {
    let tmp = setup_unicode_truncation_project();
    let output = run_fmm(tmp.path(), &["read", "unicodeBlob"]);

    assert!(
        output.status.success(),
        "fmm read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("symbol: unicodeBlob"), "got: {stdout}");
    assert!(
        stdout.contains("truncated, use --no-truncate"),
        "got: {stdout}"
    );
}

#[test]
fn read_large_bare_class_json_returns_redirect_shape() {
    let tmp = setup_large_class_project();
    let output = run_fmm(tmp.path(), &["read", "BigService", "--json"]);
    let json = parse_json(&output);

    assert_eq!(json["kind"], "class_redirect", "got: {json:#}");
    assert_eq!(json["symbol"], "BigService", "got: {json:#}");
    assert_eq!(json["file"], "src/big-service.ts", "got: {json:#}");
    assert!(json.get("source").is_none(), "got: {json:#}");

    let methods = json["methods"].as_array().unwrap();
    assert!(!methods.is_empty(), "got: {json:#}");
    assert_eq!(methods[0]["name"], "doWork000", "got: {json:#}");
    assert_eq!(
        methods[0]["lines"],
        serde_json::json!([2, 5]),
        "got: {json:#}"
    );
}

#[test]
fn read_large_bare_class_no_truncate_json_returns_source_shape() {
    let tmp = setup_large_class_project();
    let output = run_fmm(
        tmp.path(),
        &["read", "BigService", "--no-truncate", "--json"],
    );
    let json = parse_json(&output);

    assert_eq!(json["kind"], "source", "got: {json:#}");
    assert_eq!(json["symbol"], "BigService", "got: {json:#}");
    assert!(json.get("methods").is_none(), "got: {json:#}");
    assert!(
        json["source"].as_str().unwrap().contains("return input"),
        "got: {json:#}"
    );
}
