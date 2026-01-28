mod cli;
mod config;
mod extractor;
mod formatter;
mod manifest;
mod parser;

use anyhow::Result;
use clap::Parser as ClapParser;
use cli::{Cli, Commands};
use colored::Colorize;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            path,
            dry_run,
            manifest_only,
        } => {
            println!("{}", "Generating frontmatter...".green().bold());
            cli::generate(&path, dry_run, manifest_only)?;
        }
        Commands::Update {
            path,
            dry_run,
            manifest_only,
        } => {
            println!("{}", "Updating frontmatter...".green().bold());
            cli::update(&path, dry_run, manifest_only)?;
        }
        Commands::Validate { path } => {
            println!("{}", "Validating frontmatter...".green().bold());
            cli::validate(&path)?;
        }
        Commands::Init => {
            println!("{}", "Initializing fmm configuration...".green().bold());
            cli::init()?;
        }
    }

    Ok(())
}
