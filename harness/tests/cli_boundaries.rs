//! CLI surface boundaries between validation, runtime, and host binaries.

use std::process::Command;

use clap::CommandFactory;
use taskfmt_harness::cli::container::Cli as ValidationCli;
use taskfmt_harness::cli::host::Cli as HostCli;
use taskfmt_harness::cli::runtime::Cli as RuntimeCli;

const VALIDATION_COMMANDS: &[&str] = &["init", "status", "lint", "verify"];

const RUNTIME_COMMANDS: &[&str] = &[
    "container-entrypoint",
    "prereqs",
    "agent-launch",
    "codex-login",
];

const HOST_COMMANDS: &[&str] = &[
    "lint",
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

const RUNTIME_ONLY: &[&str] = RUNTIME_COMMANDS;

fn subcommand_names(cli: &clap::Command) -> Vec<String> {
    cli.get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect()
}

#[test]
fn validation_cli_lists_init_status_lint_verify_only() {
    let names = subcommand_names(&ValidationCli::command());
    for expected in VALIDATION_COMMANDS {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    for forbidden in RUNTIME_COMMANDS {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "validation must not expose {forbidden}: {names:?}"
        );
    }
    for forbidden in ["run", "experiment", "gate", "build-images", "selfcheck"] {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "validation must not expose {forbidden}: {names:?}"
        );
    }
}

#[test]
fn runtime_cli_lists_boot_commands_only() {
    let names = subcommand_names(&RuntimeCli::command());
    for expected in RUNTIME_COMMANDS {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    for forbidden in VALIDATION_COMMANDS {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "runtime must not expose {forbidden}: {names:?}"
        );
    }
}

#[test]
fn host_cli_never_lists_validation_or_runtime_only_commands() {
    let names = subcommand_names(&HostCli::command());
    for expected in HOST_COMMANDS {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    for forbidden in RUNTIME_ONLY {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "host must not expose {forbidden}: {names:?}"
        );
    }
    assert!(
        !names.iter().any(|name| name == "verify"),
        "verify belongs to in-container taskfmt only: {names:?}"
    );
    assert!(
        !names.iter().any(|name| name == "init"),
        "init belongs to in-container taskfmt only: {names:?}"
    );
    assert!(
        !names.iter().any(|name| name == "progress-init"),
        "progress-init removed from host: {names:?}"
    );
}

#[test]
fn validation_binary_rejects_host_and_runtime_subcommands() {
    let bin = std::env::var("CARGO_BIN_EXE_taskfmt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/taskfmt")
        });
    for subcommand in ["run", "container-entrypoint", "agent-launch"] {
        let output = Command::new(&bin)
            .args([subcommand, "--help"])
            .output()
            .unwrap_or_else(|err| panic!("failed to run {}: {err}", bin.display()));
        assert!(
            !output.status.success(),
            "{subcommand} should fail on validation binary"
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
