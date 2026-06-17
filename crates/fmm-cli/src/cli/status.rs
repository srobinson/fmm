use anyhow::Result;
use colored::Colorize;

use fmm_core::config::Config;
use fmm_core::store::{GIT_BRANCH_META_KEY, GIT_DIRTY_META_KEY, GIT_SHA_META_KEY};
use fmm_store;

use super::collect_files;

struct IndexGitMeta {
    sha: String,
    branch: Option<String>,
    dirty: Option<String>,
}

pub fn status() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    println!("{}", "fmm Status".cyan().bold());
    println!("{}", "=".repeat(40).dimmed());

    println!("\n{}", "Configuration:".yellow().bold());
    if std::path::Path::new(".fmmrc.toml").exists() {
        println!("  {} .fmmrc.toml found", "✓".green());
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

    let db_path = cwd.join(fmm_store::DB_FILENAME);
    let (indexed_count, git_meta): (i64, Option<IndexGitMeta>) = if db_path.exists() {
        match fmm_store::open_db(&cwd) {
            Ok(conn) => {
                let indexed_count = conn
                    .query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0))
                    .unwrap_or(0);
                let git_meta = fmm_store::connection::read_meta(&conn, GIT_SHA_META_KEY)
                    .ok()
                    .flatten()
                    .filter(|sha| !sha.is_empty())
                    .map(|sha| IndexGitMeta {
                        sha,
                        branch: fmm_store::connection::read_meta(&conn, GIT_BRANCH_META_KEY)
                            .ok()
                            .flatten()
                            .filter(|branch| !branch.is_empty()),
                        dirty: fmm_store::connection::read_meta(&conn, GIT_DIRTY_META_KEY)
                            .ok()
                            .flatten(),
                    });
                (indexed_count, git_meta)
            }
            Err(_) => (0, None),
        }
    } else {
        (0, None)
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

    println!("\n{}", "Git Metadata:".yellow().bold());
    if let Some(meta) = git_meta {
        println!("  SHA: {}", short_sha(&meta.sha).white().bold());
        println!(
            "  Branch: {}",
            meta.branch.as_deref().unwrap_or("detached").white().bold()
        );
        println!(
            "  Dirty: {}",
            format_dirty(meta.dirty.as_deref()).white().bold()
        );
    } else {
        println!("  {} not a git repo / not stamped", "!".yellow());
    }

    println!();
    Ok(())
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(12).collect()
}

fn format_dirty(value: Option<&str>) -> &'static str {
    match value {
        Some("true") => "dirty",
        Some("false") => "clean",
        _ => "unknown",
    }
}
