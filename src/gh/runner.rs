use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct RunResult {
    pub success: bool,
    pub response_text: String,
    pub cost_usd: f64,
    pub turns: u32,
}

pub fn invoke_claude(
    prompt: &str,
    repo_dir: &Path,
    model: &str,
    max_turns: u32,
    max_budget: f64,
) -> Result<RunResult> {
    let mut cmd = Command::new("claude");

    cmd.arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--max-turns")
        .arg(max_turns.to_string())
        .arg("--max-budget-usd")
        .arg(max_budget.to_string())
        .arg("--model")
        .arg(model)
        .arg("--allowedTools")
        .arg("Read,Write,Edit,Glob,Grep,Bash")
        .arg("--no-session-persistence")
        .current_dir(repo_dir);

    let output = cmd
        .output()
        .context("Failed to run 'claude'. Is the Claude CLI installed?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() && stdout.is_empty() {
        anyhow::bail!("Claude CLI failed: {}", stderr);
    }

    parse_stream_json(&stdout)
}

fn parse_stream_json(output: &str) -> Result<RunResult> {
    let mut response_text = String::new();
    let mut cost_usd: f64 = 0.0;
    let mut turns: u32 = 0;
    let mut success = false;

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
                if let Some(message) = data.get("message") {
                    if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                        for item in content {
                            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    response_text = text.to_string();
                                }
                            }
                        }
                    }
                }
            }
            Some("result") => {
                cost_usd = data
                    .get("total_cost_usd")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                turns = data.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                let is_error = data
                    .get("is_error")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(true);
                success = !is_error;

                if let Some(result_text) = data.get("result").and_then(|r| r.as_str()) {
                    if response_text.is_empty() {
                        response_text = result_text.to_string();
                    }
                }
            }
            _ => {}
        }
    }

    Ok(RunResult {
        success,
        response_text,
        cost_usd,
        turns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_successful_result() {
        let output = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Fixed the bug"}]}}
{"type":"result","is_error":false,"result":"Done","total_cost_usd":0.05,"num_turns":3}"#;
        let result = parse_stream_json(output).unwrap();
        assert!(result.success);
        assert_eq!(result.response_text, "Fixed the bug");
        assert!((result.cost_usd - 0.05).abs() < f64::EPSILON);
        assert_eq!(result.turns, 3);
    }

    #[test]
    fn parse_error_result() {
        let output = r#"{"type":"result","is_error":true,"subtype":"budget_exceeded","total_cost_usd":5.0,"num_turns":30}"#;
        let result = parse_stream_json(output).unwrap();
        assert!(!result.success);
        assert!((result.cost_usd - 5.0).abs() < f64::EPSILON);
        assert_eq!(result.turns, 30);
    }

    #[test]
    fn parse_empty_output() {
        let result = parse_stream_json("").unwrap();
        assert!(!result.success);
        assert_eq!(result.turns, 0);
    }

    #[test]
    fn parse_malformed_lines_skipped() {
        let output = "not json\n{broken\n{\"type\":\"result\",\"is_error\":false,\"total_cost_usd\":0.01,\"num_turns\":1}";
        let result = parse_stream_json(output).unwrap();
        assert!(result.success);
        assert_eq!(result.turns, 1);
    }
}
