use anyhow::Result;
use colored::Colorize;

use super::{load_manifest, missing_file_diagnostic, warn_no_sidecars};

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
    let config = fmm_core::config::Config::load_from_dir(&root).unwrap_or_default();
    let cycles = fmm_core::search::dependency_cycles(&manifest, file, edge_mode)?;
    let cycles =
        crate::cycle_report::filter_cycles(cycles, filter, |path| config.is_test_file(path));

    if json_output {
        println!("{}", serde_json::to_string_pretty(&CyclesJson { cycles })?);
    } else {
        println!("{}", fmm_core::format::format_dependency_cycles(&cycles));
    }

    Ok(())
}
