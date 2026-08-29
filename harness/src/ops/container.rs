//! Run-container lifecycle: the persistent `--privileged` container, its mounts, its env, and the
//! 0600 env-file that carries resolved secrets to `docker run` and nowhere else.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::config::{AgentProfile, ExperimentConfig, Resolved};
use crate::redact;
use crate::runstate::Manifest;

use super::docker;

/// Every run container is named `<CONTAINER_PREFIX><run id>`.
pub const CONTAINER_PREFIX: &str = "harness-";

/// `harness-<run id>` — named and persistent so the operator can re-attach later.
pub fn container_name(run_id: &str) -> String {
    format!("{CONTAINER_PREFIX}{run_id}")
}

/// A docker `--env-file` that exists only for the duration of one `docker run`.
///
/// The file is created with mode 0600 in the system temp dir, holds the resolved secrets, and is
/// deleted from `Drop` — i.e. as soon as the docker invocation returns, whatever the outcome.
pub struct SecretEnvFile {
    path: Option<PathBuf>,
}

impl SecretEnvFile {
    /// Write `KEY=value` lines. Values must not contain newlines (docker's env-file format).
    pub fn create(entries: &[(String, String)]) -> anyhow::Result<Self> {
        if entries.is_empty() {
            return Ok(Self { path: None });
        }
        let mut body = String::new();
        for (key, value) in entries {
            if key.is_empty() || key.contains(['=', '\n']) {
                anyhow::bail!("invalid secret env key: {key:?}");
            }
            if value.contains('\n') {
                anyhow::bail!(
                    "secret value for {key} contains a newline; cannot pass it via --env-file"
                );
            }
            body.push_str(&format!("{key}={value}\n"));
        }
        let dir = tempfile::env::temp_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!(".taskfmt-env-{}", uuid::Uuid::new_v4()));
        redact::register(&body);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .with_context(|| format!("creating secret env file {}", path.display()))?;
        use std::io::Write;
        file.write_all(body.as_bytes())?;
        Ok(Self { path: Some(path) })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl Drop for SecretEnvFile {
    fn drop(&mut self) {
        if let Some(path) = self.path.take()
            && let Err(err) = std::fs::remove_file(&path)
        {
            redact::eemit(&format!(
                "could not remove the secret env file {}: {err}",
                path.display()
            ));
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub container: String,
    pub image: String,
    pub mounts: Vec<docker::Mount>,
    pub env: Vec<(String, String)>,
    pub memory: String,
    pub cpus: f32,
    pub pids_limit: i64,
}

/// Build the launch plan for one run: mounts `/work /task:ro /progress /agent-home /out /seed:ro`,
/// the static env from the profile plus `TASKFMT_BASE`, `AGENT_CMD`, `AGENT_KIND`, `HERDR_SESSION`.
pub fn launch_plan(
    cfg: &ExperimentConfig,
    resolved: &Resolved,
    manifest: &Manifest,
    profile: &AgentProfile,
    agent_cmd: &str,
    base_ref: &str,
) -> LaunchPlan {
    let run_dir = PathBuf::from(&manifest.run_dir);
    let mut mounts = vec![
        docker::Mount::rw(&run_dir.join("workspace"), "/work"),
        docker::Mount::ro(&run_dir.join("task-snapshot"), "/task"),
        docker::Mount::rw(&run_dir.join("progress"), "/progress"),
        docker::Mount::rw(&run_dir.join("agent-home"), "/agent-home"),
        docker::Mount::rw(&run_dir.join("out"), "/out"),
    ];
    let seed_dir = run_dir.join("seed");
    if seed_dir.is_dir() {
        mounts.push(docker::Mount::ro(&seed_dir, "/seed"));
    }
    let _ = resolved;

    let mut env = vec![
        ("TASKFMT_BASE".to_string(), base_ref.to_string()),
        ("AGENT_CMD".to_string(), agent_cmd.to_string()),
        ("AGENT_KIND".to_string(), profile.kind.clone()),
        ("HERDR_SESSION".to_string(), "agent".to_string()),
        ("NET_MODE".to_string(), "all".to_string()),
    ];
    for (key, value) in &profile.env_static {
        env.push((key.clone(), value.clone()));
    }
    match profile.kind.as_str() {
        "claude" => {
            env.push(("CLAUDE_CONFIG_DIR".to_string(), "/agent-home".to_string()));
            env.push((
                "CLAUDE_CODE_PROJECT_DIR_NAME".to_string(),
                "work".to_string(),
            ));
            env.push((
                "CLAUDE_CODE_EFFORT_LEVEL".to_string(),
                manifest.effort.clone(),
            ));
        }
        "codex" => {
            env.push(("CODEX_HOME".to_string(), "/agent-home".to_string()));
        }
        _ => {}
    }

    LaunchPlan {
        container: manifest.container.clone(),
        image: profile.image.clone(),
        mounts,
        env,
        memory: cfg.runtime.memory.clone(),
        cpus: cfg.runtime.cpus,
        pids_limit: cfg.runtime.pids_limit,
    }
}

/// Start the container. Persistent by hard rule: no `--rm`, so the operator can re-attach.
pub fn launch(plan: &LaunchPlan, env_file: &SecretEnvFile) -> anyhow::Result<String> {
    let spec = docker::RunSpec {
        name: plan.container.clone(),
        image: plan.image.clone(),
        mounts: plan.mounts.clone(),
        env: plan.env.clone(),
        env_file: env_file.path().map(Path::to_path_buf),
        memory: plan.memory.clone(),
        cpus: plan.cpus,
        pids_limit: plan.pids_limit,
    };
    docker::run_detached(&spec)
}

/// The agent command line for a claude profile.
pub fn claude_agent_cmd(session_id: &str, model: &str, effort: &str) -> String {
    format!(
        "claude --dangerously-skip-permissions --session-id {session_id} --add-dir /task --add-dir /progress --model {model} --effort {effort}"
    )
}

/// The agent command line for a codex profile (`-m` only when a model is pinned).
pub fn codex_agent_cmd(model: &str, effort: &str) -> String {
    let model_flag = if model.trim().is_empty() {
        String::new()
    } else {
        format!(" -m {}", model.trim())
    };
    format!(
        "codex --dangerously-bypass-approvals-and-sandbox --no-alt-screen -C /work --add-dir /task --add-dir /progress{model_flag} -c model_reasoning_effort=\"{effort}\""
    )
}

/// `settings.json` pre-seed so Claude Code starts without dialogs. Contains no key material.
pub fn claude_settings_json() -> &'static str {
    r#"{
  "skipDangerousModePermissionPrompt": true,
  "theme": "dark",
  "tui": "default",
  "cleanupPeriodDays": 3650,
  "env": {"CLAUDE_CODE_GOAL_CHECKIN_MINUTES": "0"}
}
"#
}

/// `.claude.json` pre-seed (onboarding + project trust). Deliberately holds no `customApiKeyResponses`
/// key material: the token travels by env only.
pub fn claude_project_json() -> &'static str {
    r#"{
  "hasCompletedOnboarding": true,
  "lastOnboardingVersion": "2.1.250",
  "numStartups": 1,
  "projects": {"/work": {"hasTrustDialogAccepted": true, "hasCompletedProjectOnboarding": true, "allowedTools": []}}
}
"#
}

