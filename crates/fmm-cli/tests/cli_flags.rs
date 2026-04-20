//! Tests for CLI flags: completions, --markdown-help, --generate-man-pages, --no-generate.
//!
//! These verify that clap accepts the flags and that the corresponding
//! code paths produce meaningful output or side effects.

use clap::{CommandFactory, Parser};
use fmm::cli::{Cli, Commands};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use tempfile::TempDir;

// ── Shell completions ────────────────────────────────────────────────

#[test]
fn completions_bash_generates_output() {
    let mut buf = Vec::new();
    clap_complete::generate(
        clap_complete::Shell::Bash,
        &mut Cli::command(),
        "fmm",
        &mut buf,
    );
    let output = String::from_utf8(buf).unwrap();
    assert!(!output.is_empty(), "bash completions should produce output");
    assert!(
        output.contains("fmm"),
        "completions should reference the binary name"
    );
}

#[test]
fn completions_zsh_generates_output() {
    let mut buf = Vec::new();
    clap_complete::generate(
        clap_complete::Shell::Zsh,
        &mut Cli::command(),
        "fmm",
        &mut buf,
    );
    let output = String::from_utf8(buf).unwrap();
    assert!(!output.is_empty(), "zsh completions should produce output");
    assert!(output.contains("fmm"));
}

#[test]
fn completions_fish_generates_output() {
    let mut buf = Vec::new();
    clap_complete::generate(
        clap_complete::Shell::Fish,
        &mut Cli::command(),
        "fmm",
        &mut buf,
    );
    let output = String::from_utf8(buf).unwrap();
    assert!(!output.is_empty(), "fish completions should produce output");
    assert!(output.contains("fmm"));
}

#[test]
fn completions_powershell_generates_output() {
    let mut buf = Vec::new();
    clap_complete::generate(
        clap_complete::Shell::PowerShell,
        &mut Cli::command(),
        "fmm",
        &mut buf,
    );
    let output = String::from_utf8(buf).unwrap();
    assert!(
        !output.is_empty(),
        "powershell completions should produce output"
    );
    assert!(output.contains("fmm"));
}

#[test]
fn completions_subcommand_parses() {
    let cli = Cli::parse_from(["fmm", "completions", "bash"]);
    assert!(matches!(
        cli.command,
        Some(Commands::Completions {
            shell: clap_complete::Shell::Bash
        })
    ));
}

// ── --markdown-help ──────────────────────────────────────────────────

#[test]
fn markdown_help_flag_is_recognized() {
    let cli = Cli::parse_from(["fmm", "--markdown-help"]);
    assert!(cli.markdown_help);
}

#[test]
fn markdown_help_produces_markdown_output() {
    let markdown = clap_markdown::help_markdown::<Cli>();
    assert!(
        markdown.contains('#'),
        "markdown output should contain headings"
    );
    assert!(
        markdown.contains("fmm"),
        "markdown output should reference the binary"
    );
    assert!(
        markdown.contains("generate"),
        "markdown output should document subcommands"
    );
}

#[test]
fn top_level_long_help_documents_navigation_workflows() {
    let help = Cli::command().render_long_help().to_string();

    for expected in [
        "Frontmatter Matters: Structural intelligence for codebases",
        "fmm ls --sort-by downstream --limit 20",
        "fmm outline src/injector.ts --include-private",
        "fmm read Injector.loadInstance --line-numbers",
        "fmm deps src/injector.ts --depth 2 --filter source",
        "fmm exports --file src/app.ts",
        "fmm search --imports react --min-loc 100",
        "fmm glossary Injector.loadInstance --precision call-site",
        "fmm glossary Config --mode tests --no-truncate",
        "fmm mcp",
        "fmm clean",
    ] {
        assert!(
            help.contains(expected),
            "top-level long help should include {expected:?}; got:\n{help}"
        );
    }
}

// ── --generate-man-pages ─────────────────────────────────────────────

