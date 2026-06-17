use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use fmm_core::store::GitMeta;

pub(crate) fn probe(
    root: &Path,
    sha_override: Option<&str>,
    no_git: bool,
) -> Result<Option<GitMeta>> {
    if no_git || !is_git_work_tree(root)? {
        return Ok(None);
    }

    let sha = match sha_override {
        Some(sha) => sha.to_owned(),
        None => {
            let Some(sha) = optional_git_stdout(root, &["rev-parse", "HEAD"])? else {
                return Ok(None);
            };
            sha
        }
    };
    let branch = required_git_stdout(
        root,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        "resolve git branch",
    )?;
    let dirty =
        !required_git_stdout(root, &["status", "--porcelain"], "read git status")?.is_empty();

    Ok(Some(GitMeta {
        sha,
        branch: normalize_branch(&branch),
        dirty,
    }))
}

fn is_git_work_tree(root: &Path) -> Result<bool> {
    let Some(output) = optional_git_stdout(root, &["rev-parse", "--is-inside-work-tree"])? else {
        return Ok(false);
    };
    Ok(output == "true")
}

fn normalize_branch(branch: &str) -> Option<String> {
    let branch = branch.trim();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch.to_owned())
    }
}

fn required_git_stdout(root: &Path, args: &[&str], context: &str) -> Result<String> {
    optional_git_stdout(root, args)?
        .with_context(|| format!("Failed to {context} in {}", root.display()))
}

fn optional_git_stdout(root: &Path, args: &[&str]) -> Result<Option<String>> {
    let output = Command::new("git").arg("-C").arg(root).args(args).output();
    let output = match output {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("Failed to run git"),
    };
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}
