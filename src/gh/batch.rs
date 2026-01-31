//! Batch orchestrator for running N issue comparisons with checkpoint/resume.
//!
//! Reads a corpus file (`issues.json`), runs `run_issue_compare()` for each issue,
//! writes JSONL checkpoint for resume, and aggregates into proof-dataset.json/md.

use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use super::report::{IssueComparisonReport, Savings};
use super::{fetch_issue, preflight_check, run_issue_compare, IssueRef};

// ── Corpus schema (issues.json) ──

/// A single issue entry in the corpus file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusIssue {
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// The full corpus file (issues.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corpus {
    pub issues: Vec<CorpusIssue>,
}

impl Corpus {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read corpus file: {}", path.display()))?;
        let corpus: Corpus = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse corpus file: {}", path.display()))?;
        if corpus.issues.is_empty() {
            anyhow::bail!("Corpus file is empty — no issues to process");
        }
        Ok(corpus)
    }
}

// ── Batch options ──

pub struct BatchOptions {
    pub corpus_path: PathBuf,
    pub output_dir: PathBuf,
    pub model: String,
    pub max_turns: u32,
    pub max_budget: f64,
    pub dry_run: bool,
    pub resume: bool,
}

// ── Checkpoint (JSONL) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointEntry {
    url: String,
    status: CheckpointStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<IssueComparisonReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CheckpointStatus {
    Done,
    Failed,
}

fn checkpoint_path(output_dir: &Path) -> PathBuf {
    output_dir.join("checkpoint.jsonl")
}

fn load_checkpoint(output_dir: &Path) -> HashMap<String, CheckpointEntry> {
    let path = checkpoint_path(output_dir);
    let mut map = HashMap::new();
    if let Ok(file) = fs::File::open(&path) {
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            if let Ok(entry) = serde_json::from_str::<CheckpointEntry>(&line) {
                map.insert(entry.url.clone(), entry);
            }
        }
    }
    map
}

fn append_checkpoint(output_dir: &Path, entry: &CheckpointEntry) -> Result<()> {
    let path = checkpoint_path(output_dir);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open checkpoint: {}", path.display()))?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

// ── Aggregated dataset ──

/// Aggregated proof dataset — the final output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofDataset {
    pub generated_at: String,
    pub corpus_size: usize,
    pub completed: usize,
    pub failed: usize,
    pub aggregate: AggregateStats,
    pub by_tag: HashMap<String, AggregateStats>,
    pub issues: Vec<IssueComparisonReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    pub count: usize,
    pub mean_input_tokens_pct: f64,
    pub mean_total_tokens_pct: f64,
    pub mean_cost_pct: f64,
    pub mean_turns_pct: f64,
    pub mean_tool_calls_pct: f64,
    pub mean_files_read_pct: f64,
    pub mean_duration_pct: f64,
    pub median_cost_pct: f64,
    pub median_total_tokens_pct: f64,
    pub total_control_cost_usd: f64,
    pub total_fmm_cost_usd: f64,
}

fn aggregate_savings(reports: &[&IssueComparisonReport]) -> AggregateStats {
    let n = reports.len();
    if n == 0 {
        return AggregateStats {
            count: 0,
            mean_input_tokens_pct: 0.0,
            mean_total_tokens_pct: 0.0,
            mean_cost_pct: 0.0,
            mean_turns_pct: 0.0,
            mean_tool_calls_pct: 0.0,
            mean_files_read_pct: 0.0,
            mean_duration_pct: 0.0,
            median_cost_pct: 0.0,
            median_total_tokens_pct: 0.0,
            total_control_cost_usd: 0.0,
            total_fmm_cost_usd: 0.0,
        };
    }

    let sum = |f: fn(&Savings) -> f64| -> f64 {
        reports.iter().map(|r| f(&r.savings)).sum::<f64>() / n as f64
    };

    let mut cost_pcts: Vec<f64> = reports.iter().map(|r| r.savings.cost_pct).collect();
    cost_pcts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut token_pcts: Vec<f64> = reports.iter().map(|r| r.savings.total_tokens_pct).collect();
    token_pcts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    AggregateStats {
        count: n,
        mean_input_tokens_pct: sum(|s| s.input_tokens_pct),
        mean_total_tokens_pct: sum(|s| s.total_tokens_pct),
        mean_cost_pct: sum(|s| s.cost_pct),
        mean_turns_pct: sum(|s| s.turns_pct),
        mean_tool_calls_pct: sum(|s| s.tool_calls_pct),
        mean_files_read_pct: sum(|s| s.files_read_pct),
        mean_duration_pct: sum(|s| s.duration_pct),
        median_cost_pct: median(&cost_pcts),
        median_total_tokens_pct: median(&token_pcts),
        total_control_cost_usd: reports.iter().map(|r| r.control.cost_usd).sum(),
        total_fmm_cost_usd: reports.iter().map(|r| r.fmm.cost_usd).sum(),
    }
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    }
}

