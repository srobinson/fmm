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

fn db_exists(base: &Path) -> bool {
    base.join(".fmm.db").exists()
}

fn db_indexed(base: &Path, relative: &str) -> bool {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE path = ?1",
            rusqlite::params![relative],
            |row| row.get(0),
        )
        .unwrap_or(0);
    count > 0
}

fn db_export_count(base: &Path, relative: &str) -> i64 {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    conn.query_row(
        "SELECT COUNT(*) FROM exports WHERE file_path = ?1",
        rusqlite::params![relative],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

fn db_has_export(base: &Path, relative: &str, export_name: &str) -> bool {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM exports WHERE file_path = ?1 AND name = ?2",
            rusqlite::params![relative, export_name],
            |row| row.get(0),
        )
        .unwrap_or(0);
    count > 0
}

fn db_file_count(base: &Path) -> i64 {
    let conn = rusqlite::Connection::open(base.join(".fmm.db")).unwrap();
    conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap_or(0)
}

#[test]
fn generate_creates_db() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    assert!(db_exists(tmp.path()));
    assert!(db_indexed(tmp.path(), "src/auth.ts"));
    assert!(db_indexed(tmp.path(), "src/db.ts"));
    assert!(db_indexed(tmp.path(), "src/utils.py"));
}

#[test]
fn generate_indexes_exports() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    assert!(db_has_export(tmp.path(), "src/auth.ts", "validateUser"));
    assert!(db_has_export(tmp.path(), "src/auth.ts", "AuthService"));
}

#[test]
fn generate_skips_unchanged_files() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();
    let count_before = db_export_count(tmp.path(), "src/auth.ts");

    // Generate again — source unchanged, export count should be identical
    fmm::cli::generate(&[path.to_string()], false, false).unwrap();
    let count_after = db_export_count(tmp.path(), "src/auth.ts");

    assert_eq!(count_before, count_after);
}

#[test]
fn generate_updates_stale_files() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    // Modify source to add a new export
    let auth_path = tmp.path().join("src/auth.ts");
    let mut content = fs::read_to_string(&auth_path).unwrap();
    content.push_str("\nexport const NEW_EXPORT = true;\n");
    fs::write(&auth_path, content).unwrap();

    // Generate again — should detect the change and update
    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    assert!(db_has_export(tmp.path(), "src/auth.ts", "NEW_EXPORT"));
}

#[test]
fn generate_dry_run_creates_no_files() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], true, false).unwrap();

    assert!(!db_exists(tmp.path()));
}

#[test]
fn generate_dry_run_preserves_stale_db() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    // Modify source
    let auth_path = tmp.path().join("src/auth.ts");
    let mut content = fs::read_to_string(&auth_path).unwrap();
    content.push_str("\nexport const DRY_RUN_TEST = true;\n");
    fs::write(&auth_path, content).unwrap();

    fmm::cli::generate(&[path.to_string()], true, false).unwrap();

    // DB should NOT contain the new export (dry run)
    assert!(!db_has_export(tmp.path(), "src/auth.ts", "DRY_RUN_TEST"));
}

#[test]
fn validate_passes_after_generate() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();
    let result = fmm::cli::validate(&[path.to_string()]);
    assert!(result.is_ok());
}

#[test]
fn validate_fails_after_source_change() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    // Modify source to add a new export
    let auth_path = tmp.path().join("src/auth.ts");
    let mut content = fs::read_to_string(&auth_path).unwrap();
    content.push_str("\nexport function newFunction() {}\n");
    fs::write(&auth_path, content).unwrap();

    let result = fmm::cli::validate(&[path.to_string()]);
    assert!(result.is_err());
}

#[test]
fn clean_clears_db() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();
    assert!(db_indexed(tmp.path(), "src/auth.ts"));

    fmm::cli::clean(&[path.to_string()], false, false).unwrap();

    // DB file remains but all rows are cleared
    assert_eq!(db_file_count(tmp.path()), 0);
}

#[test]
fn clean_dry_run_preserves_db() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    fmm::cli::clean(&[path.to_string()], true, false).unwrap();

    // Files should still be indexed
    assert!(db_indexed(tmp.path(), "src/auth.ts"));
    assert!(db_indexed(tmp.path(), "src/db.ts"));
}

#[test]
fn full_workflow_generate_validate_clean() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    // Generate
    fmm::cli::generate(&[path.to_string()], false, false).unwrap();
    assert!(db_indexed(tmp.path(), "src/auth.ts"));

    // Validate (should pass)
    fmm::cli::validate(&[path.to_string()]).unwrap();

    // Modify source and re-generate
    let db_path = tmp.path().join("src/db.ts");
    fs::write(
        &db_path,
        "export function newConnect() {}\nexport const NEW_SIZE = 20;\n",
    )
    .unwrap();
    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    // Validate again (should pass after generate updates stale entry)
    fmm::cli::validate(&[path.to_string()]).unwrap();

    // Clean
    fmm::cli::clean(&[path.to_string()], false, false).unwrap();
    assert_eq!(db_file_count(tmp.path()), 0);
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

    // Create .gitignore that ignores utils.py
    fs::write(tmp.path().join(".gitignore"), "src/utils.py\n").unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    // TypeScript files should be indexed
    assert!(db_indexed(tmp.path(), "src/auth.ts"));
    assert!(db_indexed(tmp.path(), "src/db.ts"));
    // Python file should be ignored
    assert!(!db_indexed(tmp.path(), "src/utils.py"));
}

#[test]
fn respects_fmmignore() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    // Create .fmmignore that ignores db.ts
    fs::write(tmp.path().join(".fmmignore"), "src/db.ts\n").unwrap();

    fmm::cli::generate(&[path.to_string()], false, false).unwrap();

    assert!(db_indexed(tmp.path(), "src/auth.ts"));
    assert!(!db_indexed(tmp.path(), "src/db.ts"));
    assert!(db_indexed(tmp.path(), "src/utils.py"));
}

#[test]
fn single_file_generate() {
    let tmp = setup_project();
    let file_path = tmp.path().join("src/auth.ts");
    // When a file path is passed, root resolves to the file's parent directory
    // (no .git/.fmmrc.json in a temp dir, so root = src/).
    let src_dir = tmp.path().join("src");

    fmm::cli::generate(&[file_path.to_str().unwrap().to_string()], false, false).unwrap();

    // DB is at src/.fmm.db; file path stored relative to src/
    assert!(db_indexed(&src_dir, "auth.ts"));
    // Other files should not be indexed
    assert!(!db_indexed(&src_dir, "db.ts"));
}
