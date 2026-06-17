use clap::Parser;
use std::path::PathBuf;

mod command_tree;
mod commands;
mod files;
mod glossary;
pub mod init;
mod resolve;
mod search;
pub(crate) mod sidecar;
mod status;
mod watch;

// Re-export file/resolve utilities so sibling modules (sidecar, init, watch, status)
// can continue using `super::collect_files`, `super::resolve_root`, etc.
pub(crate) use files::{collect_files, collect_files_multi};
pub(crate) use resolve::{resolve_root, resolve_root_multi};

mod help_text;
use help_text::{HELP_TEMPLATE, LONG_ABOUT, LONG_HELP, SHORT_HELP};

mod generated_help;

pub use command_tree::Commands;
pub use commands::{
    CleanCommandArgs, CompletionsCommandArgs, CyclesCommandArgs, DepsCommandArgs, DupesCommandArgs,
    ExportsCommandArgs, GenerateCommandArgs, GlossaryCommandArgs, InitCommandArgs,
    LookupCommandArgs, LsCommandArgs, OutlineCommandArgs, ReadCommandArgs, SearchCommandArgs,
    SimilarCommandArgs, ValidateCommandArgs, WatchCommandArgs, cycles, deps, dupes, exports,
    lookup, ls, outline, read_symbol, similar,
};
pub use glossary::glossary;
pub use init::init;
pub use search::{SearchOptions, search};
pub use sidecar::{clean, generate, generate_with_git, validate};
pub use status::status;
pub use watch::watch;

#[derive(Parser)]
#[command(
    name = "fmm",
    about = LONG_ABOUT,
    long_about = LONG_ABOUT,
    before_help = SHORT_HELP,
    before_long_help = LONG_HELP,
    help_template = HELP_TEMPLATE,
    version = crate::VERSION,
    disable_help_subcommand = true,
    subcommand_required = false,
)]
pub struct Cli {
    /// Print CLI reference as Markdown and exit
    #[arg(long, hide = true)]
    pub markdown_help: bool,

    /// Generate man pages to the specified directory and exit
    #[arg(long, hide = true)]
    pub generate_man_pages: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}