#[test]
fn generate_man_pages_flag_is_recognized() {
    let cli = Cli::parse_from(["fmm", "--generate-man-pages", "/tmp/man-test"]);
    assert_eq!(
        cli.generate_man_pages,
        Some(std::path::PathBuf::from("/tmp/man-test"))
    );
}

#[test]
fn generate_man_pages_creates_files() {
    let tmp = TempDir::new().unwrap();
    let out_dir = tmp.path().join("man");
    fs::create_dir_all(&out_dir).unwrap();

    let cmd = Cli::command();
    clap_mangen::generate_to(cmd, &out_dir).unwrap();

    let man_files: Vec<_> = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "1"))
        .collect();

    assert!(
        !man_files.is_empty(),
        "man page generation should create at least one .1 file"
    );

    // The top-level man page should exist
    let has_fmm_page = man_files
        .iter()
        .any(|f| f.file_name().to_string_lossy().starts_with("fmm"));
    assert!(has_fmm_page, "should generate a man page for fmm itself");
}

// ── --no-generate on init ────────────────────────────────────────────

#[test]
fn init_no_generate_flag_is_recognized() {
    let cli = Cli::parse_from(["fmm", "init", "--no-generate"]);
    match cli.command {
        Some(Commands::Init { no_generate, .. }) => {
            assert!(no_generate, "--no-generate should be true");
        }
        other => panic!("expected Init command, got {:?}", other.is_some()),
    }
}

#[test]
fn init_without_no_generate_defaults_to_false() {
    let cli = Cli::parse_from(["fmm", "init"]);
    match cli.command {
        Some(Commands::Init { no_generate, .. }) => {
            assert!(!no_generate, "--no-generate should default to false");
        }
        other => panic!("expected Init command, got {:?}", other.is_some()),
    }
}

#[test]
fn init_removed_flags_are_rejected() {
    for flag in &["--skill", "--mcp", "--all"] {
        let result = Cli::try_parse_from(["fmm", "init", flag]);
        assert!(result.is_err(), "{} should be rejected", flag);
    }
}

// -- MCP parity surface ------------------------------------------------------

#[derive(Deserialize)]
struct ToolsToml {
    tools: BTreeMap<String, ToolDoc>,
}

#[derive(Deserialize)]
struct ToolDoc {
    cli_name: String,
    #[serde(default)]
    params: Vec<ParamDoc>,
}

#[derive(Deserialize)]
struct ParamDoc {
    #[serde(default)]
    cli_flag: Option<String>,
}

#[test]
fn tools_toml_cli_flags_are_exposed_by_clap_commands() {
    let tools: ToolsToml =
        toml::from_str(include_str!("../tools.toml")).expect("tools.toml should parse");
    let command = Cli::command();

    for (tool_name, tool) in tools.tools {
        let subcommand = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == tool.cli_name)
            .unwrap_or_else(|| {
                panic!(
                    "{tool_name} declares cli_name {:?}, but Commands does not expose it",
                    tool.cli_name
                )
            });

        let rendered_help = subcommand.clone().render_long_help().to_string();

        for param in tool.params {
            let Some(cli_flag) = param.cli_flag else {
                continue;
            };

            if let Some(long_flag) = cli_flag.strip_prefix("--") {
                let has_long_arg = subcommand
                    .get_arguments()
                    .any(|arg| arg.get_long() == Some(long_flag));
                assert!(
                    has_long_arg,
                    "{tool_name} declares {cli_flag}, but fmm {} does not expose it",
                    tool.cli_name
                );
                assert!(
                    rendered_help.contains(&cli_flag),
                    "fmm {} help should document {cli_flag}; got:\n{rendered_help}",
                    tool.cli_name
                );
            } else {
                let parse_result =
                    Cli::try_parse_from(["fmm", tool.cli_name.as_str(), "__fmm_contract_value__"]);
                assert!(
                    parse_result.is_ok(),
                    "{tool_name} declares positional cli_flag {:?}, but fmm {} rejected a positional value: {:?}",
                    cli_flag,
                    tool.cli_name,
                    parse_result.err()
                );
            }
        }
    }
}
