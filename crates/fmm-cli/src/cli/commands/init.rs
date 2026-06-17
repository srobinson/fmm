use clap::Args;

#[derive(Args)]
pub struct InitCommandArgs {
    /// Overwrite existing .fmmrc.toml without prompting
    #[arg(long)]
    pub force: bool,

    /// Skip auto-indexing (config only)
    #[arg(long)]
    pub no_generate: bool,
}
