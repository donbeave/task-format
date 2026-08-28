//! Config, selection, taskfile and CLI surface.

use clap::Parser as _;
use taskfmt::cli::Cli;
use taskfmt::config::ExperimentConfig;

const MANIFEST: &str = r#"
schema = "experiment/v1"
[github]
owner = "donbeave"
repo_prefix = "taskfmt-experiment"
[runtime]
memory = "4g"
cpus = 2
pids_limit = 2048
prereq_timeout_s = 180
kill_after_min = 90
[agents.default]
profile = "zai-flash"
[agents.profiles.zai-flash]
kind = "claude"
model = "glm-5.3-flash"
effort = "low"
image = "harness-claude:latest"
[agents.profiles.zai-flash.env_static]
ANTHROPIC_BASE_URL = "https://api.z.ai/api/anthropic"
[agents.profiles.zai-flash.env_secret]
ANTHROPIC_AUTH_TOKEN = "op://vault/item/section/field"
[agents.profiles.codex-default]
kind = "codex"
model = ""
effort = "high"
image = "harness-codex:latest"
[agents.profiles.codex-default.env_secret]
OPENAI_API_KEY = "op://vault/item/section/field"
"#;

#[test]
fn manifest_matches_the_repo_root_file() {
    let on_disk = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../experiment.toml"),
    )
    .expect("experiment.toml at the repo root");
    let cfg = ExperimentConfig::parse(&on_disk).unwrap();
    assert_eq!(cfg.github.owner, "donbeave");
    assert_eq!(cfg.github.repo_prefix, "taskfmt-experiment");
    assert_eq!(cfg.default_profile(), "zai-flash");
    let zai = cfg.profile("zai-flash").unwrap();
    assert_eq!(zai.kind, "claude");
    assert_eq!(zai.model, "glm-5.3-flash");
    assert_eq!(zai.effort, "low");
    let codex = cfg.profile("codex-default").unwrap();
    assert_eq!(codex.kind, "codex");
    // only references are committed, never values
    assert!(zai.env_secret.values().all(|v| v.starts_with("op://")));
    assert!(codex.env_secret.values().all(|v| v.starts_with("op://")));
}

#[test]
fn manifest_validation() {
    assert!(ExperimentConfig::parse(MANIFEST).is_ok());
    let bad_schema = MANIFEST.replace("experiment/v1", "experiment/v9");
    assert!(ExperimentConfig::parse(&bad_schema).is_err());
    let no_agents = "schema = \"experiment/v1\"\n";
    assert!(ExperimentConfig::parse(no_agents).is_err());
    let unknown_default = MANIFEST.replace("profile = \"zai-flash\"", "profile = \"ghost\"");
    assert!(ExperimentConfig::parse(&unknown_default).is_err());
    let bad_kind = MANIFEST.replace("kind = \"codex\"", "kind = \"ghost\"");
    assert!(ExperimentConfig::parse(&bad_kind).is_err());
}

#[test]
fn selection_semantics() {
    let dir = tempfile::tempdir().unwrap();
    for n in [101u32, 102, 103, 104] {
        let path = dir.path().join(format!("TASK-{n}"));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("README.md"), "x").unwrap();
    }
    let tasks_dir = dir.path();
    let one = |tokens: &[&str]| -> Vec<String> {
        let owned: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
        taskfmt::selection::resolve(&owned, tasks_dir).unwrap()
    };

    assert_eq!(
        one(&["all"]),
        vec!["TASK-101", "TASK-102", "TASK-103", "TASK-104"]
    );
    assert_eq!(one(&["TASK-102"]), vec!["TASK-102"]);
    assert_eq!(
        one(&["task-102"]),
        vec!["TASK-102"],
        "ids are case-insensitive"
    );
    assert_eq!(one(&["1-3"]), vec!["TASK-101", "TASK-102", "TASK-103"]);
    assert_eq!(
        one(&["TASK-102..TASK-104"]),
        vec!["TASK-102", "TASK-103", "TASK-104"]
    );
    assert_eq!(
        one(&["TASK-103", "1-2"]),
        vec!["TASK-101", "TASK-102", "TASK-103"]
    );
    assert!(
        taskfmt::selection::resolve(&["TASK-999".to_string()], tasks_dir).is_ok(),
        "ids need not exist yet"
    );
    assert!(
        taskfmt::selection::resolve(&["7".to_string()], tasks_dir).is_err(),
        "position 7 does not exist"
    );

    let done = vec!["TASK-101".to_string()];
    assert_eq!(
        taskfmt::selection::skip_completed(&one(&["all"]), &done),
        vec!["TASK-102", "TASK-103", "TASK-104"]
    );
}

