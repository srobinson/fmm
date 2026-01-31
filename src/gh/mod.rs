mod github;
mod prompt;
mod references;
mod runner;
mod workspace;

pub use github::{create_pr, fetch_issue, preflight_check, Issue, IssueRef};
pub use prompt::{build_prompt, format_dry_run};
pub use references::{extract_references, resolve_references, CodeReference, ResolvedReference};
pub use runner::{invoke_claude, RunResult};
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
        result.turns,
        result.cost_usd,
    );

    if !result.success {
        anyhow::bail!("Claude failed to fix the issue: {}", result.response_text);
    }

    // 14. Verify changes
    let has_changes = verify_changes(&repo_dir)?;
    if !has_changes {
        println!(
            "{}",
            "No changes were made. Claude may not have found a fix.".yellow()
        );
        return Ok(());
    }

    // 15. Commit
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

    println!("  {} ${:.4}", "Cost:".bold(), result.cost_usd);

    Ok(())
}

fn verify_changes(repo_dir: &std::path::Path) -> Result<bool> {
    let output = std::process::Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to run git diff")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
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
