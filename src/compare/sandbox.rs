//! Sandbox management for isolated comparison runs

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Sandbox for isolated repo comparison
pub struct Sandbox {
    /// Root directory for this sandbox
    pub root: PathBuf,
    /// Control variant directory (no FMM)
    pub control_dir: PathBuf,
    /// FMM variant directory (with manifest)
    pub fmm_dir: PathBuf,
    /// Maximum size in MB
    #[allow(dead_code)]
    max_size_mb: u64,
    /// Maximum number of files
    #[allow(dead_code)]
    max_files: u64,
    /// Creation time
    #[allow(dead_code)]
    created_at: Instant,
    /// Time to live
    #[allow(dead_code)]
    ttl: Duration,
    /// Whether to cleanup on drop
    cleanup_on_drop: bool,
}

/// Resource limits for sandbox operations
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResourceLimits {
    /// Max repo size in MB
    pub max_repo_size_mb: u64,
    /// Clone timeout in seconds
    pub clone_timeout_secs: u64,
    /// Max files to parse
    pub max_files_to_parse: u64,
    /// Parse timeout in seconds
    pub parse_timeout_secs: u64,
    /// Task timeout in seconds
    pub task_timeout_secs: u64,
    /// Max total API cost in USD
    pub max_total_api_cost: f64,
    /// Job timeout in seconds
    pub job_timeout_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_repo_size_mb: 500,
            clone_timeout_secs: 300,
            max_files_to_parse: 5000,
            parse_timeout_secs: 120,
            task_timeout_secs: 180,
            max_total_api_cost: 5.0,
            job_timeout_secs: 1800,
        }
    }
}

impl Sandbox {
    /// Create a new sandbox for a job
    pub fn new(job_id: &str) -> Result<Self> {
        validate_job_id(job_id)?;
        let root = std::env::temp_dir().join(format!("fmm-compare-{}", job_id));
        fs::create_dir_all(&root).context("Failed to create sandbox root")?;

        let control_dir = root.join("control");
        let fmm_dir = root.join("fmm");

        Ok(Self {
            root,
            control_dir,
            fmm_dir,
            max_size_mb: 500,
            max_files: 10_000,
            created_at: Instant::now(),
            ttl: Duration::from_secs(3600),
            cleanup_on_drop: true,
        })
    }

    /// Clone a repository into the sandbox
    pub fn clone_repo(&self, url: &str, branch: Option<&str>) -> Result<()> {
        validate_repo_url(url)?;
        // Clone for control variant
        self.clone_to_dir(url, branch, &self.control_dir)?;

        // Clone for FMM variant (or copy)
        self.clone_to_dir(url, branch, &self.fmm_dir)?;

        Ok(())
    }

    fn clone_to_dir(&self, url: &str, branch: Option<&str>, dir: &Path) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--single-branch");

        if let Some(b) = branch {
            cmd.arg("--branch").arg(b);
        }

        cmd.arg(url).arg(dir);

        let output = cmd.output().context("Failed to execute git clone")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git clone failed: {}", stderr);
        }

        Ok(())
    }

    /// Get the current commit SHA from a directory
    pub fn get_commit_sha(&self, dir: &Path) -> Result<String> {
        let output = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(dir)
            .output()
            .context("Failed to get commit SHA")?;

        if !output.status.success() {
            anyhow::bail!("Git rev-parse failed");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Generate FMM manifest for the FMM variant
    pub fn generate_fmm_manifest(&self) -> Result<()> {
        // Run fmm generate in the FMM directory
        let fmm_binary = std::env::current_exe().context("Failed to get current executable")?;

        let output = Command::new(&fmm_binary)
            .arg("generate")
            .arg("--manifest-only")
            .current_dir(&self.fmm_dir)
            .output()
            .context("Failed to run fmm generate")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Don't fail if fmm generate fails (might be unsupported language)
            eprintln!("Warning: fmm generate had issues: {}", stderr);
        }

        Ok(())
    }

    /// Check if sandbox exceeds limits
    #[allow(dead_code)]
    pub fn check_limits(&self) -> Result<()> {
        // Check TTL
        if self.created_at.elapsed() > self.ttl {
            anyhow::bail!("Sandbox expired (TTL exceeded)");
        }

        // Check size (simplified - just check if dirs exist and have reasonable size)
        let size = dir_size(&self.root)?;
        if size > self.max_size_mb * 1_000_000 {
            anyhow::bail!(
                "Sandbox size exceeded: {} MB > {} MB",
                size / 1_000_000,
                self.max_size_mb
            );
        }

        Ok(())
    }

    /// Count files in the FMM variant
    #[allow(dead_code)]
    pub fn count_files(&self) -> Result<u64> {
        count_files_in_dir(&self.fmm_dir)
    }

    /// Disable cleanup on drop (for debugging)
    #[allow(dead_code)]
    pub fn keep_on_drop(&mut self) {
        self.cleanup_on_drop = false;
    }

    /// Manually cleanup the sandbox
    pub fn cleanup(&self) {
        if let Err(e) = fs::remove_dir_all(&self.root) {
            eprintln!("Warning: Failed to cleanup sandbox: {}", e);
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            self.cleanup();
        }
    }
}

