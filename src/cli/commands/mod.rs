use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::Manifest;

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
    let manifest = Manifest::load(&root)?;
    Ok((root, manifest))
}

fn warn_no_sidecars() {
    println!(
        "{} No .fmm sidecars found. Run {} first.",
        "!".yellow(),
        "fmm generate".bold()
    );
}
