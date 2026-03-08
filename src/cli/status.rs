use anyhow::Result;
use colored::Colorize;

use crate::config::Config;
use crate::db;

use super::collect_files;

pub fn status() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    println!("{}", "fmm Status".cyan().bold());
    println!("{}", "=".repeat(40).dimmed());

    println!("\n{}", "Configuration:".yellow().bold());
    if std::path::Path::new(".fmmrc.toml").exists() {
        println!("  {} .fmmrc.toml found", "✓".green());
    } else if std::path::Path::new(".fmmrc.json").exists() {
        println!(
            "  {} .fmmrc.json found (deprecated — migrate to .fmmrc.toml)",
            "!".yellow()
        );
    } else {
        println!("  {} No config file (using defaults)", "!".yellow());
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

    let db_path = cwd.join(db::DB_FILENAME);
    let indexed_count: i64 = if db_path.exists() {
        db::open_db(&cwd)
            .and_then(|conn| {
                conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0))
                    .map_err(Into::into)
            })
            .unwrap_or(0)
    } else {
        0
    };

    match collect_files(".", &config) {
        Ok((files, _)) => {
            println!(
                "  {} source files, {} indexed",
                files.len().to_string().white().bold(),
                indexed_count.to_string().white().bold()
            );
        }
        Err(e) => {
            println!("  {} Error scanning: {}", "✗".red(), e);
        }
    }

    println!();
    Ok(())
}
