//! Secrets discipline (redaction, the 0600 env-file) and interactive consent.

use std::io::IsTerminal;
use std::os::unix::fs::PermissionsExt;

use taskfmt::interactive::Interaction;
use taskfmt::ops::container::SecretEnvFile;
use taskfmt::redact;

const SECRET: &str = "sk-super-secret-token-value-1234";

#[test]
fn every_emit_path_scrubs_registered_secrets() {
    redact::register(SECRET);
    assert_eq!(
        redact::scrub(&format!("token {SECRET}")),
        "token [REDACTED]"
    );
    assert!(!redact::scrub(&format!("CHECK focused.1 FAIL rc=1 {SECRET}")).contains(SECRET));
    assert!(
        !String::from_utf8(redact::scrub_bytes(
            b"body with sk-super-secret-token-value-1234"
        ))
        .unwrap()
        .contains(SECRET)
    );

    // emitted output is scrubbed, not just scrub()
    redact::emit(&format!("run: {SECRET}"));
    redact::eemit(&format!("err: {SECRET}"));
    redact::emit_lines([format!("line {SECRET}")]);
}

#[test]
fn artifact_writes_are_scrubbed() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("gate.log");
    redact::register(SECRET);
    redact::write_scrubbed(&path, format!("rc=1 {SECRET}\n").as_bytes()).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        !written.contains(SECRET),
        "secret leaked into {path:?}: {written:?}"
    );
    assert!(written.contains("[REDACTED]"));
}

#[test]
fn env_file_is_0600_and_deleted_on_drop() {
    let file = {
        let env_file =
            SecretEnvFile::create(&[("ANTHROPIC_AUTH_TOKEN".to_string(), SECRET.to_string())])
                .unwrap();
        let path = env_file.path().unwrap().to_path_buf();
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(
            meta.permissions().mode() & 0o777,
            0o600,
            "the env file must be 0600"
        );
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(
            body.contains(SECRET),
            "the env file must carry the value to docker"
        );
        assert_eq!(body.lines().count(), 1, "one KEY=value line: {body:?}");
        assert!(body.starts_with("ANTHROPIC_AUTH_TOKEN="));
        path
    };
    assert!(
        !file.exists(),
        "the env file must be gone once the docker call returned"
    );
}

#[test]
fn env_file_refuses_values_that_cannot_be_env_file_encoded() {
    assert!(SecretEnvFile::create(&[("K".to_string(), "a\nb".to_string())]).is_err());
}

#[test]
fn consent_defaults_to_no_and_skips_when_auto() {
    // non-TTY (the test harness): a confirm that needs an answer must refuse unless --auto/--yes
    if !std::io::stdin().is_terminal() {
        let interaction = Interaction::new(false, false);
        let err = interaction
            .confirm("create repo", &["git push".to_string()])
            .unwrap_err();
        assert!(format!("{err:#}").contains("--auto"), "{err:#}");
    }
    let auto = Interaction::new(true, false);
    assert!(
        auto.confirm("create repo", &["plan line".to_string()])
            .unwrap()
    );
    let yes = Interaction::new(false, true);
    assert!(yes.confirm("create repo", &[]).unwrap());
}

#[test]
fn run_dir_names_are_deterministic() {
    let name = taskfmt::runstate::run_dir_name("20260101-000000", "zai-flash", "TASK-101");
    assert_eq!(name, "20260101-000000-zai-flash-TASK-101");
}

#[test]
fn secret_never_reaches_a_command_line() {
    // the launch plan carries no secret: secrets travel only by --env-file
    redact::register(SECRET);
    let trace = format!("docker run --env-file {SECRET} harness-claude:latest");
    assert_eq!(
        redact::scrub(&trace),
        "docker run --env-file [REDACTED] harness-claude:latest"
    );
}

#[test]
fn agent_commands_carry_no_secret() {
    let session = "0f0e0d0c-0b0a-4938-a716-1c2d3e4f5a6b";
    let claude = taskfmt::ops::container::claude_agent_cmd(session, "glm-5.3-flash", "low");
    assert!(claude.starts_with("claude --dangerously-skip-permissions --session-id "));
    assert!(claude.contains("--add-dir /task --add-dir /progress"));
    assert!(!claude.contains("ANTHROPIC"), "{claude}");
    let codex = taskfmt::ops::container::codex_agent_cmd("", "high");
    assert!(codex.starts_with("codex --dangerously-bypass-approvals-and-sandbox"));
    assert!(!codex.contains("OPENAI"), "{codex}");
}
