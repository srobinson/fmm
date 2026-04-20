#[path = "cli_output_parity/cases.rs"]
mod cases;
#[path = "cli_output_parity/normalize.rs"]
mod normalize;
#[path = "cli_output_parity/parse.rs"]
mod parse;
#[path = "cli_output_parity/support.rs"]
mod support;

use cases::{mcp_parity_cases, parity_cases};
use normalize::{normalize_json, normalize_text};
use support::{call_mcp_text, ensure_repo_index, repo_root, run_fmm};

#[derive(Clone, Copy)]
enum Projection {
    Deps,
    ExportsFile,
    ExportsPattern,
    Glossary,
    Lookup,
    Ls,
    Outline,
    Read,
    SearchBare,
    SearchExport,
    SearchFilter,
}

#[test]
fn cli_text_and_json_outputs_have_semantic_parity() {
    ensure_repo_index();

    for case in parity_cases() {
        let text_output = run_fmm(case.args);
        let mut json_args = case.args.to_vec();
        json_args.push("--json");
        let json_output = run_fmm(&json_args);

        let text = normalize_text(case.projection, &text_output.stdout);
        let json = normalize_json(case.projection, &json_output.stdout);

        assert_eq!(
            text,
            json,
            "{} text/json parity mismatch\ntext:\n{}\njson:\n{}",
            case.name,
            String::from_utf8_lossy(&text_output.stdout),
            String::from_utf8_lossy(&json_output.stdout)
        );
    }
}

#[test]
fn mcp_text_and_cli_json_outputs_have_semantic_parity() {
    ensure_repo_index();
    let server = fmm::mcp::SqliteMcpServer::with_root(repo_root());

    for case in mcp_parity_cases() {
        let mcp_text = call_mcp_text(&server, case.tool, case.arguments);
        let mut json_args = case.cli_args.to_vec();
        json_args.push("--json");
        let json_output = run_fmm(&json_args);

        let mcp = normalize_text(case.projection, mcp_text.as_bytes());
        let json = normalize_json(case.projection, &json_output.stdout);

        assert_eq!(
            mcp,
            json,
            "{} MCP/CLI JSON parity mismatch\nmcp:\n{}\njson:\n{}",
            case.name,
            mcp_text,
            String::from_utf8_lossy(&json_output.stdout)
        );
    }
}
