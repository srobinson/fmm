//! Comparison report for `fmm gh issue --compare` — markdown + JSON output.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::metrics::RunMetrics;

/// Complete A/B comparison report for a single GitHub issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComparisonReport {
    pub issue_url: String,
    pub issue_title: String,
    pub issue_number: u64,
    pub repo: String,
    pub model: String,
    pub max_budget_usd: f64,
    pub max_turns: u32,
    pub timestamp: String,
    pub control: VariantResult,
    pub fmm: VariantResult,
    pub savings: Savings,
    pub verdict: String,
}

/// Metrics for one variant (control or fmm).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResult {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
    pub turns: u32,
    pub duration_ms: u64,
    pub tool_calls: u32,
    pub read_calls: u32,
    pub files_read: u32,
    pub files_changed: u32,
    pub tools_by_name: HashMap<String, u32>,
    pub success: bool,
    pub error: Option<String>,
    #[serde(skip)]
    pub diff: String,
}

impl VariantResult {
    fn from_metrics(m: &RunMetrics, diff: &str) -> Self {
        let files_changed = diff.lines().filter(|l| l.starts_with("diff --git")).count() as u32;

        Self {
            input_tokens: m.input_tokens,
            output_tokens: m.output_tokens,
            cache_read_tokens: m.cache_read_tokens,
            cache_creation_tokens: m.cache_creation_tokens,
            cost_usd: m.cost_usd,
            turns: m.turns,
            duration_ms: m.duration_ms,
            tool_calls: m.tool_calls,
            read_calls: m.read_calls,
            files_read: m.files_accessed.len() as u32,
            files_changed,
            tools_by_name: m.tools_by_name.clone(),
            success: m.success,
            error: m.error.clone(),
            diff: diff.to_string(),
        }
    }
}

/// Calculated savings between control and fmm variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Savings {
    pub input_tokens_pct: f64,
    pub total_tokens_pct: f64,
    pub cost_pct: f64,
    pub turns_pct: f64,
    pub tool_calls_pct: f64,
    pub files_read_pct: f64,
    pub duration_pct: f64,
}

/// Input parameters for creating a comparison report.
pub struct ReportInput<'a> {
    pub issue_url: &'a str,
    pub issue_title: &'a str,
    pub issue_number: u64,
    pub repo: &'a str,
    pub model: &'a str,
    pub max_budget_usd: f64,
    pub max_turns: u32,
    pub control_metrics: &'a RunMetrics,
    pub fmm_metrics: &'a RunMetrics,
    pub control_diff: &'a str,
    pub fmm_diff: &'a str,
}

impl IssueComparisonReport {
    pub fn new(input: ReportInput<'_>) -> Self {
        let control = VariantResult::from_metrics(input.control_metrics, input.control_diff);
        let fmm = VariantResult::from_metrics(input.fmm_metrics, input.fmm_diff);
        let savings = Self::calculate_savings(&control, &fmm);
        let verdict = Self::generate_verdict(&savings);

        Self {
            issue_url: input.issue_url.to_string(),
            issue_title: input.issue_title.to_string(),
            issue_number: input.issue_number,
            repo: input.repo.to_string(),
            model: input.model.to_string(),
            max_budget_usd: input.max_budget_usd,
            max_turns: input.max_turns,
            timestamp: chrono::Utc::now().to_rfc3339(),
            control,
            fmm,
            savings,
            verdict,
        }
    }

    fn calculate_savings(control: &VariantResult, fmm: &VariantResult) -> Savings {
        Savings {
            input_tokens_pct: reduction_pct(control.input_tokens as f64, fmm.input_tokens as f64),
            total_tokens_pct: reduction_pct(
                (control.input_tokens + control.output_tokens) as f64,
                (fmm.input_tokens + fmm.output_tokens) as f64,
            ),
            cost_pct: reduction_pct(control.cost_usd, fmm.cost_usd),
            turns_pct: reduction_pct(control.turns as f64, fmm.turns as f64),
            tool_calls_pct: reduction_pct(control.tool_calls as f64, fmm.tool_calls as f64),
            files_read_pct: reduction_pct(control.files_read as f64, fmm.files_read as f64),
            duration_pct: reduction_pct(control.duration_ms as f64, fmm.duration_ms as f64),
        }
    }

