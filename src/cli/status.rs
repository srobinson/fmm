use anyhow::Result;
use colored::Colorize;

use crate::config::Config;
use crate::extractor::sidecar_path_for;

use super::collect_files;

pub fn status() -> Result<()> {
    let config_path = std::path::Path::new(".fmmrc.json");
    let config_exists = config_path.exists();
    let config = Config::load().unwrap_or_default();

    println!("{}", "fmm Status".cyan().bold());
    println!("{}", "=".repeat(40).dimmed());

    println!("\n{}", "Configuration:".yellow().bold());
    if config_exists {
        println!("  {} .fmmrc.json found", "✓".green());
    } else {
        println!("  {} No .fmmrc.json (using defaults)", "!".yellow());
    }

    println!("\n{}", "Supported Languages:".yellow().bold());
    let mut langs: Vec<_> = config.languages.iter().collect();
    langs.sort();
    println!(
        "  {}",
        langs
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    println!("\n{}", "Workspace:".yellow().bold());
    let cwd = std::env::current_dir().unwrap_or_default();
    println!("  Path: {}", cwd.display());

    match collect_files(".", &config) {
        Ok(files) => {
            let sidecar_count = files
                .iter()
                .filter(|f| sidecar_path_for(f).exists())
                .count();
            println!(
                "  {} source files, {} sidecars",
                files.len().to_string().white().bold(),
                sidecar_count.to_string().white().bold()
            );
        }
        Err(e) => {
            println!("  {} Error scanning: {}", "✗".red(), e);
        }
    }

    println!();
    Ok(())
}
