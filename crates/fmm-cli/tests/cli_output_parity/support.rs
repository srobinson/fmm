use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

pub(super) fn ensure_repo_index() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let root = repo_root();
        let _guard = RepoIndexLock::acquire(&root);
        if repo_index_current(&root) {
            return;
        }

        let output = Command::cargo_bin("fmm")
            .unwrap()
            .arg("generate")
            .current_dir(root)
            .output()
            .expect("failed to run fmm generate");
        assert!(
            output.status.success(),
            "fmm generate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    });
}

fn repo_index_current(root: &Path) -> bool {
    Command::cargo_bin("fmm")
        .unwrap()
        .arg("validate")
        .current_dir(root)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

struct RepoIndexLock {
    path: PathBuf,
}

impl RepoIndexLock {
    fn acquire(root: &Path) -> Self {
        let path = root.join(".fmm.test-generate.lock");
        let start = Instant::now();

        loop {
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    assert!(
                        start.elapsed() <= Duration::from_secs(30),
                        "timed out waiting for repo index lock at {}",
                        path.display()
                    );
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => panic!("failed to create repo index lock: {error}"),
            }
        }
    }
}

impl Drop for RepoIndexLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
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
