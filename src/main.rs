use anyhow::Result;
use clap::Parser as ClapParser;
use colored::Colorize;
use fmm::cli::{self, Cli, Commands, OutputFormat};
use fmm::compare;
use fmm::mcp;

fn main() -> Result<()> {
    let cli_args = Cli::parse();

    match cli_args.command {
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
        Commands::Mcp | Commands::Serve => {
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
