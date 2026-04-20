use clap::{CommandFactory, Parser as ClapParser};
use colored::Colorize;
use fmm::cli::{self, Cli, Commands};
use fmm::mcp;

fn main() {
    if let Err(err) = run() {
        print_error(&err);
        std::process::exit(1);
    }
}

fn print_error(err: &anyhow::Error) {
    eprintln!("{} {}", "error:".red().bold(), err);
    let chain: Vec<_> = err.chain().skip(1).collect();
    if !chain.is_empty() {
        for cause in chain {
            eprintln!("  {} {}", "caused by:".yellow(), cause);
        }
    }

    let msg = err.to_string();
    if msg.contains("LOC") || msg.contains("loc") {
        eprintln!(
            "\n  {} Valid LOC filters: {}, {}, {}, {}, {}",
            "hint:".cyan(),
            ">500".bold(),
            "<100".bold(),
            "=200".bold(),
            ">=50".bold(),
            "<=1000".bold()
        );
    }
}

fn run() -> anyhow::Result<()> {
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

    run_command(command)
}

fn run_command(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Generate {
            paths,
            dry_run,
            force,
            quiet,
        } => {
            cli::generate(&paths, dry_run, force, quiet)?;
        }
        Commands::Validate { paths } => {
            println!("{}", "Validating index...".green().bold());
            cli::validate(&paths)?;
        }
        Commands::Clean {
            paths,
            dry_run,
            delete_db,
        } => {
            println!("{}", "Cleaning index...".green().bold());
            cli::clean(&paths, dry_run, delete_db)?;
        }
        Commands::Watch { path, debounce } => {
            cli::watch(&path, debounce)?;
        }
        Commands::Init { force, no_generate } => {
            cli::init(force, no_generate)?;
        }
        Commands::Status => {
            cli::status()?;
        }
        Commands::Search {
            term,
            export,
            imports,
            loc,
            min_loc,
            max_loc,
            limit,
            depends_on,
            dir,
            json,
        } => {
            cli::search(cli::SearchOptions {
                term,
                export,
                imports,
                loc,
                min_loc,
                max_loc,
                limit,
                depends_on,
                directory: dir,
                json_output: json,
            })?;
        }
        Commands::Glossary {
            pattern,
            mode,
            limit,
            json,
        } => {
            cli::glossary(pattern, &mode, limit, json)?;
        }
        Commands::Lookup { symbol, json } => {
            cli::lookup(&symbol, json)?;
        }
        Commands::Read {
            symbol,
            no_truncate,
            line_numbers,
            json,
        } => {
            cli::read_symbol(&symbol, no_truncate, line_numbers, json)?;
        }
        Commands::Deps {
            file,
            depth,
            filter,
            json,
        } => {
            cli::deps(&file, depth, &filter, json)?;
        }
        Commands::Outline {
            file,
            include_private,
            json,
        } => {
            cli::outline(&file, include_private, json)?;
        }
        Commands::Ls {
            directory,
            pattern,
            sort_by,
            order,
            group_by,
            filter,
            limit,
            offset,
            json,
        } => {
            cli::ls(
                directory.as_deref(),
                pattern.as_deref(),
                &sort_by,
                order.as_deref(),
                group_by.as_deref(),
                &filter,
                limit,
                offset,
                json,
            )?;
        }
        Commands::Exports {
            pattern,
            file,
            dir,
            limit,
            offset,
            json,
        } => {
            cli::exports(
                pattern.as_deref(),
                file.as_deref(),
                dir.as_deref(),
                limit,
                offset,
                json,
            )?;
        }
        Commands::Mcp | Commands::Serve => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "fmm", &mut std::io::stdout());
        }
    }

    Ok(())
}
