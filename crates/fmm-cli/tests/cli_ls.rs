use assert_cmd::cargo::CommandCargoExt;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_file(root: &std::path::Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn setup_ls_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(root, "src/zeta.ts", "export const zeta = 1;\n");
    write_file(root, "src/alpha.ts", "export const alpha = 1;\n");
    write_file(root, "src/middle.ts", "export const middle = 1;\n");

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

fn listed_paths(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("  - "))
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

#[test]
fn ls_sort_by_path_aliases_name_sort() {
    let tmp = setup_ls_project();
    let output = run_fmm(tmp.path(), &["ls", "--sort-by", "path", "--limit", "3"]);

    assert!(
        output.status.success(),
        "fmm ls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        listed_paths(&stdout),
        vec!["src/alpha.ts", "src/middle.ts", "src/zeta.ts"],
        "got: {stdout}"
    );
}
