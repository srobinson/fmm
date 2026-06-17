use anyhow::Result;
use clap::Args;

use fmm_core::format::{collapse_ws, format_similar};
use fmm_core::similarity::{SimilarMatch, SimilarOptions, find_similar, probe_for};

use super::{load_manifest, warn_no_sidecars};

#[derive(Args)]
pub struct SimilarCommandArgs {
    /// Probe symbol name
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Explicit signature to match (pre-write mode), e.g. '(Path) -> Config'
    #[arg(long)]
    pub signature: Option<String>,

    /// Declaration kind to match (fn, struct, trait, ...)
    #[arg(long)]
    pub kind: Option<String>,

    /// Scope candidates to a directory prefix
    #[arg(long)]
    pub directory: Option<String>,

    /// Maximum matches returned (default: 10)
    #[arg(long)]
    pub limit: Option<usize>,

    /// Include test symbols as candidates
    #[arg(long)]
    pub include_tests: bool,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct SimilarMatchJson {
    name: String,
    file: String,
    lines: [usize; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    score: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn similar(
    name: &str,
    signature: Option<String>,
    kind: Option<String>,
    directory: Option<String>,
    limit: Option<usize>,
    include_tests: bool,
    json_output: bool,
) -> Result<()> {
    let (_root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    let probe = probe_for(&manifest, name, signature, kind);
    let opts = SimilarOptions::from_args(limit, directory, include_tests);
    let matches = find_similar(&manifest, &probe, &opts);

    if json_output {
        let payload: Vec<SimilarMatchJson> = matches.iter().map(to_json).collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("{}", format_similar(name, &matches));
    }
    Ok(())
}

fn to_json(m: &SimilarMatch) -> SimilarMatchJson {
    SimilarMatchJson {
        name: m.name.clone(),
        file: m.file.clone(),
        lines: [m.start_line, m.end_line],
        signature: m.signature.as_deref().map(collapse_ws),
        kind: m.kind.clone(),
        score: (m.score * 100.0).round() / 100.0,
    }
}
