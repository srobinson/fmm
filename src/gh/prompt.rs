use crate::gh::github::{Issue, IssueRef};
use crate::gh::references::ResolvedReference;

pub fn build_prompt(
    issue: &Issue,
    issue_ref: &IssueRef,
    resolved: &[ResolvedReference],
    unresolved: &[String],
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "## Task\nFix GitHub issue #{} in {}/{}: {}\n\n",
        issue.number, issue_ref.owner, issue_ref.repo, issue.title
    ));

    prompt.push_str(&format!("## Issue Description\n{}\n\n", issue.body));

    if !resolved.is_empty() {
        prompt.push_str("## Relevant Files (from sidecar analysis)\n");
        for r in resolved {
            prompt.push_str(&format!(
                "### {} (match: {})\n",
                r.file_path, r.match_reason
            ));
            if !r.exports.is_empty() {
                prompt.push_str(&format!("  exports: [{}]\n", r.exports.join(", ")));
            }
            if !r.imports.is_empty() {
                prompt.push_str(&format!("  imports: [{}]\n", r.imports.join(", ")));
            }
            if !r.dependencies.is_empty() {
                prompt.push_str(&format!(
                    "  dependencies: [{}]\n",
                    r.dependencies.join(", ")
                ));
            }
            prompt.push_str(&format!("  loc: {}\n\n", r.loc));
        }
    }

    if !unresolved.is_empty() {
        prompt.push_str("## Unresolved References\n");
        prompt.push_str(
            "These references from the issue could not be matched to files in the codebase:\n",
        );
        for u in unresolved {
            prompt.push_str(&format!("- {}\n", u));
        }
        prompt.push('\n');
    }

    prompt.push_str("## Instructions\n");
    prompt.push_str("The file metadata above (exports, imports, dependencies, LOC) is from sidecar analysis.\n");
    prompt.push_str("Use it to understand the codebase structure WITHOUT reading source files.\n\n");
    prompt.push_str("1. Study the metadata above to understand which files are relevant and how they connect\n");
    prompt.push_str("2. Do NOT read source files just to explore — the metadata already tells you what each file exports and imports\n");
    prompt.push_str("3. ONLY read a source file when you are ready to edit it\n");
    prompt.push_str("4. Make minimal changes to fix the issue\n");
    prompt.push_str("5. Stay consistent with existing code style\n");
    prompt.push_str("6. Do NOT modify unrelated files\n");
    prompt.push_str(
        "7. Run tests if available (look for package.json scripts, Makefile, Cargo.toml, etc.)\n",
    );

    prompt
}

pub fn format_dry_run(
    issue: &Issue,
    resolved: &[ResolvedReference],
    unresolved: &[String],
    prompt: &str,
) -> String {
    let mut out = String::new();

    out.push_str("=== DRY RUN: fmm gh issue ===\n\n");
    out.push_str(&format!("Issue: #{} — {}\n", issue.number, issue.title));
    out.push_str(&format!("State: {}\n\n", issue.state));

    out.push_str(&format!(
        "--- Resolved References ({}) ---\n",
        resolved.len()
    ));
    for r in resolved {
        out.push_str(&format!(
            "  {} ({})\n    exports: {:?}, loc: {}\n",
            r.file_path, r.match_reason, r.exports, r.loc
        ));
    }

    if !unresolved.is_empty() {
        out.push_str(&format!(
            "\n--- Unresolved References ({}) ---\n",
            unresolved.len()
        ));
        for u in unresolved {
            out.push_str(&format!("  {}\n", u));
        }
    }

    out.push_str(&format!(
        "\n--- Assembled Prompt ({} chars) ---\n",
        prompt.len()
    ));
    out.push_str(prompt);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_issue() -> Issue {
        Issue {
            title: "Fix login bug".to_string(),
            body: "Login fails when password has special chars".to_string(),
            labels: vec![],
            state: "OPEN".to_string(),
            number: 42,
            html_url: "https://github.com/owner/repo/issues/42".to_string(),
        }
    }

    fn mock_issue_ref() -> IssueRef {
        IssueRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 42,
            clone_url: "https://github.com/owner/repo.git".to_string(),
        }
    }

    #[test]
    fn prompt_includes_issue_info() {
        let prompt = build_prompt(&mock_issue(), &mock_issue_ref(), &[], &[]);
        assert!(prompt.contains("#42"));
        assert!(prompt.contains("owner/repo"));
        assert!(prompt.contains("Fix login bug"));
        assert!(prompt.contains("Login fails"));
    }

    #[test]
    fn prompt_includes_resolved_metadata() {
        let resolved = vec![ResolvedReference {
            file_path: "src/auth.ts".to_string(),
            exports: vec!["login".to_string()],
            imports: vec!["express".to_string()],
            dependencies: vec!["src/db.ts".to_string()],
            loc: 100,
            match_reason: "exact file path".to_string(),
        }];
        let prompt = build_prompt(&mock_issue(), &mock_issue_ref(), &resolved, &[]);
        assert!(prompt.contains("src/auth.ts"));
        assert!(prompt.contains("exports: [login]"));
        assert!(prompt.contains("imports: [express]"));
        assert!(prompt.contains("loc: 100"));
    }

    #[test]
    fn prompt_excludes_file_contents() {
        let resolved = vec![ResolvedReference {
            file_path: "src/auth.ts".to_string(),
            exports: vec!["login".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 100,
            match_reason: "exact file path".to_string(),
        }];
        let prompt = build_prompt(&mock_issue(), &mock_issue_ref(), &resolved, &[]);
        // Should NOT contain actual source code
        assert!(!prompt.contains("function login("));
        assert!(!prompt.contains("import express"));
    }

    #[test]
    fn prompt_lists_unresolved() {
        let unresolved = vec!["symbol: unknownFn".to_string()];
        let prompt = build_prompt(&mock_issue(), &mock_issue_ref(), &[], &unresolved);
        assert!(prompt.contains("Unresolved References"));
        assert!(prompt.contains("symbol: unknownFn"));
    }

    #[test]
    fn prompt_has_instructions() {
        let prompt = build_prompt(&mock_issue(), &mock_issue_ref(), &[], &[]);
        assert!(prompt.contains("Instructions"));
        assert!(prompt.contains("ONLY read a source file when you are ready to edit it"));
        assert!(prompt.contains("minimal changes"));
    }

    #[test]
    fn dry_run_output_complete() {
        let resolved = vec![ResolvedReference {
            file_path: "src/main.rs".to_string(),
            exports: vec!["main".to_string()],
            imports: vec![],
            dependencies: vec![],
            loc: 50,
            match_reason: "entry point".to_string(),
        }];
        let unresolved = vec!["symbol: missing".to_string()];
        let prompt = build_prompt(&mock_issue(), &mock_issue_ref(), &resolved, &unresolved);
        let output = format_dry_run(&mock_issue(), &resolved, &unresolved, &prompt);

        assert!(output.contains("DRY RUN"));
        assert!(output.contains("#42"));
        assert!(output.contains("Resolved References (1)"));
        assert!(output.contains("Unresolved References (1)"));
        assert!(output.contains("Assembled Prompt"));
    }
}
