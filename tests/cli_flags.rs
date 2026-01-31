//! Tests for CLI flags: completions, --markdown-help, --generate-man-pages, --no-generate.
//!
//! These verify that clap accepts the flags and that the corresponding
//! code paths produce meaningful output or side effects.

use clap::{CommandFactory, Parser};
use fmm::cli::{Cli, Commands};
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
fn init_no_generate_combined_with_other_flags() {
    let cli = Cli::parse_from(["fmm", "init", "--skill", "--no-generate"]);
    match cli.command {
        Some(Commands::Init {
            skill, no_generate, ..
        }) => {
            assert!(skill);
            assert!(no_generate);
        }
        other => panic!("expected Init command, got {:?}", other.is_some()),
    }
}
