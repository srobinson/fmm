use anyhow::Result;
use clap::Parser as ClapParser;
use colored::Colorize;
use fmm::cli::{self, Cli, Commands, GhSubcommand, OutputFormat};
use fmm::compare;
use fmm::gh;
use fmm::mcp;

fn main() -> Result<()> {
    let cli_args = Cli::parse();

    match cli_args.command {
        Commands::Generate { path, dry_run } => {
            println!("{}", "Generating sidecars...".green().bold());
            cli::generate(&path, dry_run)?;
        }
        Commands::Update { path, dry_run } => {
            println!("{}", "Updating sidecars...".green().bold());
            cli::update(&path, dry_run)?;
        }
        Commands::Validate { path } => {
            println!("{}", "Validating sidecars...".green().bold());
            cli::validate(&path)?;
        }
        Commands::Clean { path, dry_run } => {
            println!("{}", "Cleaning sidecars...".green().bold());
            cli::clean(&path, dry_run)?;
        }
        Commands::Init {
            skill,
            mcp,
            all,
            no_generate,
        } => {
            cli::init(skill, mcp, all, no_generate)?;
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
        Commands::Gh { subcommand } => match subcommand {
            GhSubcommand::Issue {
                url,
                model,
                max_turns,
                max_budget,
                dry_run,
                branch_prefix,
                no_pr,
                workspace,
            } => {
                let options = gh::GhIssueOptions {
                    model,
                    max_turns,
                    max_budget,
                    dry_run,
                    branch_prefix,
                    no_pr,
                    workspace,
                };
                gh::gh_issue(&url, options)?;
            }
        },
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
