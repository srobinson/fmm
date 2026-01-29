mod cli;
mod compare;
mod config;
mod extractor;
mod formatter;
mod manifest;
mod mcp;
mod parser;

use anyhow::Result;
use clap::Parser as ClapParser;
use cli::{Cli, Commands, OutputFormat};
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
        Commands::Status => {
            cli::status()?;
        }
        Commands::Search {
            export,
            imports,
            loc,
            depends_on,
            json,
        } => {
            cli::search(export, imports, loc, depends_on, json)?;
        }
        Commands::Mcp => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Commands::Compare {
            url,
            branch,
            src_path,
            tasks,
            runs,
            output,
            format,
            max_budget,
            no_cache,
            quick,
            model,
        } => {
            let report_format = match format {
                OutputFormat::Json => compare::ReportFormat::Json,
                OutputFormat::Markdown => compare::ReportFormat::Markdown,
                OutputFormat::Both => compare::ReportFormat::Both,
            };

            let options = compare::CompareOptions {
                branch,
                src_path,
                task_set: tasks,
                runs,
                output,
                format: report_format,
                max_budget,
                use_cache: !no_cache,
                quick,
                model,
            };

            compare::compare(&url, options)?;
        }
    }

    Ok(())
}
