pub mod batch;
mod github;
mod prompt;
mod references;
pub mod report;
mod runner;
mod workspace;

pub use github::{create_pr, fetch_issue, preflight_check, Issue, IssueRef};
pub use prompt::{build_prompt, format_dry_run};
pub use references::{extract_references, resolve_references, CodeReference, ResolvedReference};
pub use runner::{invoke_claude, invoke_claude_with_options, InvokeOptions, RunResult};
pub use workspace::{clone_or_update, create_branch, generate_sidecars, resolve_workspace};

use anyhow::{Context, Result};
use colored::Colorize;

use crate::compare::sandbox::Sandbox;
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
    pub compare: bool,
    pub output: Option<String>,
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

    if options.compare {
        return gh_issue_compare(url, &issue, &issue_ref, &options);
    }

    gh_issue_fix(&issue, &issue_ref, &options)
}

/// Standard flow: fix the issue and create a PR.
fn gh_issue_fix(issue: &Issue, issue_ref: &IssueRef, options: &GhIssueOptions) -> Result<()> {
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
    let prompt = build_prompt(issue, issue_ref, &resolved, &unresolved);

    // 11. Dry run?
    if options.dry_run {
        println!(
            "\n{}",
            format_dry_run(issue, &resolved, &unresolved, &prompt)
        );
        return Ok(());
    }

    // 12. Create branch
    let branch = create_branch(&repo_dir, &options.branch_prefix, issue_ref.number)?;
    println!("  {} {}", "Branch:".bold(), branch);

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

    // 14. Verify changes
    let has_changes = verify_changes(&repo_dir, &pre_claude_head)?;
    if !has_changes {
        println!(
            "{}",
            "No changes were made. Claude may not have found a fix.".yellow()
        );
        return Ok(());
    }

    // 15. Commit
    commit_changes(&repo_dir, issue)?;

    // 16. Push
    println!("{}", "Pushing branch...".green().bold());
    push_branch(&repo_dir, &branch)?;

    // 17. Create PR (unless --no-pr)
    if !options.no_pr {
        println!("{}", "Creating PR...".green().bold());
        let pr_url = create_pr(&repo_dir, issue, &branch)?;
        println!("\n{}", "Done!".green().bold());
        println!("  {} {}", "PR:".bold(), pr_url);
    } else {
        println!("\n{}", "Done!".green().bold());
        println!("  Branch pushed (--no-pr, skipping PR creation)");
    }

    println!("  {} ${:.4}", "Cost:".bold(), result.metrics.cost_usd);

    Ok(())
}