/// codex `config.toml` pre-seed.
pub fn codex_config_toml() -> &'static str {
    "approval_policy = \"never\"\nsandbox_mode    = \"danger-full-access\"\n[features]\ngoals = true\n[projects.\"/work\"]\ntrust_level = \"trusted\"\n[notice]\nhide_full_access_warning = true\n[tui]\nshow_tooltips = false\nanimations = false\n"
}

/// Pre-seed `<agent-home>` for the profile kind.
pub fn preseed_agent_home(agent_home: &Path, kind: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(agent_home)?;
    match kind {
        "claude" => {
            super::write_file(&agent_home.join("settings.json"), claude_settings_json())?;
            super::write_file(&agent_home.join(".claude.json"), claude_project_json())?;
        }
        "codex" => {
            super::write_file(&agent_home.join("config.toml"), codex_config_toml())?;
        }
        other => anyhow::bail!("unknown agent kind: {other}"),
    }
    super::git::write_safe_directory(agent_home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_env_file_is_0600_and_removed_on_drop() {
        let path;
        {
            let file = SecretEnvFile::create(&[(
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                "tok-secret-abcdef".to_string(),
            )])
            .unwrap();
            path = file.path().unwrap().to_path_buf();
            let meta = std::fs::metadata(&path).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(meta.permissions().mode() & 0o777, 0o600);
            }
            let body = std::fs::read_to_string(&path).unwrap();
            assert_eq!(body, "ANTHROPIC_AUTH_TOKEN=tok-secret-abcdef\n");
        }
        assert!(!path.exists(), "env file must be gone after drop");
    }

    #[test]
    fn empty_secret_set_needs_no_file() {
        let file = SecretEnvFile::create(&[]).unwrap();
        assert!(file.path().is_none());
    }

    #[test]
    fn newline_in_secret_is_refused() {
        assert!(SecretEnvFile::create(&[("K".to_string(), "a\nb".to_string())]).is_err());
    }

    #[test]
    fn agent_command_lines() {
        let claude = claude_agent_cmd("sid", "glm-5.3-flash", "low");
        assert!(claude.contains("--session-id sid"));
        assert!(claude.contains("--add-dir /task"));
        assert!(claude.contains("--effort low"));
        assert!(claude.starts_with("claude --dangerously-skip-permissions"));
        let codex = codex_agent_cmd("", "high");
        assert!(codex.starts_with("codex --dangerously-bypass-approvals-and-sandbox"));
        assert!(
            !codex.contains(" -m "),
            "no model pin when the profile model is empty"
        );
        assert!(codex_agent_cmd("gpt-5", "high").contains(" -m gpt-5 "));
    }

    #[test]
    fn preseed_writes_config_and_gitconfig_without_key_material() {
        let dir = tempfile::tempdir().unwrap();
        preseed_agent_home(dir.path(), "claude").unwrap();
        let settings = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert!(settings.contains("skipDangerousModePermissionPrompt"));
        let project = std::fs::read_to_string(dir.path().join(".claude.json")).unwrap();
        assert!(project.contains("hasTrustDialogAccepted"));
        assert!(
            !project.contains("customApiKeyResponses"),
            "no key material in artifacts"
        );
        assert!(!settings.contains("API_KEY"));
        let gitconfig = std::fs::read_to_string(dir.path().join(".gitconfig")).unwrap();
        assert!(gitconfig.contains("directory = *"));
        let codex_dir = tempfile::tempdir().unwrap();
        preseed_agent_home(codex_dir.path(), "codex").unwrap();
        assert!(codex_dir.path().join("config.toml").is_file());
        assert!(preseed_agent_home(dir.path(), "ghost").is_err());
    }

    #[test]
    fn launch_plan_pins_the_scope_base_to_the_recorded_sha() {
        let cfg = ExperimentConfig::parse(
            "schema = \"experiment/v1\"\n[agents.default]\nprofile = \"p\"\n[agents.profiles.p]\nkind = \"claude\"\nimage = \"i\"\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let resolved = Resolved::new(dir.path(), cfg.clone());
        let profile = cfg.profile("p").unwrap().clone();
        let manifest = Manifest {
            run: "r".into(),
            run_dir: dir.path().display().to_string(),
            container: "harness-r".into(),
            agent: "p".into(),
            agent_kind: "claude".into(),
            model: "m".into(),
            effort: "low".into(),
            task: "TASK-001".into(),
            repo_url: "u".into(),
            base_sha: "0123456789abcdef0123456789abcdef01234567".into(),
            clone_sha: String::new(),
            session_id: "sid".into(),
            pane: String::new(),
            agent_name: "task".into(),
            start: String::new(),
            selfcheck: crate::runstate::SELFCHECK_NOT_RUN.into(),
            experiment: None,
            gate: None,
            status_state: String::new(),
            result_sha: None,
        };
        let plan = launch_plan(
            &cfg,
            &resolved,
            &manifest,
            &profile,
            "claude",
            &manifest.base_sha,
        );
        let base = plan
            .env
            .iter()
            .find(|(key, _)| key == "TASKFMT_BASE")
            .map(|(_, value)| value.as_str());
        assert_eq!(base, Some(manifest.base_sha.as_str()));
        assert_ne!(
            base,
            Some("baseline"),
            "the movable tag must not be the scope base"
        );
    }
}
