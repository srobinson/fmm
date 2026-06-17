use assert_cmd::cargo::CommandCargoExt;
use fmm_core::store::{GIT_BRANCH_META_KEY, GIT_DIRTY_META_KEY, GIT_SHA_META_KEY};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn setup_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("auth.ts"),
        "export function validateUser(token: string): boolean { return token.length > 0; }\n",
    )
    .unwrap();
    tmp
}

fn db_meta(base: &Path, key: &str) -> Option<String> {
    let conn = fmm_store::open_db(base).unwrap();
    fmm_store::connection::read_meta(&conn, key).unwrap()
}

fn init_git_repo(base: &Path) -> String {
    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(base)
        .output()
        .expect("git init failed");
    if !init.status.success() {
        run_git(base, &["init"]);
        run_git(base, &["checkout", "-B", "main"]);
    }
    run_git(base, &["config", "user.email", "fmm@example.test"]);
    run_git(base, &["config", "user.name", "fmm test"]);
    run_git(base, &["add", "."]);
    run_git(base, &["commit", "-m", "init"]);
    run_git(base, &["rev-parse", "HEAD"])
}

fn run_git(base: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(base)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn generate_without_git_leaves_git_metadata_unstamped() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();

    fmm::cli::generate(&[path.to_string()], false, false, true).unwrap();

    assert_eq!(db_meta(tmp.path(), GIT_SHA_META_KEY), None);
}

#[test]
fn generate_persists_git_metadata() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();
    let sha = init_git_repo(tmp.path());

    fmm::cli::generate(&[path.to_string()], false, false, true).unwrap();

    assert_eq!(
        db_meta(tmp.path(), GIT_SHA_META_KEY).as_deref(),
        Some(sha.as_str())
    );
    assert_eq!(
        db_meta(tmp.path(), GIT_BRANCH_META_KEY).as_deref(),
        Some("main")
    );
    assert_eq!(
        db_meta(tmp.path(), GIT_DIRTY_META_KEY).as_deref(),
        Some("false")
    );
}

#[test]
fn generate_sha_override_is_persisted_verbatim() {
    let tmp = setup_project();
    init_git_repo(tmp.path());

    let generate = Command::cargo_bin("fmm")
        .unwrap()
        .args(["generate", "--sha", "override-sha", "--quiet", "."])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(
        generate.status.success(),
        "{}",
        String::from_utf8_lossy(&generate.stderr)
    );

    assert_eq!(
        db_meta(tmp.path(), GIT_SHA_META_KEY).as_deref(),
        Some("override-sha")
    );
    assert_eq!(
        db_meta(tmp.path(), GIT_BRANCH_META_KEY).as_deref(),
        Some("main")
    );
    assert_eq!(
        db_meta(tmp.path(), GIT_DIRTY_META_KEY).as_deref(),
        Some("false")
    );
}

#[test]
fn generate_no_git_clears_git_metadata() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();
    init_git_repo(tmp.path());

    fmm::cli::generate(&[path.to_string()], false, false, true).unwrap();
    fmm::cli::generate_with_git(&[path.to_string()], false, false, true, None, true).unwrap();

    assert_eq!(db_meta(tmp.path(), GIT_SHA_META_KEY), None);
    assert_eq!(db_meta(tmp.path(), GIT_BRANCH_META_KEY), None);
    assert_eq!(db_meta(tmp.path(), GIT_DIRTY_META_KEY), None);
}

#[test]
fn generate_records_dirty_git_metadata() {
    let tmp = setup_project();
    let path = tmp.path().to_str().unwrap();
    init_git_repo(tmp.path());
    fs::write(tmp.path().join("dirty.ts"), "export const dirty = true;\n").unwrap();

    fmm::cli::generate(&[path.to_string()], false, false, true).unwrap();

    assert_eq!(
        db_meta(tmp.path(), GIT_DIRTY_META_KEY).as_deref(),
        Some("true")
    );
}

#[test]
fn status_prints_git_metadata_section() {
    let tmp = setup_project();
    init_git_repo(tmp.path());
    let generate = Command::cargo_bin("fmm")
        .unwrap()
        .args(["generate", "--quiet", "."])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(
        generate.status.success(),
        "{}",
        String::from_utf8_lossy(&generate.stderr)
    );

    let status = Command::cargo_bin("fmm")
        .unwrap()
        .arg("status")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("Git Metadata:"));
    assert!(stdout.contains("SHA:"));
    assert!(stdout.contains("Branch: main"));
    assert!(stdout.contains("Dirty: clean"));
}
