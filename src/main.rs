use anyhow::Result;
use clap::{CommandFactory, Parser as ClapParser};
use colored::Colorize;
use fmm::cli::{self, Cli, Commands, GhSubcommand, OutputFormat};
use fmm::compare;
use fmm::gh;
use fmm::mcp;

fn main() -> Result<()> {
    let cli_args = Cli::parse();

    if cli_args.markdown_help {
        let markdown = clap_markdown::help_markdown::<Cli>();
        print!("{}", markdown);
        return Ok(());
    }

    if let Some(out_dir) = cli_args.generate_man_pages {
        std::fs::create_dir_all(&out_dir)?;
        let cmd = Cli::command();
        clap_mangen::generate_to(cmd, &out_dir)?;
        let count = std::fs::read_dir(&out_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.to_str() == Some("1"))
            })
            .count();
        eprintln!("Generated {} man page(s) in {}", count, out_dir.display());
        return Ok(());
    }

    let command = match cli_args.command {
        Some(cmd) => cmd,
        None => {
            Cli::command().print_long_help()?;
            return Ok(());
        }
    };

    match command {
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
                compare,
                output,
            } => {
                let options = gh::GhIssueOptions {
                    model,
                    max_turns,
                    max_budget,
                    dry_run,
                    branch_prefix,
                    no_pr,
                    workspace,
                    compare,
                    output,
                };
                gh::gh_issue(&url, options)?;
            }
        },
        Commands::Mcp | Commands::Serve => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "fmm", &mut std::io::stdout());
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
