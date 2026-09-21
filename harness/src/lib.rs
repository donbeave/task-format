//! Re-exports for integration tests — preserves pre-split module paths.

pub use taskfmt_host::{config, interactive, ops, runstate, selection};

pub mod cmds {
    pub use taskfmt_host::cmds::*;
    pub mod init {
        pub use taskfmt_bin::cmds::init::*;
    }
}

pub mod cli {
    pub use taskfmt_bin::cli as container;
    pub use taskfmt_host::cli as host;
    pub use taskfmt_runtime::cli as runtime;
}

pub mod itest;
pub mod testbin;