#[test]
fn every_cli_subcommand_parses() {
    let global = ["--config", "experiment.toml", "--auto"];
    let cases: Vec<Vec<&str>> = vec![
        vec!["taskfmt", "lint"],
        vec!["taskfmt", "lint", "TASK-101"],
        vec![
            "taskfmt",
            "progress-init",
            "TASK-101",
            "--out",
            "/tmp/progress.md",
        ],
        vec!["taskfmt", "selftest"],
        vec!["taskfmt", "verify"],
        vec!["taskfmt", "verify", "--fail-fast", "--log-dir", "/tmp/logs"],
        vec!["taskfmt", "build-images", "--agent", "all", "--no-cache"],
        vec!["taskfmt", "preload"],
        vec!["taskfmt", "repo", "create"],
        vec!["taskfmt", "repo", "delete", "--yes"],
        vec![
            "taskfmt",
            "run",
            "--task",
            "TASK-101",
            "--wait",
            "--kill-after",
            "30",
        ],
        vec!["taskfmt", "gate", "20260101-000000-zai-flash-TASK-101"],
        vec!["taskfmt", "promote", "some-run", "--yes"],
        vec![
            "taskfmt",
            "status",
            "some-run",
            "--wait",
            "--kill-after",
            "30",
        ],
        vec!["taskfmt", "attach", "some-run"],
        vec![
            "taskfmt",
            "experiment",
            "--tasks",
            "all,1-3,TASK-101",
            "--resume",
            "exp-1",
        ],
        vec!["taskfmt", "container-entrypoint"],
        vec!["taskfmt", "prereqs"],
        vec!["taskfmt", "agent-launch"],
    ];
    for mut case in cases {
        case.extend_from_slice(&global);
        let parsed = Cli::try_parse_from(&case);
        assert!(parsed.is_ok(), "{case:?}: {parsed:?}");
    }
    // mutating commands demand an explicit answer: without --auto/--yes and a TTY they must
    // refuse (covered in the interactive tests); here only the parse surface matters
    assert!(
        Cli::try_parse_from(["taskfmt"]).is_err(),
        "bare taskfmt is not a command"
    );
}

#[test]
fn taskfile_grammar() {
    let text = [
        "---",
        "schema: task/v4",
        "id: TASK-007",
        r#"title: "Reject expired tokens""#,
        "kind: test",
        r#"verify: "taskfmt verify""#,
        "expected_paths:",
        r#"  - "src/*""#,
        "---",
        "",
        "# TASK-007 — Reject expired tokens",
        "",
        "## Preconditions",
        "",
        "- **P-001:** ok — `true`",
        "",
        "## Checklist",
        "",
        "<!-- checklist:start -->",
        "- [ ] **1** a",
        "    - [ ] **1.1** b — evidence: `true`.",
        "    - [ ] **1.2** c — evidence: `true`.",
        "- [ ] **2** d",
        "    - [ ] **2.1** e — evidence: `true`.",
        "<!-- checklist:end -->",
        "",
    ]
    .join("\n");
    let tf = taskfmt::taskfile::TaskFile::parse(text, std::path::Path::new("README.md")).unwrap();
    assert_eq!(tf.preconditions.len(), 1);
    let items = taskfmt::taskfile::parse_checklist(&tf.checklist);
    assert_eq!(items.len(), 5);
    assert!(items.iter().all(|item| item.well_formed), "{items:?}");
    assert_eq!(
        taskfmt::taskfile::first_leaf(&items).as_deref(),
        Some("1.1")
    );
    let leaves = taskfmt::taskfile::leaf_flags(&items);
    assert_eq!(
        items
            .iter()
            .zip(leaves)
            .filter(|(_, leaf)| *leaf)
            .map(|(item, _)| item.id.clone())
            .collect::<Vec<_>>(),
        vec!["1.1", "1.2", "2.1"]
    );
    // four spaces per level, ids match depth
    assert_eq!(items[2].depth, 1);
    assert_eq!(items[2].indent, 4);
    assert_eq!(
        items[2].normalized(),
        "    - [ ] **1.2** c — evidence: `true`."
    );
}
