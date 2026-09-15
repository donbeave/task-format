//! Command-line surface split between host operator and in-container runtimes.

pub mod container;
pub mod host;

use std::path::PathBuf;

/// Global flags shared by the host CLI.
#[derive(Debug, Clone)]
pub struct GlobalOpts {
    pub config: Option<PathBuf>,
    pub auto: bool,
    pub yes: bool,
    pub verbose: bool,
}

impl GlobalOpts {
    pub fn from_host(cli: &host::Cli) -> Self {
        Self {
            config: cli.config.clone(),
            auto: cli.auto,
            yes: cli.yes,
            verbose: cli.verbose,
        }
    }
}