// ── Proof dataset markdown ──

fn generate_proof_markdown(dataset: &ProofDataset, corpus: &Corpus) -> String {
    let mut md = String::new();

    md.push_str("# FMM Proof Dataset\n\n");
    md.push_str(&format!(
        "Across **{}** issues in **{}** repos spanning **{}** languages, \
         FMM reduced token usage by **{:.0}%** (median {:.0}%), cost by **{:.0}%** (median {:.0}%).\n\n",
        dataset.completed,
        count_unique_repos(&dataset.issues),
        count_unique_languages(corpus),
        dataset.aggregate.mean_total_tokens_pct,
        dataset.aggregate.median_total_tokens_pct,
        dataset.aggregate.mean_cost_pct,
        dataset.aggregate.median_cost_pct,
    ));

    md.push_str(&format!("**Generated:** {}\n\n", dataset.generated_at));

    // Summary table
    md.push_str("## Aggregate Results\n\n");
    md.push_str("| Metric | Mean Reduction | Median Reduction |\n");
    md.push_str("|--------|---------------|------------------|\n");
    md.push_str(&format!(
        "| Total tokens | {:.0}% | {:.0}% |\n",
        dataset.aggregate.mean_total_tokens_pct, dataset.aggregate.median_total_tokens_pct
    ));
    md.push_str(&format!(
        "| Input tokens | {:.0}% | — |\n",
        dataset.aggregate.mean_input_tokens_pct
    ));
    md.push_str(&format!(
        "| Cost | {:.0}% | {:.0}% |\n",
        dataset.aggregate.mean_cost_pct, dataset.aggregate.median_cost_pct
    ));
    md.push_str(&format!(
        "| Turns | {:.0}% | — |\n",
        dataset.aggregate.mean_turns_pct
    ));
    md.push_str(&format!(
        "| Tool calls | {:.0}% | — |\n",
        dataset.aggregate.mean_tool_calls_pct
    ));
    md.push_str(&format!(
        "| Files read | {:.0}% | — |\n",
        dataset.aggregate.mean_files_read_pct
    ));
    md.push_str(&format!(
        "| Duration | {:.0}% | — |\n\n",
        dataset.aggregate.mean_duration_pct
    ));

    md.push_str(&format!(
        "**Total cost:** Control ${:.2} + FMM ${:.2} = ${:.2}\n\n",
        dataset.aggregate.total_control_cost_usd,
        dataset.aggregate.total_fmm_cost_usd,
        dataset.aggregate.total_control_cost_usd + dataset.aggregate.total_fmm_cost_usd,
    ));

    // By-tag breakdown
    if !dataset.by_tag.is_empty() {
        md.push_str("## By Tag\n\n");
        md.push_str("| Tag | N | Mean Token Reduction | Mean Cost Reduction |\n");
        md.push_str("|-----|---|---------------------|--------------------|\n");
        let mut tags: Vec<_> = dataset.by_tag.iter().collect();
        tags.sort_by_key(|(k, _)| (*k).clone());
        for (tag, stats) in &tags {
            md.push_str(&format!(
                "| {} | {} | {:.0}% | {:.0}% |\n",
                tag, stats.count, stats.mean_total_tokens_pct, stats.mean_cost_pct
            ));
        }
        md.push('\n');
    }

    // Per-issue table
    md.push_str("## Per-Issue Results\n\n");
    md.push_str("| # | Repo | Issue | Tokens Saved | Cost Saved | Turns Saved |\n");
    md.push_str("|---|------|-------|-------------|------------|-------------|\n");
    for (i, report) in dataset.issues.iter().enumerate() {
        md.push_str(&format!(
            "| {} | {} | [#{}]({}) | {:.0}% | {:.0}% | {:.0}% |\n",
            i + 1,
            report.repo,
            report.issue_number,
            report.issue_url,
            report.savings.total_tokens_pct,
            report.savings.cost_pct,
            report.savings.turns_pct,
        ));
    }
    md.push('\n');

    if dataset.failed > 0 {
        md.push_str(&format!(
            "**Note:** {} issue(s) failed and are excluded from aggregates.\n",
            dataset.failed
        ));
    }

    md
}

