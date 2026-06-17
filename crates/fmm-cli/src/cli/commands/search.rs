use clap::Args;

#[derive(Args)]
pub struct SearchCommandArgs {
    /// Search term — searches exports, files, and imports (smart ranking)
    #[arg(value_name = "TERM")]
    pub term: Option<String>,

    /// Find file by export name (exact O(1) + fuzzy substring)
    #[arg(short = 'e', long = "export")]
    pub export: Option<String>,

    /// Find files that import a module
    #[arg(short = 'i', long = "imports")]
    pub imports: Option<String>,

    /// Filter by line count (e.g., ">500", "<100", "=200")
    #[arg(
        short = 'l',
        long = "loc",
        long_help = "Filter files by line count.

Supports comparison operators: >500, <100, >=50, <=1000, =200.
A bare number is treated as exact match (=)."
    )]
    pub loc: Option<String>,

    /// Minimum lines of code
    #[arg(long = "min-loc", value_name = "MIN_LOC")]
    pub min_loc: Option<usize>,

    /// Maximum lines of code
    #[arg(long = "max-loc", value_name = "MAX_LOC")]
    pub max_loc: Option<usize>,

    /// Maximum number of fuzzy export results
    #[arg(long, value_name = "LIMIT")]
    pub limit: Option<usize>,

    /// Find files that depend on a path
    #[arg(short = 'd', long = "depends-on")]
    pub depends_on: Option<String>,

    /// Scope --export results to a directory prefix (e.g. crates/fmm-core/src/)
    #[arg(long = "dir")]
    pub dir: Option<String>,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}
