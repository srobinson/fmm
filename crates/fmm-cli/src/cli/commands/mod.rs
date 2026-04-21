use anyhow::{Context, Result};
use colored::Colorize;

use fmm_core::manifest::Manifest;
use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;

mod deps;
mod exports;
mod lookup;
mod ls;
mod outline;
mod read;

pub use deps::deps;
pub use exports::exports;
pub use lookup::lookup;
pub use ls::ls;
pub use outline::outline;
pub use read::read_symbol;

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