fn count_unique_repos(reports: &[IssueComparisonReport]) -> usize {
    let repos: std::collections::HashSet<&str> = reports.iter().map(|r| r.repo.as_str()).collect();
    repos.len()
}

fn count_unique_languages(corpus: &Corpus) -> usize {
    let langs: std::collections::HashSet<&str> = corpus
        .issues
        .iter()
        .flat_map(|i| i.tags.iter())
        .filter(|t| LANGUAGES.contains(&t.as_str()))
        .map(|t| t.as_str())
        .collect();
    langs.len()
}

// ── Tag categories for health report ──

const LANGUAGES: &[&str] = &[
    "typescript",
    "python",
    "rust",
    "go",
    "java",
    "cpp",
    "csharp",
    "ruby",
];
const SIZES: &[&str] = &["small", "medium", "large", "massive"];
const TASK_TYPES: &[&str] = &["bugfix", "feature", "refactor", "perf"];

const MIN_ISSUES_PER_CATEGORY: usize = 4;

/// Result of validating a single corpus URL via `gh`.
#[derive(Debug)]
pub struct ValidationResult {
    pub url: String,
    pub short_ref: String,
    pub valid: bool,
    pub title: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Validate a single issue URL via the `gh` CLI.
fn validate_url(url: &str) -> ValidationResult {
    let short_ref = url
        .strip_prefix("https://github.com/")
        .unwrap_or(url)
        .replacen("/issues/", "#", 1);

    let output = std::process::Command::new("gh")
        .args(["issue", "view", url, "--json", "state,title"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
            let title = json["title"].as_str().map(String::from);
            let state = json["state"].as_str().map(|s| s.to_lowercase());
            ValidationResult {
                url: url.to_string(),
                short_ref,
                valid: true,
                title,
                state,
                error: None,
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let msg = stderr.trim().lines().next().unwrap_or("not found");
            ValidationResult {
                url: url.to_string(),
                short_ref,
                valid: false,
                title: None,
                state: None,
                error: Some(msg.to_string()),
            }
        }
        Err(e) => ValidationResult {
            url: url.to_string(),
            short_ref,
            valid: false,
            title: None,
            state: None,
            error: Some(format!("failed to run gh: {}", e)),
        },
    }
}

/// Build a tag distribution report grouped by category.
/// Returns (category_label, Vec<(tag, count)>) tuples.
pub fn tag_distribution(corpus: &Corpus) -> Vec<(&'static str, Vec<(&'static str, usize)>)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for issue in &corpus.issues {
        for tag in &issue.tags {
            *counts.entry(tag.as_str()).or_insert(0) += 1;
        }
    }

