use clap::Args;

#[derive(Args)]
pub struct WatchCommandArgs {
    /// Path to directory to watch
    #[arg(default_value = ".")]
    pub path: String,

    /// Debounce delay in milliseconds
    #[arg(long, default_value = "300")]
    pub debounce: u64,
}
