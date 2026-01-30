//! FMM Comparison API - Automated benchmarking against GitHub repositories
//!
//! This module provides:
//! - `fmm compare <url>` CLI command
//! - Repo cloning and sandbox isolation
//! - Claude CLI runner with instrumentation
//! - Standard benchmark task definitions
//! - JSON and Markdown report generation
//! - Result caching layer
//! - Cost management and budgets

mod cache;
mod orchestrator;
mod report;
mod runner;
mod sandbox;
mod tasks;

pub use orchestrator::{CompareOptions, Orchestrator};
pub use report::ReportFormat;

use anyhow::Result;
use colored::Colorize;

/// Main entry point for the compare command
pub fn compare(url: &str, options: CompareOptions) -> Result<()> {
    println!(
        "{} Starting comparison for {}",
        "⚡".yellow(),
        url.cyan().bold()
    );

    let mut orchestrator = Orchestrator::new(options)?;
    let report = orchestrator.run(url)?;

    // Display summary
    println!("\n{}", "═".repeat(60).dimmed());
    println!("{}", "COMPARISON RESULTS".green().bold());
    println!("{}", "═".repeat(60).dimmed());

    report.print_summary();

    Ok(())
}