#[allow(dead_code)]
fn dir_size(path: &Path) -> Result<u64> {
    let mut size = 0u64;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                size += dir_size(&entry.path())?;
            } else {
                size += metadata.len();
            }
        }
    }

    Ok(size)
}

#[allow(dead_code)]
fn count_files_in_dir(path: &Path) -> Result<u64> {
    let mut count = 0u64;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                count += count_files_in_dir(&entry.path())?;
            } else {
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Validate job_id contains only safe path characters
fn validate_job_id(job_id: &str) -> Result<()> {
    if job_id.is_empty() {
        anyhow::bail!("Job ID must not be empty");
    }
    if !job_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "Invalid job ID '{}': only alphanumeric, hyphens, and underscores allowed",
            job_id
        );
    }
    Ok(())
}

/// Validate repository URL is a safe HTTPS git URL
fn validate_repo_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        anyhow::bail!("Repository URL must use HTTPS: {}", url);
    }
    // Ensure it looks like a valid git host URL (github, gitlab, bitbucket, etc.)
    let host = url
        .strip_prefix("https://")
        .and_then(|s| s.split('/').next())
        .unwrap_or("");
    if host.is_empty() || !host.contains('.') {
        anyhow::bail!("Invalid repository host in URL: {}", url);
    }
    // Reject URLs with suspicious characters that could be used for injection
    if url.contains("..") || url.contains('\0') || url.contains(';') || url.contains('|') {
        anyhow::bail!("Repository URL contains invalid characters: {}", url);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let sandbox = Sandbox::new("test-123").unwrap();
        assert!(sandbox.root.exists());

        // Cleanup
        sandbox.cleanup();
        assert!(!sandbox.root.exists());
    }

    #[test]
    fn test_sandbox_rejects_traversal_job_id() {
        assert!(Sandbox::new("../escape").is_err());
        assert!(Sandbox::new("foo/../bar").is_err());
        assert!(Sandbox::new("").is_err());
    }

    #[test]
    fn test_sandbox_accepts_valid_job_id() {
        let sandbox = Sandbox::new("cmp-abc123-0f3a").unwrap();
        assert!(sandbox.root.exists());
        sandbox.cleanup();
    }

    #[test]
    fn test_validate_repo_url_https_required() {
        assert!(validate_repo_url("http://github.com/foo/bar").is_err());
        assert!(validate_repo_url("git@github.com:foo/bar.git").is_err());
        assert!(validate_repo_url("ftp://github.com/foo/bar").is_err());
    }

    #[test]
    fn test_validate_repo_url_valid() {
        assert!(validate_repo_url("https://github.com/pmndrs/zustand").is_ok());
        assert!(validate_repo_url("https://gitlab.com/user/project").is_ok());
        assert!(validate_repo_url("https://bitbucket.org/team/repo").is_ok());
    }

    #[test]
    fn test_validate_repo_url_injection() {
        assert!(validate_repo_url("https://github.com/foo;rm -rf /").is_err());
        assert!(validate_repo_url("https://github.com/foo|cat /etc/passwd").is_err());
        assert!(validate_repo_url("https://github.com/../../../etc").is_err());
    }

    #[test]
    fn test_validate_repo_url_invalid_host() {
        assert!(validate_repo_url("https:///no-host").is_err());
        assert!(validate_repo_url("https://noperiod/repo").is_err());
    }

    #[test]
    fn test_validate_job_id_valid() {
        assert!(validate_job_id("cmp-abc-123").is_ok());
        assert!(validate_job_id("simple").is_ok());
        assert!(validate_job_id("with_underscore").is_ok());
    }

    #[test]
    fn test_validate_job_id_invalid() {
        assert!(validate_job_id("").is_err());
        assert!(validate_job_id("../escape").is_err());
        assert!(validate_job_id("has space").is_err());
        assert!(validate_job_id("has;semicolon").is_err());
    }

    #[test]
    fn test_sandbox_auto_cleanup_on_drop() {
        let root_path;
        {
            let sandbox = Sandbox::new("drop-test-001").unwrap();
            root_path = sandbox.root.clone();
            assert!(root_path.exists());
            // sandbox drops here
        }
        assert!(!root_path.exists());
    }

    #[test]
    fn test_sandbox_keep_on_drop() {
        let root_path;
        {
            let mut sandbox = Sandbox::new("keep-test-001").unwrap();
            sandbox.keep_on_drop();
            root_path = sandbox.root.clone();
            // sandbox drops here but should NOT cleanup
        }
        assert!(root_path.exists());
        // Manual cleanup
        let _ = fs::remove_dir_all(&root_path);
    }
}