    fn generate_verdict(savings: &Savings) -> String {
        if savings.total_tokens_pct > 0.0 {
            format!(
                "fmm reduced token usage by {:.0}% and cost by {:.0}%.",
                savings.total_tokens_pct, savings.cost_pct
            )
        } else {
            "fmm did not reduce token usage in this run.".to_string()
        }
    }

    /// Print colorized summary to terminal.
    pub fn print_summary(&self) {
        println!("\n{}", "═".repeat(64).dimmed());
        println!("{}", "fmm gh issue --compare Results".green().bold());
        println!("{}", "═".repeat(64).dimmed());

        println!(
            "\n  {} {}#{} — {}",
            "Issue:".bold(),
            self.repo,
            self.issue_number,
            self.issue_title
        );
        println!(
            "  {} {} | Budget: ${:.2} | Max turns: {}",
            "Model:".bold(),
            self.model,
            self.max_budget_usd,
            self.max_turns
        );

        println!(
            "\n  {:20} {:>10} {:>10} {:>10} {:>8}",
            "", "Control", "FMM", "Delta", "Savings"
        );
        println!("  {}", "─".repeat(60));

        self.print_row(
            "Input tokens",
            self.control.input_tokens,
            self.fmm.input_tokens,
            self.savings.input_tokens_pct,
        );
        self.print_row(
            "Output tokens",
            self.control.output_tokens,
            self.fmm.output_tokens,
            -1.0,
        );
        self.print_row_special(
            "Cache read tokens",
            self.control.cache_read_tokens,
            self.fmm.cache_read_tokens,
        );
        self.print_row_cost(
            "Total cost",
            self.control.cost_usd,
            self.fmm.cost_usd,
            self.savings.cost_pct,
        );
        self.print_row(
            "Turns",
            self.control.turns as u64,
            self.fmm.turns as u64,
            self.savings.turns_pct,
        );
        self.print_row(
            "Tool calls",
            self.control.tool_calls as u64,
            self.fmm.tool_calls as u64,
            self.savings.tool_calls_pct,
        );
        self.print_row(
            "Files read",
            self.control.files_read as u64,
            self.fmm.files_read as u64,
            self.savings.files_read_pct,
        );
        self.print_row(
            "Files changed",
            self.control.files_changed as u64,
            self.fmm.files_changed as u64,
            -1.0,
        );
        self.print_duration_row();

        // Tool call breakdown
        if !self.control.tools_by_name.is_empty() || !self.fmm.tools_by_name.is_empty() {
            println!("\n  {}", "Tool breakdown:".bold());
            let mut all_tools: Vec<&str> = self
                .control
                .tools_by_name
                .keys()
                .chain(self.fmm.tools_by_name.keys())
                .map(|s| s.as_str())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            all_tools.sort();

            for tool in &all_tools {
                let c = self.control.tools_by_name.get(*tool).copied().unwrap_or(0);
                let f = self.fmm.tools_by_name.get(*tool).copied().unwrap_or(0);
                println!("    {:16} {:>6} {:>6}", tool, c, f);
            }
        }

        println!("\n  {} {}", "Verdict:".bold(), self.verdict.green());
    }

    fn print_row(&self, label: &str, control: u64, fmm: u64, savings_pct: f64) {
        let delta = fmm as i64 - control as i64;
        let delta_str = if delta <= 0 {
            format!("{}", delta)
        } else {
            format!("+{}", delta)
        };
        let savings_str = if savings_pct < 0.0 {
            "—".to_string()
        } else {
            format!("{:.0}%", savings_pct)
        };
        println!(
            "  {:20} {:>10} {:>10} {:>10} {:>8}",
            label,
            format_number(control),
            format_number(fmm),
            delta_str,
            savings_str
        );
    }

    fn print_row_special(&self, label: &str, control: u64, fmm: u64) {
        let delta = fmm as i64 - control as i64;
        let delta_str = if delta <= 0 {
            format!("{}", delta)
        } else {
            format!("+{}", delta)
        };
        println!(
            "  {:20} {:>10} {:>10} {:>10} {:>8}",
            label,
            format_number(control),
            format_number(fmm),
            delta_str,
            "—"
        );
    }

