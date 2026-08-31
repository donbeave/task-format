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

/// The run id (`manifest.run`).
pub const LABEL_RUN_ID: &str = "taskfmt.run_id";
/// The run directory on the host, absolute.
pub const LABEL_RUN_DIR: &str = "taskfmt.run_dir";
/// The absolute path of the `experiment.toml` this run was dispatched with.
pub const LABEL_MANIFEST: &str = "taskfmt.manifest";
/// The task id.
pub const LABEL_TASK: &str = "taskfmt.task";
/// The agent profile name from that manifest.
pub const LABEL_PROFILE: &str = "taskfmt.profile";
/// The experiment id, when the run belongs to one.
pub const LABEL_EXP: &str = "taskfmt.exp";

/// The labels that make one run's container self-describing: run id, run dir, the manifest that
/// dispatched it, task, profile, and the experiment when there is one.
///
/// A container carrying these can be asked what it is without any file on the host being readable
/// or any manifest being discoverable — which is the whole point: the run's identity belongs to the
/// run, not to the directory an operator happens to be standing in.
pub fn run_labels(manifest: &Manifest, manifest_path: &Path) -> Vec<(String, String)> {
    let mut labels = vec![
        (LABEL_RUN_ID.to_string(), manifest.run.clone()),
        (
            LABEL_RUN_DIR.to_string(),
            absolute(Path::new(&manifest.run_dir)),
        ),
        (LABEL_MANIFEST.to_string(), absolute(manifest_path)),
        (LABEL_TASK.to_string(), manifest.task.clone()),
        (LABEL_PROFILE.to_string(), manifest.agent.clone()),
    ];
    if let Some(exp) = manifest.experiment.as_ref().filter(|id| !id.is_empty()) {
        labels.push((LABEL_EXP.to_string(), exp.clone()));
    }
    labels
}

