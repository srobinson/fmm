use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::extractor::FileProcessor;

#[derive(Parser)]
#[command(
    name = "fmm",
    about = "Frontmatter Matters - Auto-generate code frontmatter",
    version,
    author
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate frontmatter for files that don't have it
    Generate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Dry run - show what would be changed
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Update existing frontmatter in all files
    Update {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Dry run - show what would be changed
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Validate that frontmatter is up to date
    Validate {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Initialize .fmmrc.json configuration file
    Init,
}

pub fn generate(path: &str, dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;

    println!("Found {} files to process", files.len());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config);
            match processor.generate(file, dry_run) {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("{} {}: {}", "Error".red(), file.display(), e);
                    None
                }
            }
        })
        .collect();

    for (file, msg) in results {
        println!("{} {}", "✓".green(), file.display());
        if dry_run {
            println!("  {}", msg.dimmed());
        }
    }

    Ok(())
}

pub fn update(path: &str, dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;

    println!("Found {} files to process", files.len());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config);
            match processor.update(file, dry_run) {
                Ok(Some(msg)) => Some((file.to_path_buf(), msg)),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("{} {}: {}", "Error".red(), file.display(), e);
                    None
                }
            }
        })
        .collect();

    for (file, msg) in results {
        println!("{} {}", "✓".green(), file.display());
        if dry_run {
            println!("  {}", msg.dimmed());
        }
    }

    Ok(())
}

pub fn validate(path: &str) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;

    println!("Validating {} files...", files.len());

    let invalid: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config);
            match processor.validate(file) {
                Ok(true) => None,
                Ok(false) => Some((file.to_path_buf(), "Out of date".to_string())),
                Err(e) => Some((file.to_path_buf(), e.to_string())),
            }
        })
        .collect();

    if invalid.is_empty() {
        println!("{} All frontmatter is up to date!", "✓".green().bold());
        Ok(())
    } else {
        println!("{} {} files need updating:", "✗".red().bold(), invalid.len());
        for (file, msg) in &invalid {
            println!("  {} {}: {}", "✗".red(), file.display(), msg.dimmed());
        }
        anyhow::bail!("Frontmatter validation failed");
    }
}

pub fn init() -> Result<()> {
    let config_path = Path::new(".fmmrc.json");
    if config_path.exists() {
        anyhow::bail!(".fmmrc.json already exists");
    }

    let default_config = Config::default();
    let json = serde_json::to_string_pretty(&default_config)?;
    std::fs::write(config_path, json).context("Failed to write .fmmrc.json")?;

    println!(
        "{} Created .fmmrc.json with default configuration",
        "✓".green()
    );
    Ok(())
}

fn collect_files(path: &str, config: &Config) -> Result<Vec<PathBuf>> {
    let path = Path::new(path);

    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let walker = WalkBuilder::new(path)
        .standard_filters(true)
        .add_custom_ignore_filename(".fmmignore")
        .build();

    let files: Vec<PathBuf> = walker
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_file()))
        .filter(|entry| {
            if let Some(ext) = entry.path().extension() {
                config.is_supported_language(ext.to_str().unwrap_or(""))
            } else {
                false
            }
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    Ok(files)
}