    fn print_row_cost(&self, label: &str, control: f64, fmm: f64, savings_pct: f64) {
        let delta = fmm - control;
        let delta_str = if delta.abs() < 0.005 {
            "$0.00".to_string()
        } else if delta < 0.0 {
            format!("-${:.2}", delta.abs())
        } else {
            format!("+${:.2}", delta)
        };
        println!(
            "  {:20} {:>10} {:>10} {:>10} {:>8}",
            label,
            format!("${:.2}", control),
            format!("${:.2}", fmm),
            delta_str,
            format!("{:.0}%", savings_pct)
        );
    }

    fn print_duration_row(&self) {
        let c = self.control.duration_ms as f64 / 1000.0;
        let f = self.fmm.duration_ms as f64 / 1000.0;
        let delta = f - c;
        let delta_str = if delta.abs() < 0.5 {
            "0s".to_string()
        } else if delta < 0.0 {
            format!("-{:.0}s", delta.abs())
        } else {
            format!("+{:.0}s", delta)
        };
        println!(
            "  {:20} {:>10} {:>10} {:>10} {:>8}",
            "Duration",
            format!("{:.0}s", c),
            format!("{:.0}s", f),
            delta_str,
            format!("{:.0}%", self.savings.duration_pct)
        );
    }

    /// Generate markdown report string.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("## fmm gh issue --compare Results\n\n");
        md.push_str(&format!(
            "**Issue:** {}#{} — {}\n",
            self.repo, self.issue_number, self.issue_title
        ));
        md.push_str(&format!(
            "**Model:** {} | **Budget:** ${:.2} | **Max turns:** {}\n",
            self.model, self.max_budget_usd, self.max_turns
        ));
        md.push_str(&format!("**Timestamp:** {}\n\n", self.timestamp));

        md.push_str("| Metric | Control | FMM | Delta | Savings |\n");
        md.push_str("|--------|---------|-----|-------|---------|\n");
        md.push_str(&format!(
            "| Input tokens | {} | {} | {} | {:.0}% |\n",
            format_number(self.control.input_tokens),
            format_number(self.fmm.input_tokens),
            delta_str(self.control.input_tokens, self.fmm.input_tokens),
            self.savings.input_tokens_pct
        ));
        md.push_str(&format!(
            "| Output tokens | {} | {} | {} | — |\n",
            format_number(self.control.output_tokens),
            format_number(self.fmm.output_tokens),
            delta_str(self.control.output_tokens, self.fmm.output_tokens)
        ));
        md.push_str(&format!(
            "| Cache read tokens | {} | {} | {} | — |\n",
            format_number(self.control.cache_read_tokens),
            format_number(self.fmm.cache_read_tokens),
            delta_str_signed(self.control.cache_read_tokens, self.fmm.cache_read_tokens)
        ));
        md.push_str(&format!(
            "| Total cost | ${:.2} | ${:.2} | {} | {:.0}% |\n",
            self.control.cost_usd,
            self.fmm.cost_usd,
            cost_delta_str(self.control.cost_usd, self.fmm.cost_usd),
            self.savings.cost_pct
        ));
        md.push_str(&format!(
            "| Turns | {} | {} | {} | {:.0}% |\n",
            self.control.turns,
            self.fmm.turns,
            delta_str(self.control.turns as u64, self.fmm.turns as u64),
            self.savings.turns_pct
        ));
        md.push_str(&format!(
            "| Tool calls | {} | {} | {} | {:.0}% |\n",
            self.control.tool_calls,
            self.fmm.tool_calls,
            delta_str(self.control.tool_calls as u64, self.fmm.tool_calls as u64),
            self.savings.tool_calls_pct
        ));
        md.push_str(&format!(
            "| Files read | {} | {} | {} | {:.0}% |\n",
            self.control.files_read,
            self.fmm.files_read,
            delta_str(self.control.files_read as u64, self.fmm.files_read as u64),
            self.savings.files_read_pct
        ));
        md.push_str(&format!(
            "| Files changed | {} | {} | {} | — |\n",
            self.control.files_changed,
            self.fmm.files_changed,
            delta_str(
                self.control.files_changed as u64,
                self.fmm.files_changed as u64
            )
        ));
        md.push_str(&format!(
            "| Duration | {:.0}s | {:.0}s | {} | {:.0}% |\n\n",
            self.control.duration_ms as f64 / 1000.0,
            self.fmm.duration_ms as f64 / 1000.0,
            duration_delta_str(self.control.duration_ms, self.fmm.duration_ms),
            self.savings.duration_pct
        ));

