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
        Commands::Generate(args) => {
            cli::generate_with_git(
                &args.paths,
                args.dry_run,
                args.force,
                args.quiet,
                args.sha.as_deref(),
                args.no_git,
            )?;
        }
        Commands::Validate(args) => {
            println!("{}", "Validating index...".green().bold());
            cli::validate(&args.paths)?;
        }
        Commands::Clean(args) => {
            println!("{}", "Cleaning index...".green().bold());
            cli::clean(&args.paths, args.dry_run, args.delete_db)?;
        }
        Commands::Watch(args) => {
            cli::watch(&args.path, args.debounce)?;
        }
        Commands::Init(args) => {
            cli::init(args.force, args.no_generate)?;
        }
        Commands::Status => {
            cli::status()?;
        }
        Commands::Search(args) => {
            cli::search(cli::SearchOptions {
                term: args.term,
                export: args.export,
                imports: args.imports,
                loc: args.loc,
                min_loc: args.min_loc,
                max_loc: args.max_loc,
                limit: args.limit,
                depends_on: args.depends_on,
                directory: args.dir,
                json_output: args.json,
            })?;
        }
        Commands::Glossary(args) => {
            cli::glossary(
                args.pattern,
                &args.mode,
                args.limit,
                &args.precision,
                args.no_truncate,
                args.json,
            )?;
        }
        Commands::Lookup(args) => {
            cli::lookup(&args.symbol, args.json)?;
        }
        Commands::Similar(args) => {
            let name = args.name;
            cli::similar(
                &name,
                args.signature,
                args.kind,
                args.directory,
                args.limit,
                args.include_tests,
                args.json,
            )?;
        }
        Commands::Read(args) => {
            cli::read_symbol(&args.symbol, args.no_truncate, args.line_numbers, args.json)?;
        }
        Commands::Deps(args) => {
            cli::deps(&args.file, args.depth, &args.filter, args.json)?;
        }
        Commands::Cycles(args) => {
            cli::cycles(
                args.file.as_deref(),
                &args.filter,
                &args.edge_mode,
                args.json,
            )?;
        }
        Commands::Outline(args) => {
            cli::outline(&args.file, args.include_private, args.json)?;
        }
        Commands::Ls(args) => {
            cli::ls(
                args.directory.as_deref(),
                args.pattern.as_deref(),
                &args.sort_by,
                args.order.as_deref(),
                args.group_by.as_deref(),
                &args.filter,
                args.limit,
                args.offset,
                args.json,
            )?;
        }
        Commands::Exports(args) => {
            cli::exports(
                args.pattern.as_deref(),
                args.file.as_deref(),
                args.dir.as_deref(),
                args.limit,
                args.offset,
                args.json,
            )?;
        }
        Commands::Mcp | Commands::Serve => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Commands::Completions(args) => {
            clap_complete::generate(
                args.shell,
                &mut Cli::command(),
                "fmm",
                &mut std::io::stdout(),
            );
        }
    }

    Ok(())
}
