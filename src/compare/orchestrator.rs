//! Comparison orchestrator - coordinates all components

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use super::cache::{CacheKey, CacheManager};
use super::report::{ComparisonReport, ReportFormat};
use super::runner::{ClaudeRunner, RunResult};
use super::sandbox::Sandbox;
use super::tasks::{Task, TaskSet};

/// Options for comparison run
#[derive(Debug, Clone)]
pub struct CompareOptions {
    /// Branch to compare (default: main)
    pub branch: Option<String>,
    /// Path within repo to analyze (default: src/)
    #[allow(dead_code)]
    pub src_path: Option<String>,
    /// Task set to use (standard, quick, or custom path)
    pub task_set: String,
    /// Number of runs per task (for averaging)
    #[allow(dead_code)]
    pub runs: u32,
    /// Output directory for results
    pub output: Option<PathBuf>,
    /// Output format
    pub format: ReportFormat,
    /// Maximum budget in USD
    pub max_budget: f64,
    /// Use cached results when available
    pub use_cache: bool,
    /// Quick mode (fewer tasks)
    pub quick: bool,
    /// Model to use
    #[allow(dead_code)]
    pub model: String,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            branch: None,
            src_path: None,
            task_set: "standard".to_string(),
            runs: 1,
            output: None,
            format: ReportFormat::Both,
            max_budget: 10.0,
            use_cache: true,
            quick: false,
            model: "sonnet".to_string(),
        }
    }
}

/// Orchestrator for comparison runs
pub struct Orchestrator {
    options: CompareOptions,
    cache: CacheManager,
    runner: ClaudeRunner,
    total_cost: f64,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub fn new(options: CompareOptions) -> Result<Self> {
        let cache = CacheManager::new(None)?;
        let runner = ClaudeRunner::new();

        Ok(Self {
            options,
            cache,
            runner,
            total_cost: 0.0,
        })
    }

    /// Run comparison on a repository
    pub fn run(&mut self, url: &str) -> Result<ComparisonReport> {
        let job_id = generate_job_id();

        println!("{} Job ID: {}", "📋".yellow(), job_id.cyan());

        // Step 1: Create sandbox and clone repo
        println!("{} Setting up sandbox...", "🔧".yellow());
        let sandbox = Sandbox::new(&job_id)?;
        sandbox.clone_repo(url, self.options.branch.as_deref())?;

        let commit_sha = sandbox.get_commit_sha(&sandbox.control_dir)?;
        println!(
            "  {} Cloned at commit {}",
            "✓".green(),
            &commit_sha[..8].dimmed()
        );

        // Step 2: Generate FMM manifest for FMM variant
        println!("{} Generating FMM manifest...", "🔧".yellow());
        sandbox.generate_fmm_manifest()?;

        // Check if manifest was generated
        let manifest_path = sandbox.fmm_dir.join(".fmm").join("index.json");
        if manifest_path.exists() {
            let metadata = fs::metadata(&manifest_path)?;
            println!(
                "  {} Manifest generated ({} bytes)",
                "✓".green(),
                metadata.len()
            );
        } else {
            println!(
                "  {} No manifest generated (unsupported language?)",
                "!".yellow()
            );
        }

        // Step 3: Load tasks
        let task_set = if self.options.quick {
            TaskSet::quick()
        } else {
            match self.options.task_set.as_str() {
                "standard" => TaskSet::standard(),
                "quick" => TaskSet::quick(),
                path => self.load_custom_tasks(path)?,
            }
        };

        println!(
            "{} Running {} tasks...",
            "🚀".yellow(),
            task_set.tasks.len()
        );

        // Step 4: Run tasks
        let mut results: Vec<(Task, RunResult, RunResult)> = vec![];

        for (i, task) in task_set.tasks.iter().enumerate() {
            println!(
                "\n{} Task {}/{}: {}",
                "▶".cyan(),
                i + 1,
                task_set.tasks.len(),
                task.name.white().bold()
            );

            // Check budget
            if self.total_cost >= self.options.max_budget {
                println!(
                    "{} Budget limit reached (${:.2} / ${:.2})",
                    "⚠".yellow(),
                    self.total_cost,
                    self.options.max_budget
                );
                break;
            }

            // Run control variant
            let control_result =
                self.run_task_with_cache(task, &sandbox.control_dir, "control", url, &commit_sha)?;

            // Run FMM variant
            let fmm_context = self.build_fmm_context(&sandbox.fmm_dir)?;
            let fmm_result = self.run_task_with_fmm(
                task,
                &sandbox.fmm_dir,
                "fmm",
                url,
                &commit_sha,
                &fmm_context,
            )?;

            // Update cost tracking
            self.total_cost += control_result.total_cost_usd + fmm_result.total_cost_usd;

            // Report progress
            let reduction = if control_result.tool_calls > 0 {
                ((control_result.tool_calls as f64 - fmm_result.tool_calls as f64)
                    / control_result.tool_calls as f64)
                    * 100.0
            } else {
                0.0
            };

            println!(
                "  Control: {} tools | FMM: {} tools | Reduction: {:.1}%",
                control_result.tool_calls, fmm_result.tool_calls, reduction
            );

            results.push((task.clone(), control_result, fmm_result));
        }

        // Step 5: Generate report
        println!("\n{} Generating report...", "📊".yellow());
        let branch = self
            .options
            .branch
            .clone()
            .unwrap_or_else(|| "main".to_string());
        let report = ComparisonReport::new(job_id, url.to_string(), commit_sha, branch, results);

        // Save report
        if let Some(ref output_dir) = self.options.output {
            let saved = report.save(output_dir, self.options.format)?;
            for path in saved {
                println!("  {} Saved: {}", "✓".green(), path.dimmed());
            }
        }

        // Also save to cache
        let report_path = self.cache.save_report(&report)?;
        println!(
            "  {} Cached: {}",
            "✓".green(),
            report_path.display().to_string().dimmed()
        );

        println!("\n{} Total cost: ${:.4}", "💰".yellow(), self.total_cost);

        Ok(report)
    }

