//! CLI integration tests for the generate → validate → clean workflow.
//!
//! These tests exercise fmm's public CLI functions end-to-end using real
//! temp directories with actual source files.

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Create a temp directory with sample source files for testing.
fn setup_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("auth.ts"),
        r#"import { hash } from 'bcrypt';
import { Pool } from './db';

export function validateUser(token: string): boolean {
    return token.length > 0;
}

export class AuthService {
    private pool: Pool;
}
"#,
    )
    .unwrap();

    fs::write(
        src.join("db.ts"),
        r#"import pg from 'pg';

export function connect(): void {}
export const POOL_SIZE = 10;
"#,
    )
    .unwrap();

    fs::write(
        src.join("utils.py"),
        r#"import os
import json

def parse_config(path: str) -> dict:
    with open(path) as f:
        return json.load(f)

def format_output(data: dict) -> str:
    return json.dumps(data, indent=2)
"#,
    )
    .unwrap();

    tmp
}

fn sidecar_exists(base: &Path, relative: &str) -> bool {
    let sidecar = base.join(format!("{}.fmm", relative));
    sidecar.exists()
}

fn sidecar_content(base: &Path, relative: &str) -> String {
    let sidecar = base.join(format!("{}.fmm", relative));
    fs::read_to_string(sidecar).unwrap()
}

#[test]
fn generate_creates_sidecars() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(sidecar_exists(tmp.path(), "src/db.ts"));
    assert!(sidecar_exists(tmp.path(), "src/utils.py"));
}

#[test]
fn generate_sidecar_content_is_valid_yaml() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    let content = sidecar_content(tmp.path(), "src/auth.ts");
    assert!(content.contains("file:"));
    assert!(content.contains("exports:"));
    assert!(content.contains("validateUser"));
    assert!(content.contains("AuthService"));
}

#[test]
fn generate_skips_unchanged_sidecars() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    let sidecar_path = tmp.path().join("src/auth.ts.fmm");
    let content_before = fs::read_to_string(&sidecar_path).unwrap();

    // Generate again — source unchanged, sidecar should be identical
    fmm::cli::generate(path, false).unwrap();

    let content_after = fs::read_to_string(&sidecar_path).unwrap();
    assert_eq!(content_before, content_after);
}

#[test]
fn generate_updates_stale_sidecars() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    // Modify source to add a new export
    let auth_path = tmp.path().join("src/auth.ts");
    let mut content = fs::read_to_string(&auth_path).unwrap();
    content.push_str("\nexport const NEW_EXPORT = true;\n");
    fs::write(&auth_path, content).unwrap();

    // Generate again — should detect the change and update
    fmm::cli::generate(path, false).unwrap();

    let sidecar = sidecar_content(tmp.path(), "src/auth.ts");
    assert!(sidecar.contains("NEW_EXPORT"));
}

#[test]
fn generate_dry_run_creates_no_files() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, true).unwrap();

    assert!(!sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(!sidecar_exists(tmp.path(), "src/db.ts"));
}

#[test]
fn generate_dry_run_preserves_stale_sidecars() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    // Modify source
    let auth_path = tmp.path().join("src/auth.ts");
    let mut content = fs::read_to_string(&auth_path).unwrap();
    content.push_str("\nexport const DRY_RUN_TEST = true;\n");
    fs::write(&auth_path, content).unwrap();

    fmm::cli::generate(path, true).unwrap();

    // Sidecar should NOT contain the new export (dry run)
    let sidecar = sidecar_content(tmp.path(), "src/auth.ts");
    assert!(!sidecar.contains("DRY_RUN_TEST"));
}

#[test]
fn validate_passes_after_generate() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();
    let result = fmm::cli::validate(path);
    assert!(result.is_ok());
}

#[test]
fn validate_fails_after_source_change() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    // Modify source to add a new export
    let auth_path = tmp.path().join("src/auth.ts");
    let mut content = fs::read_to_string(&auth_path).unwrap();
    content.push_str("\nexport function newFunction() {}\n");
    fs::write(&auth_path, content).unwrap();

    let result = fmm::cli::validate(path);
    assert!(result.is_err());
}

#[test]
fn clean_removes_all_sidecars() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();
    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));

    fmm::cli::clean(path, false).unwrap();

    assert!(!sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(!sidecar_exists(tmp.path(), "src/db.ts"));
    assert!(!sidecar_exists(tmp.path(), "src/utils.py"));
}

#[test]
fn clean_dry_run_preserves_files() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(path, false).unwrap();

    fmm::cli::clean(path, true).unwrap();

    // Files should still exist
    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(sidecar_exists(tmp.path(), "src/db.ts"));
}

#[test]
fn full_workflow_generate_validate_clean() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    // Generate
    fmm::cli::generate(path, false).unwrap();
    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));

    // Validate (should pass)
    fmm::cli::validate(path).unwrap();

    // Modify source and re-generate (replaces old update step)
    let db_path = tmp.path().join("src/db.ts");
    fs::write(
        &db_path,
        "export function newConnect() {}\nexport const NEW_SIZE = 20;\n",
    )
    .unwrap();
    fmm::cli::generate(path, false).unwrap();

    // Validate again (should pass after generate updates stale sidecars)
    fmm::cli::validate(path).unwrap();

    // Clean
    fmm::cli::clean(path, false).unwrap();
    assert!(!sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(!sidecar_exists(tmp.path(), "src/db.ts"));
}

#[test]
fn respects_gitignore() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    // The ignore crate only respects .gitignore inside a git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init failed");

    // Create .gitignore that ignores the utils.py
    fs::write(tmp.path().join(".gitignore"), "src/utils.py\n").unwrap();

    fmm::cli::generate(path, false).unwrap();

    // TypeScript files should have sidecars
    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(sidecar_exists(tmp.path(), "src/db.ts"));
    // Python file should be ignored
    assert!(!sidecar_exists(tmp.path(), "src/utils.py"));
}

#[test]
fn respects_fmmignore() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    // Create .fmmignore that ignores db.ts
    fs::write(tmp.path().join(".fmmignore"), "src/db.ts\n").unwrap();

    fmm::cli::generate(path, false).unwrap();

    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));
    assert!(!sidecar_exists(tmp.path(), "src/db.ts"));
    assert!(sidecar_exists(tmp.path(), "src/utils.py"));
}

#[test]
fn single_file_generate() {
    let tmp = setup_project();
    let file_path = tmp.path().join("src/auth.ts");

    fmm::cli::generate(file_path.to_str().unwrap(), false).unwrap();

    // Only the targeted file gets a sidecar
    assert!(sidecar_exists(tmp.path(), "src/auth.ts"));
    // Other files should not have sidecars
    assert!(!sidecar_exists(tmp.path(), "src/db.ts"));
}
