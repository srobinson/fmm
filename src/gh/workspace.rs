use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::GlobalConfig;

pub fn resolve_workspace(
    global_config: &GlobalConfig,
    cli_override: Option<&str>,
) -> Result<PathBuf> {
    let workspace = match cli_override {
        Some(path) => PathBuf::from(path),
        None => global_config.workspace_dir.clone(),
    };

    std::fs::create_dir_all(&workspace)
        .with_context(|| format!("Failed to create workspace dir: {}", workspace.display()))?;

    Ok(workspace)
}

pub fn clone_or_update(
    clone_url: &str,
    workspace_root: &Path,
    owner: &str,
    repo: &str,
) -> Result<PathBuf> {
    let repo_dir = workspace_root.join(format!("{}-{}", owner, repo));

    if repo_dir.join(".git").exists() {
        // Update existing clone
        let output = Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(&repo_dir)
            .output()
            .context("Failed to git fetch")?;

        if !output.status.success() {
            anyhow::bail!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Determine default branch
        let default_branch = detect_default_branch(&repo_dir)?;

        let output = Command::new("git")
            .args(["checkout", &default_branch])
            .current_dir(&repo_dir)
            .output()
            .context("Failed to git checkout")?;

        if !output.status.success() {
            anyhow::bail!(
                "git checkout {} failed: {}",
                default_branch,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let output = Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(&repo_dir)
            .output()
            .context("Failed to git pull")?;

        if !output.status.success() {
            anyhow::bail!(
                "git pull failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        // Fresh clone (full history for branching)
        let output = Command::new("git")
            .args(["clone", clone_url, &repo_dir.to_string_lossy()])
            .output()
            .context("Failed to git clone")?;

        if !output.status.success() {
            anyhow::bail!(
                "git clone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(repo_dir)
}

fn detect_default_branch(repo_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to detect default branch")?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Strip "origin/" prefix
        Ok(branch
            .strip_prefix("origin/")
            .unwrap_or(&branch)
            .to_string())
    } else {
        // Fallback: try main, then master
        for candidate in ["main", "master"] {
            let output = Command::new("git")
                .args(["rev-parse", "--verify", &format!("origin/{}", candidate)])
                .current_dir(repo_dir)
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    return Ok(candidate.to_string());
                }
            }
        }
        Ok("main".to_string())
    }
}

pub fn generate_sidecars(repo_dir: &Path) -> Result<()> {
    let fmm_binary = std::env::current_exe().context("Failed to get current executable")?;

    let output = Command::new(&fmm_binary)
        .args(["generate", "."])
        .current_dir(repo_dir)
        .output()
        .context("Failed to run fmm generate")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("fmm generate failed: {}", stderr);
    }

    Ok(())
}

pub fn create_branch(repo_dir: &Path, prefix: &str, issue_number: u64) -> Result<String> {
    let branch_name = format!("{}/fix-issue-{}", prefix, issue_number);

    let output = Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .current_dir(repo_dir)
        .output()
        .context("Failed to create branch")?;

    if !output.status.success() {
        anyhow::bail!(
            "git checkout -b {} failed: {}",
            branch_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(branch_name)
}