        // Tool breakdown
        if !self.control.tools_by_name.is_empty() || !self.fmm.tools_by_name.is_empty() {
            md.push_str("### Tool Breakdown\n\n");
            md.push_str("| Tool | Control | FMM |\n");
            md.push_str("|------|---------|-----|\n");

            let mut all_tools: Vec<&str> = self
                .control
                .tools_by_name
                .keys()
                .chain(self.fmm.tools_by_name.keys())
                .map(|s| s.as_str())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            all_tools.sort();

            for tool in &all_tools {
                let c = self.control.tools_by_name.get(*tool).copied().unwrap_or(0);
                let f = self.fmm.tools_by_name.get(*tool).copied().unwrap_or(0);
                md.push_str(&format!("| {} | {} | {} |\n", tool, c, f));
            }
            md.push('\n');
        }

        md.push_str(&format!("**Verdict:** {}\n", self.verdict));

        // Truncated diffs
        fn truncated_diff(diff: &str, max_lines: usize) -> String {
            let lines: Vec<&str> = diff.lines().collect();
            if lines.is_empty() {
                return "(no changes)".to_string();
            }
            let shown: Vec<&str> = lines.iter().take(max_lines).copied().collect();
            let mut out = shown.join("\n");
            if lines.len() > max_lines {
                out.push_str(&format!("\n\n... ({} more lines)", lines.len() - max_lines));
            }
            out
        }

        if !self.control.diff.is_empty() || !self.fmm.diff.is_empty() {
            md.push_str("\n### Diffs\n\n");
            md.push_str("<details>\n<summary>Control diff</summary>\n\n```diff\n");
            md.push_str(&truncated_diff(&self.control.diff, 100));
            md.push_str("\n```\n</details>\n\n");
            md.push_str("<details>\n<summary>FMM diff</summary>\n\n```diff\n");
            md.push_str(&truncated_diff(&self.fmm.diff, 100));
            md.push_str("\n```\n</details>\n");
        }

        md
    }

    /// Save report as JSON and/or Markdown files.
    pub fn save(&self, output_dir: &Path) -> anyhow::Result<Vec<String>> {
        fs::create_dir_all(output_dir)?;
        let mut saved = vec![];

        let base = format!(
            "compare-{}_{}",
            self.repo.replace('/', "-"),
            self.issue_number
        );

        let json_path = output_dir.join(format!("{}.json", base));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&json_path, json)?;
        saved.push(json_path.display().to_string());

        let md_path = output_dir.join(format!("{}.md", base));
        fs::write(&md_path, self.to_markdown())?;
        saved.push(md_path.display().to_string());

        if !self.control.diff.is_empty() {
            let diff_path = output_dir.join("control.diff");
            fs::write(&diff_path, &self.control.diff)?;
            saved.push(diff_path.display().to_string());
        }

        if !self.fmm.diff.is_empty() {
            let diff_path = output_dir.join("fmm.diff");
            fs::write(&diff_path, &self.fmm.diff)?;
            saved.push(diff_path.display().to_string());
        }

        Ok(saved)
    }
}

