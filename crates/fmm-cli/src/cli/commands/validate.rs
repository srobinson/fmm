use clap::Args;

#[derive(Args)]
pub struct ValidateCommandArgs {
    /// Paths to files or directories (defaults to current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<String>,
}
