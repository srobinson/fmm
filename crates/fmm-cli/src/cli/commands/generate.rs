use clap::Args;

#[derive(Args)]
pub struct GenerateCommandArgs {
    /// Paths to files or directories (defaults to current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Show what would be created/updated without writing files
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Re-index all files, bypassing mtime comparison
    #[arg(short, long)]
    pub force: bool,

    /// Override the indexed git SHA stamp
    #[arg(long, value_name = "SHA", conflicts_with = "no_git")]
    pub sha: Option<String>,

    /// Skip git metadata probing and leave git meta keys unstamped
    #[arg(long)]
    pub no_git: bool,

    /// Suppress progress bars — print only the final summary line
    #[arg(short = 'q', long)]
    pub quiet: bool,
}
