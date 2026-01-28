use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config::Config;
use crate::extractor::FileProcessor;
use crate::manifest::Manifest;

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

        /// Only generate manifest, skip inline frontmatter
        #[arg(long)]
        manifest_only: bool,
    },

    /// Update existing frontmatter in all files
    Update {
        /// Path to file or directory
        #[arg(default_value = ".")]
        path: String,

        /// Dry run - show what would be changed
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Only update manifest, skip inline frontmatter
        #[arg(long)]
        manifest_only: bool,
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

pub fn generate(path: &str, dry_run: bool, manifest_only: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = std::env::current_dir()?;

    println!("Found {} files to process", files.len());

    // Load or create manifest
    let manifest = Mutex::new(Manifest::load(&root)?.unwrap_or_default());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config);

            // Extract metadata for manifest
            if let Ok(Some(metadata)) = processor.extract_metadata(file) {
                let relative_path = file
                    .strip_prefix(&root)
                    .unwrap_or(file)
                    .display()
                    .to_string();

                // Add to manifest
                if let Ok(mut m) = manifest.lock() {
                    m.add_file(&relative_path, metadata);
                }
            }

            // Skip inline frontmatter if manifest_only
            if manifest_only {
                return Some((file.to_path_buf(), "Added to manifest".to_string()));
            }

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

    // Save manifest
    if !dry_run {
        let mut m = manifest.lock().unwrap();
        m.touch();
        m.save(&root)?;
        println!(
            "{} Saved manifest with {} files",
            "✓".green(),
            m.file_count()
        );
    }

    for (file, msg) in results {
        println!("{} {}", "✓".green(), file.display());
        if dry_run {
            println!("  {}", msg.dimmed());
        }
    }

    Ok(())
}

pub fn update(path: &str, dry_run: bool, manifest_only: bool) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let files = collect_files(path, &config)?;
    let root = std::env::current_dir()?;

    println!("Found {} files to process", files.len());

    // Create fresh manifest (update rebuilds from scratch)
    let manifest = Mutex::new(Manifest::new());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config);

            // Extract metadata for manifest
            if let Ok(Some(metadata)) = processor.extract_metadata(file) {
                let relative_path = file
                    .strip_prefix(&root)
                    .unwrap_or(file)
                    .display()
                    .to_string();

                // Add to manifest
                if let Ok(mut m) = manifest.lock() {
                    m.add_file(&relative_path, metadata);
                }
            }

            // Skip inline frontmatter if manifest_only
            if manifest_only {
                return Some((file.to_path_buf(), "Updated in manifest".to_string()));
            }

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

    // Save manifest
    if !dry_run {
        let mut m = manifest.lock().unwrap();
        m.touch();
        m.save(&root)?;
        println!(
            "{} Saved manifest with {} files",
            "✓".green(),
            m.file_count()
        );
    }

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
    let root = std::env::current_dir()?;

    println!("Validating {} files...", files.len());

    // Load manifest for validation
    let manifest = Manifest::load(&root)?;
    let has_manifest = manifest.is_some();

    let invalid: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let processor = FileProcessor::new(&config);

            // Validate inline frontmatter
            let inline_valid = match processor.validate(file) {
                Ok(valid) => valid,
                Err(e) => {
                    return Some((file.to_path_buf(), format!("Error: {}", e)));
                }
            };

            // Validate manifest entry
            let manifest_valid = if let Some(ref m) = manifest {
                let relative_path = file
                    .strip_prefix(&root)
                    .unwrap_or(file)
                    .display()
                    .to_string();

                if let Ok(Some(current_metadata)) = processor.extract_metadata(file) {
                    m.validate_file(&relative_path, &current_metadata)
                } else {
                    false
                }
            } else {
                true // No manifest to validate against
            };

            if inline_valid && manifest_valid {
                None
            } else {
                let mut reasons = Vec::new();
                if !inline_valid {
                    reasons.push("inline frontmatter out of date");
                }
                if has_manifest && !manifest_valid {
                    reasons.push("manifest out of date");
                }
                Some((file.to_path_buf(), reasons.join(", ")))
            }
        })
        .collect();

    // Check if manifest exists
    let manifest_missing = has_manifest.then_some(()).is_none() && !files.is_empty();

    if invalid.is_empty() && !manifest_missing {
        println!("{} All frontmatter is up to date!", "✓".green().bold());
        if has_manifest {
            println!("{} Manifest is in sync", "✓".green());
        }
        Ok(())
    } else {
        if manifest_missing {
            println!(
                "{} Manifest file missing (.fmm/index.json)",
                "✗".red().bold()
            );
        }
        if !invalid.is_empty() {
            println!(
                "{} {} files need updating:",
                "✗".red().bold(),
                invalid.len()
            );
            for (file, msg) in &invalid {
                println!("  {} {}: {}", "✗".red(), file.display(), msg.dimmed());
            }
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
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
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
