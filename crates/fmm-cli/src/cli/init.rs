use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use fmm_core::config::Config;

use super::collect_files;
use super::sidecar;

pub fn init(force: bool, no_generate: bool) -> Result<()> {
    println!(
        "\n{}",
        "Frontmatter Matters — SQLite code intelligence for LLM navigation"
            .cyan()
            .bold()
    );
    println!();

    init_config(force)?;

    if !no_generate {
        println!();
        let config = Config::load().unwrap_or_default();
        let (files, _) = collect_files(".", &config)?;

        if !files.is_empty() {
            let mut lang_set = std::collections::BTreeSet::new();
            for file in &files {
                if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
                    lang_set.insert(ext.to_string());
                }
            }
            println!(
                "{} {} source files detected ({})",
                "✓".green(),
                files.len(),
                lang_set.into_iter().collect::<Vec<_>>().join(", ")
            );

            sidecar::generate(&[".".to_string()], false, false, false)?;

            let root = super::resolve_root(".")?;
            if let Ok(conn) = fmm_store::open_db(&root) {
                let file_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
                    .unwrap_or(0);
                let export_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM exports", [], |r| r.get(0))
                    .unwrap_or(0);
                println!(
                    "\n  {} {} files indexed, {} exports",
                    "✓".green(),
                    file_count,
                    export_count
                );

                if let Ok(store) = fmm_store::SqliteStore::open(&root)
                    && let Ok(manifest) = fmm_core::store::FmmStore::load_manifest(&store)
                    && let Some((export_name, _)) = manifest.export_index.iter().next()
                {
                    println!(
                        "\n  {} Try: fmm search --export {}",
                        "next:".cyan(),
                        export_name
                    );
                }
            }
        } else {
            println!(
                "{} No supported source files found — index will be created when you add code",
                "!".yellow()
            );
        }
    }

    println!();
    println!("{}", "Setup complete!".green().bold());
    println!("  Config:   .fmmrc.toml");

    println!(
        "  {} Add '.fmm.db' to your .gitignore — the index is regeneratable",
        "hint:".cyan()
    );
    println!(
        "  {} .fmmrc.toml is optional — delete it to use built-in defaults",
        "hint:".cyan()
    );

    if no_generate {
        println!(
            "\n  {} Run 'fmm generate' to index your codebase",
            "next:".cyan()
        );
    } else {
        println!(
            "\n  {} Your AI assistant now navigates this codebase via the fmm index",
            "✓".green()
        );
    }

    Ok(())
}

const FMMRC_TEMPLATE: &str = r#"# fmm configuration
# Only include fields you want to override — defaults apply for everything else.

# Maximum lines per file. Files exceeding this limit are skipped during indexing.
# Default: 100000
# max_lines = 100_000

# Glob patterns to exclude (in addition to .gitignore and .fmmignore).
# exclude = ["benchmarks/fixtures/**", "vendor/**"]

# Override which file extensions to index (default: 29 languages).
# languages = ["ts", "tsx", "js", "jsx", "py", "rs"]

# Test file detection patterns.
# [test_patterns]
# path_contains = ["/test/", "/tests/", "/spec/", "/e2e/", "/__tests__/"]
# filename_suffixes = [".spec.ts", ".test.ts", ".test.js", "_test.go", "_test.rs"]
"#;

fn init_config(force: bool) -> Result<()> {
    let toml_path = Path::new(".fmmrc.toml");
    if toml_path.exists() && !force {
        println!("{} .fmmrc.toml already exists (skipping)", "!".yellow());
        return Ok(());
    }

    if toml_path.exists() {
        println!("{} Overwriting .fmmrc.toml (--force)", "!".yellow());
    }

    std::fs::write(toml_path, FMMRC_TEMPLATE).context("Failed to write .fmmrc.toml")?;

    println!("{} Created .fmmrc.toml", "✓".green());
    Ok(())
}
