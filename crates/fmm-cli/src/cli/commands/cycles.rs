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

    /// Include module-hierarchy facade edges
    #[arg(long = "include-mod-hierarchy")]
    pub include_mod_hierarchy: bool,

    /// Include edges that keep each SCC connected
    #[arg(long = "explain", alias = "edges")]
    pub explain: bool,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct CyclesJson {
    cycles: Vec<CycleJson>,
}

#[derive(serde::Serialize)]
struct CycleJson {
    files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edges: Option<Vec<CycleEdgeJson>>,
}

#[derive(serde::Serialize)]
struct CycleEdgeJson {
    source: String,
    target: String,
    kind: &'static str,
}

pub fn cycles(
    file: Option<&str>,
    filter: &str,
    edge_mode: &str,
    include_mod_hierarchy: bool,
    explain: bool,
    json_output: bool,
) -> Result<()> {
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
    let options =
        fmm_core::search::CycleOptions::new(edge_mode).include_mod_hierarchy(include_mod_hierarchy);
    let config = fmm_core::config::Config::load_from_dir(&root).unwrap_or_default();
    let cycles = fmm_core::search::dependency_cycle_reports_with_path_filter(
        &manifest,
        file,
        options,
        |path| file_filter.keeps(path, |candidate| config.is_test_file(candidate)),
    )?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&CyclesJson {
                cycles: cycles
                    .iter()
                    .map(|cycle| CycleJson {
                        files: cycle.files.clone(),
                        edges: explain.then(|| {
                            cycle
                                .edges
                                .iter()
                                .map(|edge| CycleEdgeJson {
                                    source: edge.source.clone(),
                                    target: edge.target.clone(),
                                    kind: edge.kind.as_str(),
                                })
                                .collect()
                        }),
                    })
                    .collect(),
            })?
        );
    } else {
        println!(
            "{}",
            fmm_core::format::format_dependency_cycle_reports(&cycles, explain)
        );
    }

    Ok(())
}
