//! Local filesystem task monitor. Execution settings are operator-owned CLI flags.
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use taskfmt::server::{AppState, ServerConfig, adapter::AdapterConfig, router};

#[derive(Parser)]
#[command(about="Local filesystem task progress monitor",version=taskfmt::VERSION)]
struct Args {
    /// Directory containing project/group/task packages.
    #[arg(long, visible_alias = "tasks-root", default_value = "projects")]
    projects_root: PathBuf,
    #[arg(long, default_value = ".monitor-runs")]
    runs_root: PathBuf,
    #[arg(long, default_value = "127.0.0.1:3001")]
    bind: SocketAddr,
    #[arg(long, default_value = "http://127.0.0.1:5173")]
    origin: String,
    #[arg(long, default_value_t = 2)]
    concurrency: usize,
    /// Explicit existing taskfmt executable. Omit to serve read-only monitoring.
    #[arg(long,requires_all=["config","repo"])]
    taskfmt: Option<PathBuf>,
    #[arg(long, requires = "taskfmt")]
    config: Option<PathBuf>,
    /// Existing operator-approved repository; the browser cannot override this value.
    #[arg(long, requires = "taskfmt")]
    repo: Option<String>,
    #[arg(long, requires = "taskfmt")]
    agent: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.bind.ip().is_loopback(),
        "task-monitor only binds loopback addresses"
    );
    let adapter = match (args.taskfmt, args.config, args.repo) {
        (Some(taskfmt), Some(experiment_config), Some(repo)) => {
            let config = AdapterConfig {
                taskfmt: taskfmt.canonicalize()?,
                experiment_config: experiment_config.canonicalize()?,
                repo,
                agent: args.agent,
            };
            config.validate()?;
            Some(config)
        }
        _ => None,
    };
    let state = AppState::open(ServerConfig {
        tasks_root: args.projects_root,
        runs_root: args.runs_root,
        adapter,
        concurrency: args.concurrency,
        origin: args.origin,
    })
    .await?;
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    eprintln!(
        "task-monitor listening on http://{}",
        listener.local_addr()?
    );
    let shutdown_state = state.clone();
    axum::serve(listener, router(state.clone()))
        .with_graceful_shutdown(async move {
            #[cfg(unix)]
            {
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(mut terminate) => {
                        tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = terminate.recv()=>{}}
                    }
                    Err(error) => eprintln!("cannot install shutdown signal: {error}"),
                }
            }
            #[cfg(not(unix))]
            let _ = tokio::signal::ctrl_c().await;
            shutdown_state.shutdown().await;
        })
        .await?;
    state.shutdown().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_default_and_legacy_alias_remain_compatible() {
        assert_eq!(
            Args::try_parse_from(["task-monitor"])
                .unwrap()
                .projects_root,
            PathBuf::from("projects")
        );
        for flag in ["--projects-root", "--tasks-root"] {
            let args =
                Args::try_parse_from(["task-monitor", flag, "/tmp/custom-projects"]).unwrap();
            assert_eq!(args.projects_root, PathBuf::from("/tmp/custom-projects"));
        }
        assert!(
            Args::try_parse_from([
                "task-monitor",
                "--projects-root",
                "one",
                "--tasks-root",
                "two"
            ])
            .is_err()
        );
    }
}