    let build = |label: &'static str,
                 tags: &[&'static str]|
     -> (&'static str, Vec<(&'static str, usize)>) {
        let items: Vec<(&'static str, usize)> = tags
            .iter()
            .map(|&t| (t, *counts.get(t).unwrap_or(&0)))
            .collect();
        (label, items)
    };

    vec![
        build("Languages", LANGUAGES),
        build("Sizes", SIZES),
        build("Task types", TASK_TYPES),
    ]
}

/// Detect categories with fewer than MIN_ISSUES_PER_CATEGORY issues.
/// Returns list of (tag, count) pairs that are below threshold.
pub fn detect_gaps(distribution: &[(&str, Vec<(&str, usize)>)]) -> Vec<(String, usize)> {
    let mut gaps = Vec::new();
    for (_category, tags) in distribution {
        for &(tag, count) in tags {
            if count > 0 && count < MIN_ISSUES_PER_CATEGORY {
                gaps.push((tag.to_string(), count));
            }
        }
    }
    gaps
}

/// Format a distribution row like: `typescript(5) python(5) rust(5) ...`
fn format_distribution_row(tags: &[(&str, usize)]) -> String {
    tags.iter()
        .filter(|(_, count)| *count > 0)
        .map(|(tag, count)| format!("{}({})", tag, count))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run corpus validation: check all URLs, print health report.
/// Returns 0 on success, 1 if any URL is invalid.
pub fn run_validate(corpus_path: &Path) -> Result<i32> {
    let corpus = Corpus::load(corpus_path)?;
    let total = corpus.issues.len();

    println!("Validating {} issues...", total);

    let mut invalid_count = 0usize;
    for (i, issue) in corpus.issues.iter().enumerate() {
        let result = validate_url(&issue.url);
        let label = format!("  [{}/{}]", i + 1, total);
        if result.valid {
            let state = result.state.as_deref().unwrap_or("unknown");
            println!("{} {} {} ({})", label, "✓".green(), result.short_ref, state,);
        } else {
            invalid_count += 1;
            let err = result.error.as_deref().unwrap_or("not found");
            println!("{} {} {} — {}", label, "✗".red(), result.short_ref, err,);
        }
    }

    // Health report
    println!("\n{}", "Corpus Health:".bold());
    let distribution = tag_distribution(&corpus);
    for (category, tags) in &distribution {
        let row = format_distribution_row(tags);
        println!("  {:14}{}", format!("{}:", category), row);
    }

    // Gap detection
    let gaps = detect_gaps(&distribution);
    if !gaps.is_empty() {
        println!();
        for (tag, count) in &gaps {
            println!(
                "  {} {} has only {} issue(s) (minimum: {})",
                "⚠".yellow(),
                tag,
                count,
                MIN_ISSUES_PER_CATEGORY,
            );
        }
    }

    // Summary
    if invalid_count > 0 {
        println!(
            "\n{} {} invalid URL(s) found — fix before running batch",
            "⚠".yellow().bold(),
            invalid_count,
        );
        Ok(1)
    } else {
        println!("\n{} All {} URLs valid", "✓".green().bold(), total,);
        Ok(0)
    }
}

// ── Main batch orchestrator ──

pub fn run_batch(options: BatchOptions) -> Result<()> {
    preflight_check()?;

    let corpus = Corpus::load(&options.corpus_path)?;

    // Dry run: show plan + cost estimate
    if options.dry_run {
        return print_dry_run(&corpus, &options);
    }

    fs::create_dir_all(&options.output_dir)?;

    // Load checkpoint for resume
    let checkpoint = if options.resume {
        let cp = load_checkpoint(&options.output_dir);
        if !cp.is_empty() {
            println!(
                "{} Resuming — {} issues already completed",
                "RESUME".cyan().bold(),
                cp.len()
            );
        }
        cp
    } else {
        HashMap::new()
    };

    let total = corpus.issues.len();
    let mut reports: Vec<IssueComparisonReport> = Vec::new();
    let mut failed = 0usize;
    let mut total_cost = 0.0f64;

    // Collect reports from checkpoint
    for entry in checkpoint.values() {
        if let Some(ref report) = entry.report {
            reports.push(report.clone());
            total_cost += report.control.cost_usd + report.fmm.cost_usd;
        } else {
            failed += 1;
        }
    }

    // Per-issue budget: divide remaining budget evenly across remaining issues, capped at $5
    let remaining = total - checkpoint.len();
    let remaining_budget = options.max_budget - total_cost;
    let per_issue_budget = if remaining > 0 {
        (remaining_budget / remaining as f64).min(5.0)
    } else {
        5.0
    };

    println!(
        "\n{} Running batch: {} issues, model={}, total_budget=${:.2}, per_issue=${:.2}\n",
        "BATCH".cyan().bold(),
        total,
        options.model,
        options.max_budget,
        per_issue_budget,
    );

    for (i, issue_entry) in corpus.issues.iter().enumerate() {
        let label = format!("[{}/{}]", i + 1, total);

        // Skip if already in checkpoint
        if checkpoint.contains_key(&issue_entry.url) {
            println!(
                "  {} {} — {} (checkpoint)",
                label.dimmed(),
                issue_entry.url,
                "SKIP".yellow()
            );
            continue;
        }

        // Budget guard
        if total_cost >= options.max_budget {
            println!(
                "\n{} Budget exhausted (${:.2} >= ${:.2}), stopping batch",
                "STOP".red().bold(),
                total_cost,
                options.max_budget,
            );
            break;
        }

        println!(
            "\n{} {} {}",
            label.cyan().bold(),
            "Processing".green(),
            issue_entry.url,
        );

        match run_single_issue(
            &issue_entry.url,
            &options.model,
            options.max_turns,
            per_issue_budget,
        ) {
            Ok(report) => {
                let cost = report.control.cost_usd + report.fmm.cost_usd;
                total_cost += cost;

                println!(
                    "  {} tokens: {:.0}%, cost: {:.0}%, ${:.4}",
                    "OK".green().bold(),
                    report.savings.total_tokens_pct,
                    report.savings.cost_pct,
                    cost,
                );

                // Save individual report
                let saved = report.save(&options.output_dir)?;
                for path in &saved {
                    println!("  {} {}", "Saved:".dimmed(), path.dimmed());
                }

                append_checkpoint(
                    &options.output_dir,
                    &CheckpointEntry {
                        url: issue_entry.url.clone(),
                        status: CheckpointStatus::Done,
                        report: Some(report.clone()),
                        error: None,
                    },
                )?;

                reports.push(report);
            }
            Err(e) => {
                failed += 1;
                let err_msg = format!("{:#}", e);
                println!("  {} {}", "FAIL".red().bold(), err_msg);

                append_checkpoint(
                    &options.output_dir,
                    &CheckpointEntry {
                        url: issue_entry.url.clone(),
                        status: CheckpointStatus::Failed,
                        report: None,
                        error: Some(err_msg),
                    },
                )?;
            }
        }
    }

    // Aggregate and write proof dataset
    let report_refs: Vec<&IssueComparisonReport> = reports.iter().collect();
    let aggregate = aggregate_savings(&report_refs);

    // Build by-tag breakdown
    let mut by_tag: HashMap<String, Vec<&IssueComparisonReport>> = HashMap::new();
    for issue_entry in &corpus.issues {
        if let Some(report) = reports.iter().find(|r| r.issue_url == issue_entry.url) {
            for tag in &issue_entry.tags {
                by_tag.entry(tag.clone()).or_default().push(report);
            }
        }
    }
    let by_tag_stats: HashMap<String, AggregateStats> = by_tag
        .iter()
        .map(|(tag, reps)| (tag.clone(), aggregate_savings(reps)))
        .collect();

    let dataset = ProofDataset {
        generated_at: chrono::Utc::now().to_rfc3339(),
        corpus_size: total,
        completed: reports.len(),
        failed,
        aggregate,
        by_tag: by_tag_stats,
        issues: reports,
    };

    // Write proof-dataset.json
    let json_path = options.output_dir.join("proof-dataset.json");
    let json = serde_json::to_string_pretty(&dataset)?;
    fs::write(&json_path, &json)?;
    println!("\n  {} {}", "Saved:".bold(), json_path.display());

    // Write proof-dataset.md
    let md_path = options.output_dir.join("proof-dataset.md");
    let markdown = generate_proof_markdown(&dataset, &corpus);
    fs::write(&md_path, &markdown)?;
    println!("  {} {}", "Saved:".bold(), md_path.display());

    // Final summary
    println!("\n{}", "=".repeat(64).dimmed());
    println!("{}", "Batch Complete".green().bold());
    println!("{}", "=".repeat(64).dimmed());
    println!(
        "  {} {}/{} completed, {} failed",
        "Results:".bold(),
        dataset.completed,
        total,
        failed,
    );
    println!(
        "  {} Mean token reduction: {:.0}%, Mean cost reduction: {:.0}%",
        "Savings:".bold(),
        dataset.aggregate.mean_total_tokens_pct,
        dataset.aggregate.mean_cost_pct,
    );
    println!("  {} ${:.2}", "Total cost:".bold(), total_cost,);

    Ok(())
}

/// Run a single issue comparison (fetch + compare).
fn run_single_issue(
    url: &str,
    model: &str,
    max_turns: u32,
    max_budget: f64,
) -> Result<IssueComparisonReport> {
    let issue_ref = IssueRef::parse(url)?;
    let issue = fetch_issue(url)?;
    run_issue_compare(url, &issue, &issue_ref, model, max_turns, max_budget)
}

fn print_dry_run(corpus: &Corpus, options: &BatchOptions) -> Result<()> {
    println!("{}", "DRY RUN — Batch Plan".cyan().bold());
    println!("{}", "=".repeat(64).dimmed());

    println!("\n  {} {}", "Corpus:".bold(), options.corpus_path.display());
    println!("  {} {}", "Issues:".bold(), corpus.issues.len());
    println!("  {} {}", "Model:".bold(), options.model);
    println!(
        "  {} ${:.2}",
        "Max budget (total):".bold(),
        options.max_budget
    );
    println!("  {} {}", "Output:".bold(), options.output_dir.display());

    // Tag breakdown
    let mut tag_counts: HashMap<&str, usize> = HashMap::new();
    for issue in &corpus.issues {
        for tag in &issue.tags {
            *tag_counts.entry(tag.as_str()).or_insert(0) += 1;
        }
    }
    if !tag_counts.is_empty() {
        println!("\n  {}", "Tags:".bold());
        let mut tags: Vec<_> = tag_counts.iter().collect();
        tags.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
        for (tag, count) in &tags {
            println!("    {} ({})", tag, count);
        }
    }

    // Cost estimate: capped by total budget, but each issue runs 2 Claude invocations
    let per_issue_est = 5.0_f64.min(options.max_budget / corpus.issues.len().max(1) as f64);
    let total_est = (per_issue_est * 2.0 * corpus.issues.len() as f64).min(options.max_budget);
    println!(
        "\n  {} ${:.2} (budget cap: ${:.2}, est ${:.2}/issue x 2 variants x {} issues)",
        "Est. max cost:".bold(),
        total_est,
        options.max_budget,
        per_issue_est,
        corpus.issues.len(),
    );

    println!("\n  {}", "Issues:".bold());
    for (i, issue) in corpus.issues.iter().enumerate() {
        let tags = if issue.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", issue.tags.join(", "))
        };
        println!("    {}. {}{}", i + 1, issue.url, tags.dimmed());
    }

    println!(
        "\n{} Run without --dry-run to execute",
        "Ready.".green().bold()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gh::report::{IssueComparisonReport, ReportInput};
    use crate::metrics::RunMetrics;
    use std::collections::HashMap as StdHashMap;
    use tempfile::TempDir;

    fn make_metrics(input: u64, output: u64, cost: f64, turns: u32, tools: u32) -> RunMetrics {
        RunMetrics {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: cost,
            turns,
            duration_ms: turns as u64 * 5000,
            tool_calls: tools,
            tools_by_name: StdHashMap::new(),
            files_accessed: vec!["a.rs".to_string()],
            read_calls: tools / 2,
            success: true,
            error: None,
        }
    }

    fn make_report(
        issue_num: u64,
        control_input: u64,
        control_cost: f64,
        fmm_input: u64,
        fmm_cost: f64,
    ) -> IssueComparisonReport {
        let control = make_metrics(control_input, 1000, control_cost, 10, 20);
        let fmm = make_metrics(fmm_input, 800, fmm_cost, 4, 8);

        IssueComparisonReport::new(ReportInput {
            issue_url: &format!("https://github.com/test/repo/issues/{}", issue_num),
            issue_title: &format!("Test issue {}", issue_num),
            issue_number: issue_num,
            repo: "test/repo",
            model: "sonnet",
            max_budget_usd: 5.0,
            max_turns: 30,
            control_metrics: &control,
            fmm_metrics: &fmm,
            control_diff: "",
            fmm_diff: "",
        })
    }

    #[test]
    fn corpus_parse_valid() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("issues.json");
        fs::write(
            &path,
            r#"{"issues":[
                {"url":"https://github.com/o/r/issues/1","tags":["rust","small","bugfix"]},
                {"url":"https://github.com/o/r/issues/2","tags":["python","large","feature"]}
            ]}"#,
        )
        .unwrap();