/// A path as a string, made absolute where the process can (a label read from another machine's
/// working directory is worthless).
fn absolute(path: &Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

/// The run directory a container belongs to: its [`LABEL_RUN_DIR`] label, else the parent of its
/// `/work` bind mount. The second branch is what makes containers launched before these labels
/// existed still locatable.
pub fn run_dir_of(info: &docker::ContainerInfo) -> Option<PathBuf> {
    if let Some(dir) = info.label(LABEL_RUN_DIR) {
        return Some(PathBuf::from(dir));
    }
    info.work_mount
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// The manifest that dispatched the run, when the container names one that is still on disk.
pub fn manifest_of(info: &docker::ContainerInfo) -> Option<PathBuf> {
    info.label(LABEL_MANIFEST)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

/// The run id: the [`LABEL_RUN_ID`] label, else the container name with [`CONTAINER_PREFIX`]
/// stripped (the naming rule this harness has always followed).
pub fn run_id_of(info: &docker::ContainerInfo) -> String {
    match info.label(LABEL_RUN_ID) {
        Some(id) => id.to_string(),
        None => info
            .name
            .strip_prefix(CONTAINER_PREFIX)
            .unwrap_or(&info.name)
            .to_string(),
    }
}

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
    /// [`run_labels`] — the run's identity, carried by the container itself.
    pub labels: Vec<(String, String)>,
    pub memory: String,
    pub cpus: f32,
    pub pids_limit: i64,
}

/// Build the launch plan for one run: mounts `/work /task:ro /progress /agent-home /out /seed:ro`,
/// the static env from the profile plus `TASKFMT_BASE`, `AGENT_CMD`, `AGENT_KIND`, `HERDR_SESSION`,
/// and the `taskfmt.*` labels that let every later command find this run from the container alone.
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
        docker::Mount::rw(&run_dir.join("workspace"), docker::WORK_MOUNT),
        docker::Mount::ro(&run_dir.join("task-snapshot"), "/task"),
        docker::Mount::rw(&run_dir.join("progress"), "/progress"),
        docker::Mount::rw(&run_dir.join("agent-home"), "/agent-home"),
        docker::Mount::rw(&run_dir.join("out"), "/out"),
    ];
    let seed_dir = run_dir.join("seed");
    if seed_dir.is_dir() {
        mounts.push(docker::Mount::ro(&seed_dir, "/seed"));
    }
    let labels = run_labels(manifest, &resolved.manifest);

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
        labels,
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
        labels: plan.labels.clone(),
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
/// `extraKnownMarketplaces` + `enabledPlugins` carry the rust-analyzer-lsp plugin into every
/// per-run config; the plugins/ tree itself is copied from the image's /opt/claude-plugin-seed
/// by the container entrypoint (the per-run /agent-home bind mount masks the baked copy).
pub fn claude_settings_json() -> &'static str {
    r#"{
  "skipDangerousModePermissionPrompt": true,
  "theme": "dark",
  "tui": "default",
  "cleanupPeriodDays": 3650,
  "env": {"CLAUDE_CODE_GOAL_CHECKIN_MINUTES": "0"},
  "extraKnownMarketplaces": {
    "claude-plugins-official": {
      "source": {"source": "git", "url": "https://github.com/anthropics/claude-plugins-official.git"}
    }
  },
  "enabledPlugins": {"rust-analyzer-lsp@claude-plugins-official": true}
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
        assert!(
            settings.contains("extraKnownMarketplaces")
                && settings.contains("claude-plugins-official")
                && settings.contains("\"rust-analyzer-lsp@claude-plugins-official\": true"),
            "the seeded settings must enable the rust-analyzer-lsp plugin"
        );
        let parsed: serde_json::Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(
            parsed["extraKnownMarketplaces"]["claude-plugins-official"]["source"]["source"],
            "git"
        );
        assert_eq!(
            parsed["enabledPlugins"]["rust-analyzer-lsp@claude-plugins-official"],
            true
        );
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

    /// A manifest with every field the label set reads.
    fn sample_manifest(run_dir: &Path) -> Manifest {
        Manifest {
            run: "20260101-000000-p-TASK-001".into(),
            run_dir: run_dir.display().to_string(),
            container: "harness-20260101-000000-p-TASK-001".into(),
            agent: "zai-flash".into(),
            agent_kind: "claude".into(),
            model: "m".into(),
            effort: "high".into(),
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
        }
    }

    #[test]
    fn run_labels_carry_the_runs_whole_identity() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path().join("runs/20260101-000000-p-TASK-001");
        let manifest_path = dir.path().join("experiment.toml");
        let mut manifest = sample_manifest(&run_dir);
        let labels = run_labels(&manifest, &manifest_path);
        let get = |key: &str| {
            labels
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert_eq!(get(LABEL_RUN_ID), "20260101-000000-p-TASK-001");
        assert_eq!(get(LABEL_RUN_DIR), run_dir.display().to_string());
        assert_eq!(get(LABEL_MANIFEST), manifest_path.display().to_string());
        assert_eq!(get(LABEL_TASK), "TASK-001");
        assert_eq!(get(LABEL_PROFILE), "zai-flash");
        assert!(
            !labels.iter().any(|(k, _)| k == LABEL_EXP),
            "no experiment id, no label"
        );
        // a run inside an experiment carries it too
        manifest.experiment = Some("exp-20260101-000000".into());
        assert_eq!(
            run_labels(&manifest, &manifest_path)
                .iter()
                .find(|(k, _)| k == LABEL_EXP)
                .map(|(_, v)| v.clone()),
            Some("exp-20260101-000000".to_string())
        );
        // and no label may carry key material: the label set is manifest fields and paths only
        assert!(!labels.iter().any(|(_, v)| v.contains("op://")));
    }

    #[test]
    fn a_labelled_container_answers_without_touching_a_mount() {
        let mut labels = std::collections::BTreeMap::new();
        labels.insert(LABEL_RUN_ID.to_string(), "r".to_string());
        labels.insert(LABEL_RUN_DIR.to_string(), "/runs/r".to_string());
        labels.insert(
            LABEL_MANIFEST.to_string(),
            "/nowhere/experiment.toml".into(),
        );
        let info = docker::ContainerInfo {
            name: "harness-r".into(),
            state: "running".into(),
            // deliberately disagreeing with the label: the label is the run's own statement
            work_mount: Some(PathBuf::from("/elsewhere/other/workspace")),
            labels,
        };
        assert_eq!(run_dir_of(&info), Some(PathBuf::from("/runs/r")));
        assert_eq!(run_id_of(&info), "r");
        assert_eq!(
            manifest_of(&info),
            None,
            "a manifest label pointing at nothing on disk is not a manifest"
        );
    }

    #[test]
    fn a_pre_label_container_is_located_by_its_work_mount() {
        // the shape of the containers this change was written against: `docker inspect` shows `{}`
        let info = docker::ContainerInfo {
            name: "harness-20260829-194052-zai-flash-TASK-002".into(),
            state: "running".into(),
            work_mount: Some(PathBuf::from(
                "/runs/20260829-194052-zai-flash-TASK-002/workspace",
            )),
            labels: std::collections::BTreeMap::new(),
        };
        assert_eq!(
            run_dir_of(&info),
            Some(PathBuf::from("/runs/20260829-194052-zai-flash-TASK-002"))
        );
        assert_eq!(run_id_of(&info), "20260829-194052-zai-flash-TASK-002");
        assert_eq!(manifest_of(&info), None);
        // nothing at all to go on: no run dir, but still a run id from the name
        let bare = docker::ContainerInfo {
            name: "harness-r".into(),
            state: "exited".into(),
            ..Default::default()
        };
        assert_eq!(run_dir_of(&bare), None);
        assert_eq!(run_id_of(&bare), "r");
    }

    #[test]
    fn a_manifest_label_that_exists_is_returned() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("experiment.toml");
        std::fs::write(&manifest_path, "schema = \"experiment/v1\"\n").unwrap();
        let mut labels = std::collections::BTreeMap::new();
        labels.insert(
            LABEL_MANIFEST.to_string(),
            manifest_path.display().to_string(),
        );
        let info = docker::ContainerInfo {
            name: "harness-r".into(),
            labels,
            ..Default::default()
        };
        assert_eq!(manifest_of(&info), Some(manifest_path));
    }

    #[test]
    fn the_launch_plan_labels_the_container_with_its_run() {
        let cfg = ExperimentConfig::parse(
            "schema = \"experiment/v1\"\n[agents.default]\nprofile = \"p\"\n[agents.profiles.p]\nkind = \"claude\"\nimage = \"i\"\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let resolved = Resolved::new(dir.path(), cfg.clone());
        let profile = cfg.profile("p").unwrap().clone();
        let manifest = sample_manifest(&dir.path().join("runs/20260101-000000-p-TASK-001"));
        let plan = launch_plan(&cfg, &resolved, &manifest, &profile, "claude", "base");
        let label = |key: &str| {
            plan.labels
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(label(LABEL_RUN_ID), Some(manifest.run.clone()));
        assert_eq!(
            label(LABEL_MANIFEST),
            Some(dir.path().join("experiment.toml").display().to_string()),
            "the container names the manifest that dispatched it"
        );
        // the /work mount and the run-dir label must agree: both halves of the locator read them
        let work = plan
            .mounts
            .iter()
            .find(|mount| mount.container == docker::WORK_MOUNT)
            .expect("a /work mount");
        assert_eq!(
            work.host.parent().map(|p| p.display().to_string()),
            label(LABEL_RUN_DIR)
        );
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