/// Compare flow: run control (no sidecars) vs fmm (with sidecars) in isolated sandboxes.
fn gh_issue_compare(
    url: &str,
    issue: &Issue,
    issue_ref: &IssueRef,
    options: &GhIssueOptions,
) -> Result<()> {
    let report = run_issue_compare(
        url,
        issue,
        issue_ref,
        &options.model,
        options.max_turns,
        options.max_budget,
    )?;

    report.print_summary();

    // Save report files
    if let Some(ref output_dir) = options.output {
        let saved = report.save(std::path::Path::new(output_dir))?;
        println!();
        for path in &saved {
            println!("  {} {}", "Saved:".bold(), path.dimmed());
        }
    } else {
        // Default: print JSON to allow piping
        println!("\n{}", "--- JSON Report ---".dimmed());
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    let total_cost = report.control.cost_usd + report.fmm.cost_usd;
    println!("\n  {} ${:.4}", "Total cost:".bold(), total_cost);

    Ok(())
}

/// Run a single issue A/B comparison and return the report.
/// This is the reusable core used by both `fmm gh issue --compare` and `fmm gh batch`.
pub fn run_issue_compare(
    url: &str,
    issue: &Issue,
    issue_ref: &IssueRef,
    model: &str,
    max_turns: u32,
    max_budget: f64,
) -> Result<report::IssueComparisonReport> {
    let repo_slug = format!("{}/{}", issue_ref.owner, issue_ref.repo);

    println!(
        "\n{} A/B comparison for {}#{}",
        "COMPARE MODE".cyan().bold(),
        repo_slug,
        issue_ref.number
    );

    // --- Sandbox setup ---
    println!("{}", "Setting up sandboxes...".green().bold());
    let job_id = generate_compare_job_id(issue_ref);
    let sandbox = Sandbox::new(&job_id)?;
    sandbox.clone_repo(&issue_ref.clone_url, None)?;
    println!("  {} Cloned into dual sandboxes", "OK".green());

    // --- Build prompts ---
    let control_prompt = build_raw_issue_prompt(issue, issue_ref);

    println!("{}", "Generating FMM sidecars...".green().bold());
    sandbox.generate_fmm_manifest()?;
    sandbox.setup_fmm_integration()?;

    let manifest = Manifest::load_from_sidecars(&sandbox.fmm_dir)?;
    println!("  {} {} files indexed", "OK".green(), manifest.files.len());

    let refs = extract_references(&issue.body);
    let (resolved, unresolved) = resolve_references(&refs, &manifest);
    let fmm_prompt = build_prompt(issue, issue_ref, &resolved, &unresolved);
    let fmm_context = build_fmm_context(&sandbox.fmm_dir);

    // --- Run control variant FIRST (cold cache, no sidecars) ---
    println!(
        "\n{} Running {} variant...",
        "1/2".cyan().bold(),
        "control".white().bold()
    );
    println!("  {} No sidecars, no skill, no MCP", "Config:".dimmed());

    let control_result = invoke_claude_with_options(InvokeOptions {
        prompt: &control_prompt,
        repo_dir: &sandbox.control_dir,
        model,
        max_turns,
        max_budget,
        allowed_tools: Some("Read,Write,Edit,Glob,Grep,Bash"),
        setting_sources: Some(""),
        append_system_prompt: None,
    })?;

    println!(
        "  {} {}, {} turns, {} tool calls, ${:.4}",
        if control_result.success {
            "OK".green()
        } else {
            "FAIL".red()
        },
        if control_result.success {
            "success"
        } else {
            "failed"
        },
        control_result.metrics.turns,
        control_result.metrics.tool_calls,
        control_result.metrics.cost_usd,
    );

    // --- Run FMM variant SECOND (sidecars + skill + MCP) ---
    println!(
        "\n{} Running {} variant...",
        "2/2".cyan().bold(),
        "fmm".green().bold()
    );
    println!("  {} Sidecars + skill + MCP + context", "Config:".dimmed());

    let fmm_result = invoke_claude_with_options(InvokeOptions {
        prompt: &fmm_prompt,
        repo_dir: &sandbox.fmm_dir,
        model,
        max_turns,
        max_budget,
        allowed_tools: Some("Read,Write,Edit,Glob,Grep,Bash"),
        setting_sources: Some("local"),
        append_system_prompt: Some(&fmm_context),
    })?;

    println!(
        "  {} {}, {} turns, {} tool calls, ${:.4}",
        if fmm_result.success {
            "OK".green()
        } else {
            "FAIL".red()
        },
        if fmm_result.success {
            "success"
        } else {
            "failed"
        },
        fmm_result.metrics.turns,
        fmm_result.metrics.tool_calls,
        fmm_result.metrics.cost_usd,
    );

    // --- Generate report ---
    let report = report::IssueComparisonReport::new(report::ReportInput {
        issue_url: url,
        issue_title: &issue.title,
        issue_number: issue_ref.number,
        repo: &repo_slug,
        model,
        max_budget_usd: max_budget,
        max_turns,
        control_metrics: &control_result.metrics,
        fmm_metrics: &fmm_result.metrics,
    });

    Ok(report)
}

/// Build a raw issue prompt without sidecar references (for control variant).
fn build_raw_issue_prompt(issue: &Issue, issue_ref: &IssueRef) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "## Task\nFix GitHub issue #{} in {}/{}: {}\n\n",
        issue.number, issue_ref.owner, issue_ref.repo, issue.title
    ));

    prompt.push_str(&format!("## Issue Description\n{}\n\n", issue.body));

    prompt.push_str("## Instructions\n");
    prompt.push_str("1. Explore the codebase to find the relevant files\n");
    prompt.push_str("2. Make minimal changes to fix the issue\n");
    prompt.push_str("3. Stay consistent with existing code style\n");
    prompt.push_str("4. Do NOT modify unrelated files\n");
    prompt.push_str(
        "5. Run tests if available (look for package.json scripts, Makefile, Cargo.toml, etc.)\n",
    );

    prompt
}

/// Build FMM context string for system prompt injection.
fn build_fmm_context(fmm_dir: &std::path::Path) -> String {
    let has_sidecars = walkdir::WalkDir::new(fmm_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("fmm"));

    if !has_sidecars {
        return String::new();
    }

    r#"This repository has .fmm sidecar files — structured metadata companions for source files.

For every source file (e.g. foo.ts), there may be a foo.ts.fmm containing:
- exports: what the file defines
- imports: external packages used
- dependencies: local files it imports
- loc: file size

Use sidecars to navigate: Grep "exports:.*SymbolName" **/*.fmm to find files.
Only open source files you need to edit."#
        .to_string()
}

fn generate_compare_job_id(issue_ref: &IssueRef) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let ts = duration.as_secs();
    let ns = duration.subsec_nanos();
    let random: u16 = ((ns / 1000) % 65536) as u16;

    // Sanitize repo name: only keep alphanumeric, hyphens, underscores
    let safe_repo: String = issue_ref
        .repo
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    format!(
        "gh-cmp-{}-{}-{:x}-{:04x}",
        safe_repo, issue_ref.number, ts, random
    )
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
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to run git status")?;

    if !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        return Ok(true);
    }

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

    let status = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to check staged changes")?;

    if status.status.success() {
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
