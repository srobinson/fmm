use clap::Args;
use clap_complete::Shell;

#[derive(Args)]
pub struct CompletionsCommandArgs {
    /// Target shell
    pub shell: Shell,
}
