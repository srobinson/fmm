use clap::Args;

#[derive(Args)]
pub struct GlossaryCommandArgs {
    /// Symbol name or substring pattern (case-insensitive)
    #[arg(value_name = "PATTERN")]
    pub pattern: Option<String>,

    /// Filter mode: source (default, no tests), tests (test coverage only), all (unfiltered)
    #[arg(long, value_name = "MODE", default_value = "source", value_parser = ["source", "tests", "all"])]
    pub mode: String,

    /// Maximum number of entries returned (default: 10)
    #[arg(long)]
    pub limit: Option<usize>,

    /// Precision level: named (default, fast) or call-site (tree-sitter verification)
    #[arg(long, value_name = "PRECISION", default_value = "named", value_parser = ["named", "call-site"])]
    pub precision: String,

    /// Match only the exact export name instead of a substring
    #[arg(long)]
    pub exact: bool,

    /// Return full output, bypassing the 10KB truncation cap
    #[arg(long = "no-truncate")]
    pub no_truncate: bool,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}