        let corpus = Corpus::load(&path).unwrap();
        assert_eq!(corpus.issues.len(), 2);
        assert_eq!(corpus.issues[0].tags, vec!["rust", "small", "bugfix"]);
    }

    #[test]
    fn corpus_parse_empty_fails() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("issues.json");
        fs::write(&path, r#"{"issues":[]}"#).unwrap();
        assert!(Corpus::load(&path).is_err());
    }

    #[test]
    fn corpus_parse_no_tags() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("issues.json");
        fs::write(
            &path,
            r#"{"issues":[{"url":"https://github.com/o/r/issues/1"}]}"#,
        )
        .unwrap();
        let corpus = Corpus::load(&path).unwrap();
        assert!(corpus.issues[0].tags.is_empty());
    }

    #[test]
    fn aggregate_savings_calculates_means() {
        let r1 = make_report(1, 10000, 0.50, 3000, 0.15);
        let r2 = make_report(2, 8000, 0.40, 4000, 0.20);
        let reports: Vec<&IssueComparisonReport> = vec![&r1, &r2];

        let stats = aggregate_savings(&reports);
        assert_eq!(stats.count, 2);
        assert!(stats.mean_input_tokens_pct > 0.0);
        assert!(stats.mean_cost_pct > 0.0);
        assert!(stats.total_control_cost_usd > 0.0);
        assert!(stats.total_fmm_cost_usd > 0.0);
    }

    #[test]
    fn aggregate_savings_empty() {
        let stats = aggregate_savings(&[]);
        assert_eq!(stats.count, 0);
        assert_eq!(stats.mean_cost_pct, 0.0);
    }

    #[test]
    fn median_odd() {
        assert!((median(&[1.0, 2.0, 3.0]) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn median_even() {
        assert!((median(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn median_empty() {
        assert!((median(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn median_single() {
        assert!((median(&[42.0]) - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn checkpoint_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let report = make_report(1, 10000, 0.50, 3000, 0.15);

        let entry = CheckpointEntry {
            url: "https://github.com/o/r/issues/1".to_string(),
            status: CheckpointStatus::Done,
            report: Some(report),
            error: None,
        };

        append_checkpoint(tmp.path(), &entry).unwrap();

        let cp = load_checkpoint(tmp.path());
        assert_eq!(cp.len(), 1);
        assert!(cp.contains_key("https://github.com/o/r/issues/1"));
        assert_eq!(
            cp["https://github.com/o/r/issues/1"].status,
            CheckpointStatus::Done
        );
    }

    #[test]
    fn checkpoint_multiple_appends() {
        let tmp = TempDir::new().unwrap();

        for i in 1..=3 {
            let entry = CheckpointEntry {
                url: format!("https://github.com/o/r/issues/{}", i),
                status: CheckpointStatus::Done,
                report: Some(make_report(i, 10000, 0.50, 3000, 0.15)),
                error: None,
            };
            append_checkpoint(tmp.path(), &entry).unwrap();
        }

        let cp = load_checkpoint(tmp.path());
        assert_eq!(cp.len(), 3);
    }

    #[test]
    fn checkpoint_failed_entry() {
        let tmp = TempDir::new().unwrap();

        let entry = CheckpointEntry {
            url: "https://github.com/o/r/issues/99".to_string(),
            status: CheckpointStatus::Failed,
            report: None,
            error: Some("fetch failed".to_string()),
        };
        append_checkpoint(tmp.path(), &entry).unwrap();

        let cp = load_checkpoint(tmp.path());
        assert_eq!(
            cp["https://github.com/o/r/issues/99"].status,
            CheckpointStatus::Failed
        );
        assert!(cp["https://github.com/o/r/issues/99"].error.is_some());
    }

    #[test]
    fn proof_markdown_contains_headline_numbers() {
        let r1 = make_report(1, 10000, 0.50, 3000, 0.15);
        let r2 = make_report(2, 8000, 0.40, 4000, 0.20);

        let corpus = Corpus {
            issues: vec![
                CorpusIssue {
                    url: r1.issue_url.clone(),
                    tags: vec!["rust".to_string()],
                },
                CorpusIssue {
                    url: r2.issue_url.clone(),
                    tags: vec!["python".to_string()],
                },
            ],
        };

        let report_refs: Vec<&IssueComparisonReport> = vec![&r1, &r2];
        let aggregate = aggregate_savings(&report_refs);

        let dataset = ProofDataset {
            generated_at: "2026-01-31T00:00:00Z".to_string(),
            corpus_size: 2,
            completed: 2,
            failed: 0,
            aggregate,
            by_tag: HashMap::new(),
            issues: vec![r1, r2],
        };

        let md = generate_proof_markdown(&dataset, &corpus);
        assert!(md.contains("FMM Proof Dataset"));
        assert!(md.contains("**2** issues"));
        assert!(md.contains("languages"));
        assert!(md.contains("Per-Issue Results"));
        assert!(md.contains("test/repo"));
    }

    // ── Validation / health report tests ──

    fn make_corpus(tags_list: Vec<Vec<&str>>) -> Corpus {
        Corpus {
            issues: tags_list
                .into_iter()
                .enumerate()
                .map(|(i, tags)| CorpusIssue {
                    url: format!("https://github.com/o/r/issues/{}", i + 1),
                    tags: tags.into_iter().map(String::from).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn tag_distribution_counts_correctly() {
        let corpus = make_corpus(vec![
            vec!["typescript", "small", "bugfix"],
            vec!["typescript", "medium", "bugfix"],
            vec!["python", "large", "feature"],
            vec!["rust", "small", "refactor"],
        ]);

        let dist = tag_distribution(&corpus);
        assert_eq!(dist.len(), 3);

        // Languages
        let (label, langs) = &dist[0];
        assert_eq!(*label, "Languages");
        let ts_count = langs.iter().find(|(t, _)| *t == "typescript").unwrap().1;
        assert_eq!(ts_count, 2);
        let py_count = langs.iter().find(|(t, _)| *t == "python").unwrap().1;
        assert_eq!(py_count, 1);
        let rust_count = langs.iter().find(|(t, _)| *t == "rust").unwrap().1;
        assert_eq!(rust_count, 1);
        let go_count = langs.iter().find(|(t, _)| *t == "go").unwrap().1;
        assert_eq!(go_count, 0);

        // Sizes
        let (label, sizes) = &dist[1];
        assert_eq!(*label, "Sizes");
        let small = sizes.iter().find(|(t, _)| *t == "small").unwrap().1;
        assert_eq!(small, 2);
        let medium = sizes.iter().find(|(t, _)| *t == "medium").unwrap().1;
        assert_eq!(medium, 1);
        let large = sizes.iter().find(|(t, _)| *t == "large").unwrap().1;
        assert_eq!(large, 1);

        // Task types
        let (label, tasks) = &dist[2];
        assert_eq!(*label, "Task types");
        let bugfix = tasks.iter().find(|(t, _)| *t == "bugfix").unwrap().1;
        assert_eq!(bugfix, 2);
        let feature = tasks.iter().find(|(t, _)| *t == "feature").unwrap().1;
        assert_eq!(feature, 1);
        let refactor = tasks.iter().find(|(t, _)| *t == "refactor").unwrap().1;
        assert_eq!(refactor, 1);
        let perf = tasks.iter().find(|(t, _)| *t == "perf").unwrap().1;
        assert_eq!(perf, 0);
    }

    #[test]
    fn tag_distribution_empty_corpus() {
        let corpus = Corpus {
            issues: vec![CorpusIssue {
                url: "https://github.com/o/r/issues/1".to_string(),
                tags: vec![],
            }],
        };
        let dist = tag_distribution(&corpus);
        for (_label, tags) in &dist {
            for (_, count) in tags {
                assert_eq!(*count, 0);
            }
        }
    }

    #[test]
    fn detect_gaps_finds_low_count_tags() {
        let corpus = make_corpus(vec![
            vec!["typescript", "small", "bugfix"],
            vec!["typescript", "small", "bugfix"],
            vec!["typescript", "small", "bugfix"],
            vec!["typescript", "small", "bugfix"],
            vec!["python", "medium", "feature"],
            vec!["python", "medium", "feature"],
            vec!["python", "medium", "feature"],
        ]);
        let dist = tag_distribution(&corpus);
        let gaps = detect_gaps(&dist);

        // python has 3 (<4), small has 4 (ok), medium has 3 (<4), bugfix has 4 (ok), feature has 3 (<4)
        let gap_tags: Vec<&str> = gaps.iter().map(|(t, _)| t.as_str()).collect();
        assert!(gap_tags.contains(&"python"));
        assert!(gap_tags.contains(&"medium"));
        assert!(gap_tags.contains(&"feature"));
        assert!(!gap_tags.contains(&"typescript"));
        assert!(!gap_tags.contains(&"small"));
        assert!(!gap_tags.contains(&"bugfix"));
    }

    #[test]
    fn detect_gaps_ignores_zero_count_tags() {
        let corpus = make_corpus(vec![vec!["typescript", "small", "bugfix"]]);
        let dist = tag_distribution(&corpus);
        let gaps = detect_gaps(&dist);

        // Tags with 0 count should NOT appear as gaps (they're absent, not low)
        // Tags with count 1 should appear
        let gap_tags: Vec<&str> = gaps.iter().map(|(t, _)| t.as_str()).collect();
        assert!(gap_tags.contains(&"typescript")); // count=1, < 4
        assert!(!gap_tags.contains(&"go")); // count=0, not a gap
        assert!(!gap_tags.contains(&"perf")); // count=0, not a gap
    }

    #[test]
    fn detect_gaps_none_when_all_sufficient() {
        // 4 issues per language — no gaps expected for languages present
        let mut tags_list = Vec::new();
        for lang in LANGUAGES {
            for _ in 0..4 {
                tags_list.push(vec![*lang, "small", "bugfix"]);
            }
        }
        let corpus = make_corpus(tags_list);
        let dist = tag_distribution(&corpus);
        let gaps = detect_gaps(&dist);

        // Languages all have 4+ — no language gaps
        let gap_tags: Vec<&str> = gaps.iter().map(|(t, _)| t.as_str()).collect();
        for lang in LANGUAGES {
            assert!(!gap_tags.contains(lang));
        }
    }

    #[test]
    fn format_distribution_row_skips_zeros() {
        let tags = vec![("typescript", 5), ("python", 0), ("rust", 3)];
        let row = format_distribution_row(&tags);
        assert_eq!(row, "typescript(5) rust(3)");
        assert!(!row.contains("python"));
    }

    #[test]
    fn format_distribution_row_empty() {
        let tags: Vec<(&str, usize)> = vec![("a", 0), ("b", 0)];
        let row = format_distribution_row(&tags);
        assert_eq!(row, "");
    }
}
