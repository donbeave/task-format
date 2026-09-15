//! CLI surface boundaries between in-container `taskfmt` and host `taskfmt-host`.

use std::process::Command;

use clap::CommandFactory;
use taskfmt::cli::container::Cli as ContainerCli;
use taskfmt::cli::host::Cli as HostCli;

const CONTAINER_COMMANDS: &[&str] = &[
    "lint",
    "verify",
    "container-entrypoint",
    "prereqs",
    "agent-launch",
    "codex-login",
];

const HOST_COMMANDS: &[&str] = &[
    "lint",
    "progress-init",
    "selftest",
    "selfcheck",
    "build-images",
    "preload",
    "repo",
    "run",
    "gate",
    "promote",
    "status",
    "attach",
    "ps",
    "experiment",
];

const CONTAINER_ONLY: &[&str] = &[
    "container-entrypoint",
    "prereqs",
    "agent-launch",
    "codex-login",
];

fn subcommand_names(cli: &clap::Command) -> Vec<String> {
    cli.get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect()
}

#[test]
fn container_cli_lists_validation_and_runtime_only() {
    let names = subcommand_names(&ContainerCli::command());
    for expected in CONTAINER_COMMANDS {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    for forbidden in ["run", "experiment", "gate", "build-images", "selfcheck"] {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "container must not expose {forbidden}: {names:?}"
        );
    }
}

#[test]
fn host_cli_never_lists_container_runtime_commands() {
    let names = subcommand_names(&HostCli::command());
    for expected in HOST_COMMANDS {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    for forbidden in CONTAINER_ONLY {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "host must not expose {forbidden}: {names:?}"
        );
    }
    assert!(
        !names.iter().any(|name| name == "verify"),
        "verify belongs to in-container taskfmt only: {names:?}"
    );
}

#[test]
fn container_binary_rejects_host_only_subcommands() {
    let bin = std::env::var("CARGO_BIN_EXE_taskfmt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/taskfmt")
        });
    for subcommand in ["run", "experiment", "gate"] {
        let output = Command::new(&bin)
            .args([subcommand, "--help"])
            .output()
            .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()));
        assert!(
            !output.status.success(),
            "{subcommand} should fail on container binary"
        );
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("unrecognized subcommand"),
            "{subcommand}: {combined}"
        );
    }
}
