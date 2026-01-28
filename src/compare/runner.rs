//! Claude CLI runner with instrumentation for benchmarking

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use super::tasks::Task;

/// Result of a single benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    /// Task ID
    pub task_id: String,
    /// Variant (control or fmm)
    pub variant: String,
    /// Total tool calls made
    pub tool_calls: u32,
    /// Breakdown of tool calls by name
    pub tools_by_name: HashMap<String, u32>,
    /// Files accessed via Read tool
    pub files_accessed: Vec<String>,
    /// Number of read calls
    pub read_calls: u32,
    /// Input tokens used
    pub input_tokens: u64,
    /// Output tokens used
    pub output_tokens: u64,
    /// Cache read tokens
    pub cache_read_tokens: u64,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of turns
    pub num_turns: u32,
    /// Final response text
    pub response: String,
    /// Whether the run was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Metrics collected during a run
#[derive(Debug, Default)]
pub struct RunMetrics {
    pub tool_calls: u32,
    pub tools_by_name: HashMap<String, u32>,
    pub files_accessed: Vec<String>,
    pub read_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub duration_ms: u64,
    pub num_turns: u32,
}

/// Claude CLI runner with instrumentation
pub struct ClaudeRunner {
    /// Allowed tools (empty = all)
    allowed_tools: Vec<String>,
    /// Model to use
    model: String,
    /// Whether to skip permissions (for sandboxed environments)
    skip_permissions: bool,
}

impl Default for ClaudeRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeRunner {
    pub fn new() -> Self {
        Self {
            allowed_tools: vec![
                "Read".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
                "LS".to_string(),
            ],
            model: "sonnet".to_string(),
            skip_permissions: false,
        }
    }

    /// Set allowed tools
    #[allow(dead_code)]
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    /// Set model
    #[allow(dead_code)]
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// Enable skipping permissions (for sandboxed environments)
    #[allow(dead_code)]
    pub fn skip_permissions(mut self, skip: bool) -> Self {
        self.skip_permissions = skip;
        self
    }

    /// Run a task and collect metrics
    pub fn run_task(
        &self,
        task: &Task,
        working_dir: &Path,
        variant: &str,
        fmm_context: Option<&str>,
    ) -> Result<RunResult> {
        let start = Instant::now();

        let mut cmd = Command::new("claude");

        // Print mode (non-interactive)
        cmd.arg("-p").arg(&task.prompt);

        // Output format for parsing
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("--verbose");

        // Limits
        cmd.arg("--max-turns").arg(task.max_turns.to_string());
        cmd.arg("--max-budget-usd")
            .arg(task.max_budget_usd.to_string());

        // Model
        cmd.arg("--model").arg(&self.model);

        // Tools
        if !self.allowed_tools.is_empty() {
            cmd.arg("--allowedTools").arg(self.allowed_tools.join(","));
        }

        // FMM context injection via append-system-prompt
        if let Some(context) = fmm_context {
            cmd.arg("--append-system-prompt").arg(context);
        }

        // Skip permissions if in sandbox
        if self.skip_permissions {
            cmd.arg("--dangerously-skip-permissions");
        }

        // Session handling
        cmd.arg("--no-session-persistence");

        // Working directory
        cmd.current_dir(working_dir);

        // Execute
        let output = cmd.output().context("Failed to execute claude CLI")?;

        let duration = start.elapsed();

        // Parse the output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() && stdout.is_empty() {
            return Ok(RunResult {
                task_id: task.id.clone(),
                variant: variant.to_string(),
                tool_calls: 0,
                tools_by_name: HashMap::new(),
                files_accessed: vec![],
                read_calls: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                total_cost_usd: 0.0,
                duration_ms: duration.as_millis() as u64,
                num_turns: 0,
                response: String::new(),
                success: false,
                error: Some(stderr.to_string()),
            });
        }

        // Parse stream-json output
        self.parse_stream_json(&stdout, &task.id, variant, duration)
    }

    fn parse_stream_json(
        &self,
        output: &str,
        task_id: &str,
        variant: &str,
        duration: Duration,
    ) -> Result<RunResult> {
        let mut metrics = RunMetrics::default();
        let mut response_text = String::new();
        let mut final_result: Option<serde_json::Value> = None;

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let data: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            match data.get("type").and_then(|v| v.as_str()) {
                Some("assistant") => {
                    // Parse tool calls from assistant message
                    if let Some(message) = data.get("message") {
                        if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                            for item in content {
                                if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                    metrics.tool_calls += 1;

                                    if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                                        *metrics
                                            .tools_by_name
                                            .entry(name.to_string())
                                            .or_insert(0) += 1;

                                        // Track Read calls specifically
                                        if name == "Read" || name == "View" {
                                            metrics.read_calls += 1;
                                            if let Some(input) = item.get("input") {
                                                if let Some(path) = input
                                                    .get("file_path")
                                                    .or(input.get("path"))
                                                    .and_then(|p| p.as_str())
                                                {
                                                    metrics.files_accessed.push(path.to_string());
                                                }
                                            }
                                        }
                                    }
                                } else if item.get("type").and_then(|t| t.as_str()) == Some("text")
                                {
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                        response_text = text.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
                Some("result") => {
                    final_result = Some(data.clone());

                    // Extract metrics from result
                    if let Some(usage) = data.get("usage") {
                        metrics.input_tokens = usage
                            .get("input_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        metrics.output_tokens = usage
                            .get("output_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        metrics.cache_read_tokens = usage
                            .get("cache_read_input_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                    }

                    metrics.total_cost_usd = data
                        .get("total_cost_usd")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    metrics.num_turns =
                        data.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    metrics.duration_ms = data
                        .get("duration_ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(duration.as_millis() as u64);

                    if let Some(result_text) = data.get("result").and_then(|r| r.as_str()) {
                        if response_text.is_empty() {
                            response_text = result_text.to_string();
                        }
                    }
                }
                _ => {}
            }
        }

        let success = final_result
            .as_ref()
            .and_then(|r| r.get("is_error"))
            .and_then(|e| e.as_bool())
            .map(|e| !e)
            .unwrap_or(false);

        let error = if !success {
            final_result
                .as_ref()
                .and_then(|r| r.get("subtype"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        Ok(RunResult {
            task_id: task_id.to_string(),
            variant: variant.to_string(),
            tool_calls: metrics.tool_calls,
            tools_by_name: metrics.tools_by_name,
            files_accessed: metrics.files_accessed,
            read_calls: metrics.read_calls,
            input_tokens: metrics.input_tokens,
            output_tokens: metrics.output_tokens,
            cache_read_tokens: metrics.cache_read_tokens,
            total_cost_usd: metrics.total_cost_usd,
            duration_ms: metrics.duration_ms,
            num_turns: metrics.num_turns,
            response: response_text,
            success,
            error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_creation() {
        let runner = ClaudeRunner::new();
        assert!(!runner.allowed_tools.is_empty());
    }
}
