use clap::Args;

#[derive(Args)]
pub struct CleanCommandArgs {
    /// Paths to files or directories (defaults to current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Show what would be removed without deleting files
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Delete the .fmm.db file entirely instead of just clearing its contents
    #[arg(long = "db")]
    pub delete_db: bool,
}
