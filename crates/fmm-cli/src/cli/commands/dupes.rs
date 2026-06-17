use anyhow::Result;
use clap::Args;
use fmm_core::dupes::{DupeOptions, find_dupe_clusters};
use fmm_core::format::search_formatters::format_dupe_clusters;

use super::{load_manifest, warn_no_sidecars};

#[derive(Args, Debug)]
pub struct DupesCommandArgs {
    /// Scope candidates to a directory prefix
    #[arg(long = "dir")]
    pub directory: Option<String>,

    /// Restrict candidates to a declaration kind
    #[arg(long = "kind")]
    pub kind: Vec<String>,

    /// Minimum pair score required to join a cluster
    #[arg(long = "min-score")]
    pub min_score: Option<f64>,

    /// Maximum clusters returned
    #[arg(long)]
    pub limit: Option<usize>,

    /// Include test symbols as candidates
    #[arg(long)]
    pub include_tests: bool,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}

pub fn dupes(
    directory: Option<String>,
    kinds: Vec<String>,
    min_score: Option<f64>,
    limit: Option<usize>,
    include_tests: bool,
    json_output: bool,
) -> Result<()> {
    let (_root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    let opts = DupeOptions::from_args(directory, kinds, min_score, limit, include_tests);
    let result = find_dupe_clusters(&manifest, &opts);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", format_dupe_clusters(&result));
    }

    Ok(())
}
