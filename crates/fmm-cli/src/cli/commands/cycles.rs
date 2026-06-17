use anyhow::Result;
use clap::Args;
use colored::Colorize;

use super::{load_manifest, missing_file_diagnostic, warn_no_sidecars};

#[derive(Args)]
pub struct CyclesCommandArgs {
    /// Optional source file path to scope cycle reports
    #[arg(value_name = "FILE")]
    pub file: Option<String>,

    /// Filter cycle graph by file type: all (default), source (exclude tests), tests (only tests)
    #[arg(long, default_value = "all", value_parser = ["all", "source", "tests"])]
    pub filter: String,

    /// Edge mode: runtime (default, excludes type-only edges) or all
    #[arg(long = "edge-mode", default_value = "runtime", value_parser = ["runtime", "all"])]
    pub edge_mode: String,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct CyclesJson {
    cycles: Vec<Vec<String>>,
}

pub fn cycles(file: Option<&str>, filter: &str, edge_mode: &str, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if let Some(file) = file {
        if file.ends_with('/') || root.join(file).is_dir() {
            anyhow::bail!(
                "'{}' is a directory. Use {} to list files.",
                file,
                format!("fmm ls {}", file).bold()
            );
        }
        if !manifest.files.contains_key(file) {
            anyhow::bail!(missing_file_diagnostic(&root, file));
        }
    }

    let edge_mode =
        crate::cycle_report::parse_edge_mode(Some(edge_mode)).map_err(anyhow::Error::msg)?;
    let file_filter =
        crate::cycle_report::CycleFileFilter::parse(filter).map_err(anyhow::Error::msg)?;
    let config = fmm_core::config::Config::load_from_dir(&root).unwrap_or_default();
    let cycles =
        fmm_core::search::dependency_cycles_with_path_filter(&manifest, file, edge_mode, |path| {
            file_filter.keeps(path, |candidate| config.is_test_file(candidate))
        })?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&CyclesJson { cycles })?);
    } else {
        println!("{}", fmm_core::format::format_dependency_cycles(&cycles));
    }

    Ok(())
}