fn reduction_pct(control: f64, fmm: f64) -> f64 {
    if control == 0.0 {
        0.0
    } else {
        ((control - fmm) / control) * 100.0
    }
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn delta_str(control: u64, fmm: u64) -> String {
    let d = fmm as i64 - control as i64;
    if d <= 0 {
        format!("{}", d)
    } else {
        format!("+{}", d)
    }
}

fn delta_str_signed(control: u64, fmm: u64) -> String {
    let d = fmm as i64 - control as i64;
    if d > 0 {
        format!("+{}", format_number(d as u64))
    } else if d < 0 {
        format!("-{}", format_number((-d) as u64))
    } else {
        "0".to_string()
    }
}

fn cost_delta_str(control: f64, fmm: f64) -> String {
    let d = fmm - control;
    if d.abs() < 0.005 {
        "$0.00".to_string()
    } else if d < 0.0 {
        format!("-${:.2}", d.abs())
    } else {
        format!("+${:.2}", d)
    }
}

fn duration_delta_str(control_ms: u64, fmm_ms: u64) -> String {
    let d = fmm_ms as i64 - control_ms as i64;
    let secs = d.abs() as f64 / 1000.0;
    if d == 0 {
        "0s".to_string()
    } else if d < 0 {
        format!("-{:.0}s", secs)
    } else {
        format!("+{:.0}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::RunMetrics;
    use std::collections::HashMap;

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
            tools_by_name: HashMap::new(),
            files_accessed: vec!["a.rs".to_string()],
            read_calls: tools / 2,
            success: true,
            error: None,
        }
    }

    #[test]
    fn report_calculates_savings() {
        let control = make_metrics(48000, 8000, 0.42, 12, 34);
        let fmm = make_metrics(12000, 7900, 0.11, 5, 14);

        let report = IssueComparisonReport::new(ReportInput {
            issue_url: "https://github.com/test/repo/issues/1",
            issue_title: "Fix bug",
            issue_number: 1,
            repo: "test/repo",
            model: "sonnet",
            max_budget_usd: 5.0,
            max_turns: 30,
            control_metrics: &control,
            fmm_metrics: &fmm,
            control_diff: "",
            fmm_diff: "",
        });

        assert!(report.savings.input_tokens_pct > 70.0);
        assert!(report.savings.cost_pct > 70.0);
        assert!(report.savings.tool_calls_pct > 50.0);
        assert!(report.verdict.contains("reduced"));
    }

    #[test]
    fn report_markdown_contains_key_fields() {
        let control = make_metrics(1000, 500, 0.05, 3, 10);
        let fmm = make_metrics(500, 400, 0.02, 2, 4);

        let report = IssueComparisonReport::new(ReportInput {
            issue_url: "https://github.com/o/r/issues/42",
            issue_title: "SQL injection",
            issue_number: 42,
            repo: "o/r",
            model: "sonnet",
            max_budget_usd: 5.0,
            max_turns: 30,
            control_metrics: &control,
            fmm_metrics: &fmm,
            control_diff: "",
            fmm_diff: "",
        });

        let md = report.to_markdown();
        assert!(md.contains("fmm gh issue --compare"));
        assert!(md.contains("o/r#42"));
        assert!(md.contains("SQL injection"));
        assert!(md.contains("Verdict"));
    }

    #[test]
    fn report_json_roundtrip() {
        let control = make_metrics(1000, 500, 0.05, 3, 10);
        let fmm = make_metrics(500, 400, 0.02, 2, 4);

        let report = IssueComparisonReport::new(ReportInput {
            issue_url: "https://github.com/o/r/issues/1",
            issue_title: "Bug",
            issue_number: 1,
            repo: "o/r",
            model: "sonnet",
            max_budget_usd: 5.0,
            max_turns: 30,
            control_metrics: &control,
            fmm_metrics: &fmm,
            control_diff: "",
            fmm_diff: "",
        });

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: IssueComparisonReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.issue_number, 1);
        assert_eq!(deserialized.control.input_tokens, 1000);
        assert_eq!(deserialized.fmm.input_tokens, 500);
    }

    #[test]
    fn reduction_pct_zero_control() {
        assert_eq!(reduction_pct(0.0, 100.0), 0.0);
    }

    #[test]
    fn format_number_ranges() {
        assert_eq!(format_number(42), "42");
        assert_eq!(format_number(1500), "1.5K");
        assert_eq!(format_number(2_500_000), "2.5M");
    }

    #[test]
    fn report_no_improvement_verdict() {
        let control = make_metrics(1000, 500, 0.05, 3, 10);
        let fmm = make_metrics(1200, 600, 0.06, 4, 12);

        let report = IssueComparisonReport::new(ReportInput {
            issue_url: "https://github.com/o/r/issues/1",
            issue_title: "Bug",
            issue_number: 1,
            repo: "o/r",
            model: "sonnet",
            max_budget_usd: 5.0,
            max_turns: 30,
            control_metrics: &control,
            fmm_metrics: &fmm,
            control_diff: "",
            fmm_diff: "",
        });

        assert!(report.verdict.contains("did not reduce"));
    }
}
