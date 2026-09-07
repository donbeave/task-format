//! Internal crash-coupled executor owner; launched only by task-monitor.
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let artifacts = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing execution artifacts"))?;
    anyhow::ensure!(args.next().is_none(), "unexpected supervisor arguments");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(taskfmt::server::adapter::supervise(artifacts.into()));
    // Dedicated process: supervision already joined the child, cleaned containers, and
    // released its lease. Tokio's stdin reader may still block on the parent's live pipe;
    // it must not delay process exit after successful supervision.
    runtime.shutdown_background();
    result
}
