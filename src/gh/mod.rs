mod github;
mod prompt;
mod references;
mod runner;
mod workspace;

pub use github::{create_pr, fetch_issue, preflight_check, Issue, IssueRef};
pub use prompt::{build_prompt, format_dry_run};
pub use references::{extract_references, resolve_references, CodeReference, ResolvedReference};
pub use runner::{invoke_claude, invoke_claude_with_options, InvokeOptions, RunResult};
pub use workspace::{clone_or_update, create_branch, generate_sidecars, resolve_workspace};

use anyhow::{Context, Result};
use colored::Colorize;

use crate::config::GlobalConfig;
use crate::manifest::Manifest;

pub struct GhIssueOptions {
    pub model: String,
    pub max_turns: u32,
    pub max_budget: f64,
    pub dry_run: bool,
    pub branch_prefix: String,
    pub no_pr: bool,
    pub workspace: Option<String>,
}

pub fn gh_issue(url: &str, options: GhIssueOptions) -> Result<()> {
    // 1. Preflight
    println!("{}", "Checking prerequisites...".green().bold());
    preflight_check()?;

    // 2. Parse URL
    let issue_ref = IssueRef::parse(url)?;
    println!(
        "  {} {}/{} #{}",
        "Issue:".bold(),
        issue_ref.owner,
        issue_ref.repo,
        issue_ref.number
    );

    // 3. Fetch issue
    println!("{}", "Fetching issue...".green().bold());
    let issue = fetch_issue(url)?;
    println!("  {} {}", "Title:".bold(), issue.title);

    // 4. Resolve workspace
    let global_config = GlobalConfig::load();
    let workspace_root = resolve_workspace(&global_config, options.workspace.as_deref())?;

    // 5. Clone or update
    println!("{}", "Preparing workspace...".green().bold());
    let repo_dir = clone_or_update(
        &issue_ref.clone_url,
        &workspace_root,
        &issue_ref.owner,
        &issue_ref.repo,
    )?;

    // 6. Generate sidecars
    println!("{}", "Generating sidecars...".green().bold());
    generate_sidecars(&repo_dir)?;

    // 7. Load manifest
    let manifest = Manifest::load_from_sidecars(&repo_dir)?;
    println!(
        "  {} {} files indexed",
        "Index:".bold(),
        manifest.files.len()
    );

    // 8. Extract references
    let refs = extract_references(&issue.body);
    println!("  {} {} references extracted", "Refs:".bold(), refs.len());

    // 9. Resolve references
    let (resolved, unresolved) = resolve_references(&refs, &manifest);
    println!(
        "  {} {} resolved, {} unresolved",
        "Match:".bold(),
        resolved.len(),
        unresolved.len()
    );

    // 10. Build prompt
    let prompt = build_prompt(&issue, &issue_ref, &resolved, &unresolved);

    // 11. Dry run?
    if options.dry_run {
        println!(
            "\n{}",
            format_dry_run(&issue, &resolved, &unresolved, &prompt)
        );
        return Ok(());
    }

    // 12. Create branch
    let branch = create_branch(&repo_dir, &options.branch_prefix, issue_ref.number)?;
    println!("  {} {}", "Branch:".bold(), branch);

    // Record HEAD before Claude runs so we can detect if it made commits
    let pre_claude_head = get_head_sha(&repo_dir)?;

    // 13. Invoke Claude
    println!("{}", "Running Claude...".green().bold());
    let result = invoke_claude(
        &prompt,
        &repo_dir,
        &options.model,
        options.max_turns,
        options.max_budget,
    )?;
    println!(
        "  {} {}, {} turns, ${:.4}",
        "Result:".bold(),
        if result.success {
            "success".green()
        } else {
            "failed".red()
        },
        result.metrics.turns,
        result.metrics.cost_usd,
    );

    if !result.success {
        anyhow::bail!("Claude failed to fix the issue: {}", result.response_text);
    }

    // 14. Verify changes (working tree or new commits)
    let has_changes = verify_changes(&repo_dir, &pre_claude_head)?;
    if !has_changes {
        println!(
            "{}",
            "No changes were made. Claude may not have found a fix.".yellow()
        );
        return Ok(());
    }

    // 15. Commit (skip if Claude already committed and working tree is clean)
    commit_changes(&repo_dir, &issue)?;

    // 16. Push
    println!("{}", "Pushing branch...".green().bold());
    push_branch(&repo_dir, &branch)?;

    // 17. Create PR (unless --no-pr)
    if !options.no_pr {
        println!("{}", "Creating PR...".green().bold());
        let pr_url = create_pr(&repo_dir, &issue, &branch)?;
        println!("\n{}", "Done!".green().bold());
        println!("  {} {}", "PR:".bold(), pr_url);
    } else {
        println!("\n{}", "Done!".green().bold());
        println!("  Branch pushed (--no-pr, skipping PR creation)");
    }

    println!("  {} ${:.4}", "Cost:".bold(), result.metrics.cost_usd);

    Ok(())
}

fn get_head_sha(repo_dir: &std::path::Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to get HEAD sha")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn verify_changes(repo_dir: &std::path::Path, pre_claude_head: &str) -> Result<bool> {
    // Check for working tree changes (unstaged, staged, or untracked)
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to run git status")?;

    if !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        return Ok(true);
    }

    // Check if Claude made its own commits by comparing current HEAD to pre-invoke HEAD
    let current_head = get_head_sha(repo_dir)?;
    if current_head != pre_claude_head {
        return Ok(true);
    }

    Ok(false)
}

fn commit_changes(repo_dir: &std::path::Path, issue: &Issue) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to git add")?;

    if !output.status.success() {
        anyhow::bail!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Check if there's anything to commit (Claude may have committed already)
    let status = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to check staged changes")?;

    if status.status.success() {
        // Nothing staged — Claude already committed its changes
        return Ok(());
    }

    let commit_msg = format!("fix: {} (#{})", issue.title, issue.number);
    let output = std::process::Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(repo_dir)
        .output()
        .context("Failed to git commit")?;

    if !output.status.success() {
        anyhow::bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn push_branch(repo_dir: &std::path::Path, branch: &str) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["push", "-u", "origin", branch])
        .current_dir(repo_dir)
        .output()
        .context("Failed to push branch")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "git push failed (branch '{}' exists locally for manual recovery): {}",
            branch,
            stderr
        );
    }

    Ok(())
}
