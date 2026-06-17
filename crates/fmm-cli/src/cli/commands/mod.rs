use anyhow::{Context, Result};
use colored::Colorize;

use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;

mod clean;
mod completions;
mod cycles;
mod deps;
mod dupes;
mod exports;
mod generate;
mod glossary;
mod init;
mod lookup;
mod ls;
mod outline;
mod read;
mod search;
mod similar;
mod validate;
mod watch;

pub use clean::CleanCommandArgs;
pub use completions::CompletionsCommandArgs;
pub use cycles::CyclesCommandArgs;
pub use cycles::cycles;
pub use deps::DepsCommandArgs;
pub use deps::deps;
pub use dupes::DupesCommandArgs;
pub use dupes::dupes;
pub use exports::ExportsCommandArgs;
pub use exports::exports;
pub use generate::GenerateCommandArgs;
pub use glossary::GlossaryCommandArgs;
pub use init::InitCommandArgs;
pub use lookup::LookupCommandArgs;
pub use lookup::lookup;
pub use ls::LsCommandArgs;
pub use ls::ls;
pub use outline::OutlineCommandArgs;
pub use outline::outline;
pub use read::ReadCommandArgs;
pub use read::read_symbol;
pub use search::SearchCommandArgs;
pub use similar::SimilarCommandArgs;
pub use similar::similar;
pub use validate::ValidateCommandArgs;
pub use watch::WatchCommandArgs;

fn load_manifest() -> Result<(std::path::PathBuf, Manifest)> {
    let root = std::env::current_dir().context("Failed to get current directory")?;
    let manifest = SqliteStore::open(&root)?.load_manifest()?;
    Ok((root, manifest))
}

fn missing_file_diagnostic(root: &std::path::Path, file: &str) -> String {
    if root.join(file).exists() {
        format!("File exists but is missing from the fmm index: {file}. Run 'fmm generate'.")
    } else {
        format!("File not found in workspace: {file}")
    }
}

fn warn_no_sidecars() {
    println!(
        "{} No fmm index found. Run {} first.",
        "!".yellow(),
        "fmm generate".bold()
    );
}
