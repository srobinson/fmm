use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

pub(super) fn ensure_repo_index() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let output = Command::cargo_bin("fmm")
            .unwrap()
            .arg("generate")
            .current_dir(repo_root())
            .output()
            .expect("failed to run fmm generate");
        assert!(
            output.status.success(),
            "fmm generate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    });
}

pub(super) fn call_mcp_text(server: &fmm::mcp::SqliteMcpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

pub(super) fn run_fmm(args: &[&str]) -> Output {
    let output = Command::cargo_bin("fmm")
        .unwrap()
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("failed to run fmm");
    assert!(
        output.status.success(),
        "fmm {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub(super) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under repo/crates/fmm-cli")
        .to_path_buf()
}
