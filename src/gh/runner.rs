use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::metrics::{self, RunMetrics};

pub struct RunResult {
    pub success: bool,
    pub response_text: String,
    pub metrics: RunMetrics,
}

/// Configuration for how Claude is invoked (controls vs fmm variant settings).
pub struct InvokeOptions<'a> {
    pub prompt: &'a str,
    pub repo_dir: &'a Path,
    pub model: &'a str,
    pub max_turns: u32,
    pub max_budget: f64,
    /// Extra tools beyond the default set (Read,Write,Edit,Glob,Grep,Bash)
    pub allowed_tools: Option<&'a str>,
    /// Setting sources override (e.g., "" for fully isolated, "local" for fmm variant)
    pub setting_sources: Option<&'a str>,
    /// System prompt to append (fmm context injection)
    pub append_system_prompt: Option<&'a str>,
}

pub fn invoke_claude(
    prompt: &str,
    repo_dir: &Path,
    model: &str,
    max_turns: u32,
    max_budget: f64,
) -> Result<RunResult> {
    invoke_claude_with_options(InvokeOptions {
        prompt,
        repo_dir,
        model,
        max_turns,
        max_budget,
        allowed_tools: None,
        setting_sources: None,
        append_system_prompt: None,
    })
}

pub fn invoke_claude_with_options(opts: InvokeOptions<'_>) -> Result<RunResult> {
    let start = Instant::now();

    let mut cmd = Command::new("claude");

    cmd.arg("-p")
        .arg(opts.prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--max-turns")
        .arg(opts.max_turns.to_string())
        .arg("--max-budget-usd")
        .arg(opts.max_budget.to_string())
        .arg("--model")
        .arg(opts.model)
        .arg("--allowedTools")
        .arg(
            opts.allowed_tools
                .unwrap_or("Read,Write,Edit,Glob,Grep,Bash"),
        )
        .arg("--no-session-persistence")
        .current_dir(opts.repo_dir);

    if let Some(sources) = opts.setting_sources {
        cmd.arg("--setting-sources").arg(sources);
    }

    if let Some(system_prompt) = opts.append_system_prompt {
        cmd.arg("--append-system-prompt").arg(system_prompt);
    }

    let output = cmd
        .output()
        .context("Failed to run 'claude'. Is the Claude CLI installed?")?;

    let duration = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() && stdout.is_empty() {
        anyhow::bail!("Claude CLI failed: {}", stderr);
    }

    let mut parsed = metrics::parse_stream_json(&stdout, duration)?;

    // Override success if CLI exited non-zero
    if !output.status.success() {
        parsed.metrics.success = false;
        if parsed.metrics.error.is_none() {
            parsed.metrics.error = Some(format!(
                "CLI exited with status {}",
                output.status.code().unwrap_or(-1)
            ));
        }
    }

    Ok(RunResult {
        success: parsed.metrics.success,
        response_text: parsed.response_text,
        metrics: parsed.metrics,
    })
}

#[cfg(test)]
mod tests {
    use crate::metrics::parse_stream_json;
    use std::time::Duration;

    fn dur(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn parse_successful_result() {
        let output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Fixed the bug"}]}}
{"type":"result","is_error":false,"result":"Done","total_cost_usd":0.05,"num_turns":3,"usage":{"input_tokens":1000,"output_tokens":500},"duration_ms":5000}"#;
        let parsed = parse_stream_json(output, dur(0)).unwrap();
        assert!(parsed.metrics.success);
        assert_eq!(parsed.response_text, "Fixed the bug");
        assert!((parsed.metrics.cost_usd - 0.05).abs() < f64::EPSILON);
        assert_eq!(parsed.metrics.turns, 3);
    }

    #[test]
    fn parse_error_result() {
        let output = r#"{"type":"result","is_error":true,"subtype":"budget_exceeded","total_cost_usd":5.0,"num_turns":30,"usage":{"input_tokens":100,"output_tokens":50},"duration_ms":10000}"#;
        let parsed = parse_stream_json(output, dur(10000)).unwrap();
        assert!(!parsed.metrics.success);
        assert!((parsed.metrics.cost_usd - 5.0).abs() < f64::EPSILON);
        assert_eq!(parsed.metrics.turns, 30);
    }

    #[test]
    fn parse_empty_output() {
        let parsed = parse_stream_json("", dur(0)).unwrap();
        assert!(!parsed.metrics.success);
        assert_eq!(parsed.metrics.turns, 0);
    }

    #[test]
    fn parse_malformed_lines_skipped() {
        let output = "not json\n{broken\n{\"type\":\"result\",\"is_error\":false,\"total_cost_usd\":0.01,\"num_turns\":1,\"usage\":{\"input_tokens\":10,\"output_tokens\":5},\"duration_ms\":100}";
        let parsed = parse_stream_json(output, dur(100)).unwrap();
        assert!(parsed.metrics.success);
        assert_eq!(parsed.metrics.turns, 1);
    }
}
