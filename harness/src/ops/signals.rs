//! Signal flags for the container-side supervisors.
//!
//! The container entrypoint must park (never exit) after a prereq failure, and `agent-launch` must
//! stop the herdr server gracefully when `docker stop` sends SIGTERM. `signal-hook`'s flag API
//! keeps this unsafe-free.

use std::sync::atomic::{AtomicBool, Ordering};

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;

static TERMINATE: std::sync::LazyLock<std::sync::Arc<AtomicBool>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(AtomicBool::new(false)));

/// Install SIGTERM/SIGINT handlers that raise the terminate flag. Idempotent.
pub fn install_terminate_flag() -> anyhow::Result<()> {
    flag::register(SIGTERM, TERMINATE.clone())?;
    flag::register(SIGINT, TERMINATE.clone())?;
    Ok(())
}

pub fn terminate_requested() -> bool {
    TERMINATE.load(Ordering::SeqCst)
}

/// Block until the terminate flag is raised or `child` exits, polling every 200 ms.
/// Returns `true` when the flag fired first.
pub fn wait_or_terminate(child: &mut std::process::Child) -> anyhow::Result<bool> {
    loop {
        if terminate_requested() {
            return Ok(true);
        }
        match child.try_wait()? {
            Some(_) => return Ok(false),
            None => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    }
}

/// Sleep in small slices so a pending signal is noticed promptly.
pub fn sleep_until_terminate(total: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + total;
    while std::time::Instant::now() < deadline {
        if terminate_requested() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    terminate_requested()
}