    fn run_task_with_cache(
        &mut self,
        task: &Task,
        working_dir: &std::path::Path,
        variant: &str,
        repo_url: &str,
        commit_sha: &str,
    ) -> Result<RunResult> {
        // Check cache
        if self.options.use_cache {
            let cache_key = CacheKey::new(repo_url, commit_sha, &task.id, variant);
            if let Some(cached) = self.cache.get(&cache_key) {
                println!("  {} {} (cached)", "●".dimmed(), variant.dimmed());
                return Ok(cached);
            }
        }

        // Run task
        print!("  {} {}...", "●".cyan(), variant);
        let result = self.runner.run_task(task, working_dir, variant, None)?;

        // Cache result
        if self.options.use_cache && result.success {
            let cache_key = CacheKey::new(repo_url, commit_sha, &task.id, variant);
            self.cache.set(cache_key, result.clone())?;
        }

        println!(
            " {} ({} tools, ${:.4})",
            if result.success {
                "✓".green()
            } else {
                "✗".red()
            },
            result.tool_calls,
            result.total_cost_usd
        );

        Ok(result)
    }

    fn run_task_with_fmm(
        &mut self,
        task: &Task,
        working_dir: &std::path::Path,
        variant: &str,
        repo_url: &str,
        commit_sha: &str,
        fmm_context: &str,
    ) -> Result<RunResult> {
        // Check cache
        if self.options.use_cache {
            let cache_key = CacheKey::new(repo_url, commit_sha, &task.id, variant);
            if let Some(cached) = self.cache.get(&cache_key) {
                println!("  {} {} (cached)", "●".dimmed(), variant.dimmed());
                return Ok(cached);
            }
        }

        // Run task with FMM context
        print!("  {} {}...", "●".cyan(), variant);
        let context = if fmm_context.is_empty() {
            None
        } else {
            Some(fmm_context)
        };
        let result = self.runner.run_task(task, working_dir, variant, context)?;

        // Cache result
        if self.options.use_cache && result.success {
            let cache_key = CacheKey::new(repo_url, commit_sha, &task.id, variant);
            self.cache.set(cache_key, result.clone())?;
        }

        println!(
            " {} ({} tools, ${:.4})",
            if result.success {
                "✓".green()
            } else {
                "✗".red()
            },
            result.tool_calls,
            result.total_cost_usd
        );

        Ok(result)
    }

    fn build_fmm_context(&self, fmm_dir: &std::path::Path) -> Result<String> {
        let manifest_path = fmm_dir.join(".fmm").join("index.json");

        if !manifest_path.exists() {
            return Ok(String::new());
        }

        let manifest_content =
            fs::read_to_string(&manifest_path).context("Failed to read FMM manifest")?;

        // Build context prompt
        let context = format!(
            r#"IMPORTANT: This repository has an FMM (Frontmatter Matters) manifest available.

Before exploring the codebase with file reads, FIRST consult this manifest to understand the codebase structure.
The manifest contains:
- File paths and their exports
- Import relationships
- Line counts
- An export index for quick lookups

FMM MANIFEST:
```json
{}
```

Use this manifest to:
1. Find files by export name without reading them
2. Understand file relationships before diving into code
3. Identify entry points and main modules
4. Reduce unnecessary file reads

Only read files when you need the actual implementation details."#,
            manifest_content
        );

        Ok(context)
    }

    fn load_custom_tasks(&self, path: &str) -> Result<TaskSet> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to load custom tasks from {}", path))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse custom tasks from {}", path))
    }
}

fn generate_job_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let timestamp = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Use nanoseconds for randomness within the same second
    let random: u16 = ((nanos / 1000) % 65536) as u16;

    format!("cmp-{:x}-{:04x}", timestamp, random)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_generation() {
        let id1 = generate_job_id();

        assert!(id1.starts_with("cmp-"));
        assert!(!id1.is_empty());
        // Just verify format, not uniqueness (timing-dependent)
        assert!(id1.len() > 10);
    }

    #[test]
    fn test_default_options() {
        let opts = CompareOptions::default();
        assert_eq!(opts.runs, 1);
        assert_eq!(opts.max_budget, 10.0);
        assert!(opts.use_cache);
    }
}
