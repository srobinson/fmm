use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::collections::HashSet;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn setup_search_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/auth/session.ts",
        "import jwt from 'jwt';\nimport redis from 'redis';\nimport { Types } from './types';\nimport { Config } from '../config';\n\nexport function createSession() {\n  return jwt.sign({});\n}\n\nexport function validateSession(token: string) {\n  return jwt.verify(token);\n}\n",
    );

    write_file(
        root,
        "src/auth/types.ts",
        "export interface SessionToken {\n  token: string;\n  expires: number;\n}\n\nexport type UserRole = 'admin' | 'user';\n",
    );

    write_file(
        root,
        "src/config.ts",
        "import dotenv from 'dotenv';\n\nexport function loadConfig() {\n  dotenv.config();\n  return {};\n}\n\nexport interface AppConfig {\n  port: number;\n}\n",
    );

    write_file(
        root,
        "src/db/pool.ts",
        "import pg from 'pg';\nimport { Config } from '../config';\n\nexport class Pool {\n  private client: pg.Client;\n}\n\nexport function createPool() {\n  return new Pool();\n}\n",
    );

    write_file(
        root,
        "src/utils/crypto.ts",
        "import bcrypt from 'bcrypt';\n\nexport function hashPassword(pw: string) {\n  return bcrypt.hash(pw, 10);\n}\n\nexport function verifyPassword(pw: string, hash: string) {\n  return bcrypt.compare(pw, hash);\n}\n",
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

fn parse_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "fmm search failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn search_limit_caps_bare_fuzzy_exports() {
    let tmp = setup_search_project();
    let output = run_fmm(tmp.path(), &["search", "session", "--limit", "2", "--json"]);
    let json = parse_json(&output);
    let exports = json["exports"].as_array().unwrap();

    assert_eq!(exports.len(), 2, "got: {json:#}");
    assert!(exports.iter().all(|entry| {
        entry["name"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("session")
    }));
}

#[test]
fn search_min_max_loc_filters_json_results() {
    let tmp = setup_search_project();
    let output = run_fmm(
        tmp.path(),
        &["search", "--min-loc", "7", "--max-loc", "10", "--json"],
    );
    let json = parse_json(&output);
    let files: HashSet<&str> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["file"].as_str().unwrap())
        .collect();

    assert!(files.contains("src/config.ts"), "got: {json:#}");
    assert!(files.contains("src/db/pool.ts"), "got: {json:#}");
    assert!(files.contains("src/utils/crypto.ts"), "got: {json:#}");
    assert!(!files.contains("src/auth/session.ts"), "got: {json:#}");
    assert!(!files.contains("src/auth/types.ts"), "got: {json:#}");
}

#[test]
fn search_loc_shorthand_still_filters() {
    let tmp = setup_search_project();
    let output = run_fmm(tmp.path(), &["search", "--loc", ">10", "--json"]);
    let json = parse_json(&output);
    let results = json.as_array().unwrap();

    assert!(!results.is_empty(), "got: {json:#}");
    assert!(
        results
            .iter()
            .all(|entry| entry["loc"].as_u64().unwrap() > 10)
    );
}

#[test]
fn search_term_and_mcp_loc_filters_intersect() {
    let tmp = setup_search_project();
    let output = run_fmm(
        tmp.path(),
        &[
            "search",
            "session",
            "--imports",
            "jwt",
            "--min-loc",
            "10",
            "--json",
        ],
    );
    let json = parse_json(&output);
    let exports: HashSet<&str> = json["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();

    assert!(exports.contains("createSession"), "got: {json:#}");
    assert!(exports.contains("validateSession"), "got: {json:#}");
    assert!(!exports.contains("SessionToken"), "got: {json:#}");
}

#[test]
fn search_loc_conflicts_with_min_loc() {
    let tmp = TempDir::new().unwrap();
    let output = run_fmm(tmp.path(), &["search", "--loc", ">10", "--min-loc", "5"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--loc cannot be combined with --min-loc or --max-loc"),
        "got: {stderr}"
    );
}

#[test]
fn search_help_documents_mcp_flags() {
    let output = Command::cargo_bin("fmm")
        .unwrap()
        .args(["search", "--help"])
        .output()
        .expect("failed to run fmm search --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--limit <LIMIT>"), "got: {stdout}");
    assert!(stdout.contains("--min-loc <MIN_LOC>"), "got: {stdout}");
    assert!(stdout.contains("--max-loc <MAX_LOC>"), "got: {stdout}");
}
