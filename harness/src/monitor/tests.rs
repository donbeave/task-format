use super::*;

fn fixture(statuses: &[(&str, &str, &[&str])]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let group = root.path().join("Jackin/new-design");
    fs::create_dir_all(&group).unwrap();
    fs::write(
        root.path().join("Jackin/README.md"),
        "# Jackin\n\nProject description",
    )
    .unwrap();
    fs::write(group.join("README.md"), "# New design\n\nGroup description").unwrap();
    for (number, status, dependencies) in statuses {
        let task = group.join(number);
        fs::create_dir(&task).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/README.md"),
            task.join("README.md"),
        )
        .unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/verify.toml"),
            task.join("verify.toml"),
        )
        .unwrap();
        fs::write(
            task.join("task.toml"),
            format!(
                "schema = \"task-meta/v1\"\nstatus = {status:?}\ndependencies = {dependencies:?}\n"
            ),
        )
        .unwrap();
    }
    root
}

#[test]
fn discovers_normalized_hierarchy_and_preserves_contract_identity() {
    let root = fixture(&[("001", "pending", &[])]);
    let catalog = Catalog::load(root.path()).unwrap();
    assert_eq!(catalog.projects["jackin"].name, "Jackin");
    assert_eq!(
        catalog.tasks["jackin/new-design/001"].contract_id,
        "TASK-042"
    );
    assert!(catalog.eligibility("jackin/new-design/001").allowed);
}

#[test]
fn rejects_invalid_ids_and_strict_metadata() {
    for id in [
        "../a/001",
        "a/b/..",
        "a/b/1",
        "A/b/001",
        "a/b/001/x",
        "a/b/%2e%2e",
        "a\\b/c/001",
    ] {
        assert!(validate_id(id).is_err(), "{id}");
    }
    for text in [
        "schema='task-meta/v1'\nstatus='blocked'\ndependencies=[]",
        "schema='task-meta/v2'\nstatus='pending'\ndependencies=[]",
        "schema='task-meta/v1'\nstatus='pending'\ndependencies=[]\nextra=true",
        "schema='task-meta/v1'\nstatus='pending'\ndependencies=['a/b/001','a/b/001']",
    ] {
        assert!(TaskMetadata::parse(text).is_err());
    }
}

#[test]
fn rejects_missing_self_and_cyclic_dependencies() {
    for tasks in [
        vec![("001", "pending", vec!["jackin/new-design/999"])],
        vec![("001", "pending", vec!["jackin/new-design/001"])],
        vec![
            ("001", "pending", vec!["jackin/new-design/002"]),
            ("002", "draft", vec!["jackin/new-design/001"]),
        ],
    ] {
        let entries: Vec<_> = tasks
            .iter()
            .map(|(n, s, d)| (*n, *s, d.as_slice()))
            .collect();
        let root = fixture(&entries);
        assert!(Catalog::load(root.path()).is_err());
    }
}

#[test]
fn parallel_dag_readiness_failure_and_success_follow_dependencies() {
    let root = fixture(&[
        ("001", "pending", &[]),
        ("002", "pending", &["jackin/new-design/001"]),
        ("003", "pending", &["jackin/new-design/001"]),
        (
            "004",
            "pending",
            &["jackin/new-design/002", "jackin/new-design/003"],
        ),
    ]);
    let mut catalog = Catalog::load(root.path()).unwrap();
    let ids = catalog
        .validate_scope(&Scope::Project("jackin".into()))
        .unwrap();
    assert_eq!(catalog.ready_tasks(&ids), ["jackin/new-design/001"]);
    catalog.start(&ids[0]).unwrap();
    assert!(catalog.start(&ids[0]).is_err());
    assert!(
        catalog
            .validate_scope(&Scope::Project("jackin".into()))
            .is_err()
    );
    catalog.finish(&ids[0], false).unwrap();
    assert_eq!(catalog.ready_tasks(&ids), ["jackin/new-design/001"]);
    catalog.start(&ids[0]).unwrap();
    catalog.finish(&ids[0], true).unwrap();
    assert_eq!(
        catalog.ready_tasks(&ids),
        ["jackin/new-design/002", "jackin/new-design/003"]
    );
    assert!(!catalog.eligibility(&ids[3]).allowed);
    let counts = catalog.aggregate(&ids);
    assert_eq!(
        (counts.done, counts.pending, counts.blocked, counts.progress),
        (1, 3, 1, 25.0)
    );
}

#[test]
fn drafts_are_visible_but_never_execute_or_block() {
    let root = fixture(&[
        ("001", "draft", &[]),
        ("002", "pending", &["jackin/new-design/001"]),
    ]);
    let mut catalog = Catalog::load(root.path()).unwrap();
    assert!(catalog.start("jackin/new-design/001").is_err());
    assert!(catalog.eligibility("jackin/new-design/002").allowed);
    assert_eq!(
        catalog
            .validate_scope(&Scope::Project("jackin".into()))
            .unwrap(),
        ["jackin/new-design/002"]
    );
}

#[test]
fn recovery_persists_pending_and_locks_release_on_drop() {
    let root = fixture(&[("001", "in_progress", &[])]);
    let mut catalog = Catalog::load(root.path()).unwrap();
    catalog.recover("jackin/new-design/001", false).unwrap();
    assert_eq!(
        Catalog::load(root.path()).unwrap().tasks["jackin/new-design/001"]
            .metadata
            .status,
        Status::Pending
    );
    let lock = RunLock::acquire(root.path()).unwrap();
    assert!(RunLock::acquire(root.path()).is_err());
    drop(lock);
    assert!(RunLock::acquire(root.path()).is_ok());
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_and_normalized_duplicate_directories() {
    let root = fixture(&[("001", "pending", &[])]);
    std::os::unix::fs::symlink(root.path().join("Jackin"), root.path().join("escape")).unwrap();
    assert!(
        Catalog::load(root.path())
            .unwrap_err()
            .to_string()
            .contains("symlink")
    );
    fs::remove_file(root.path().join("escape")).unwrap();
    // Case-insensitive filesystems themselves prohibit the conflicting directory.
    if let Err(error) = fs::create_dir(root.path().join("JACKIN")) {
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        return;
    }
    fs::write(root.path().join("JACKIN/README.md"), "# Duplicate").unwrap();
    assert!(Catalog::load(root.path()).is_err());
}

#[test]
fn percentage_uses_event_leaves_and_done_override() {
    let task =
        TaskFile::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/README.md"))
            .unwrap();
    let summary = ProgressSummary::from_task(&task, Status::Pending, None);
    assert!(summary.total > 0);
    assert_eq!(summary.percentage, 0.0);
    assert_eq!(
        ProgressSummary::from_task(&task, Status::Done, None).percentage,
        100.0
    );
    let initial =
        crate::progress::generate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example"))
            .unwrap();
    let progress = crate::progress::ProgressFile::parse(&initial.body, &task).unwrap();
    assert_eq!(
        ProgressSummary::from_task(&task, Status::Pending, Some(&progress))
            .current_leaf
            .as_deref(),
        Some(initial.first_leaf.as_str())
    );
}

#[test]
fn partial_progress_reopening_and_failure_are_derived_from_actual_events() {
    let task =
        TaskFile::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/README.md"))
            .unwrap();
    let items = crate::taskfile::parse_checklist(&task.checklist);
    let leaves: Vec<_> = items
        .iter()
        .zip(crate::taskfile::leaf_flags(&items))
        .filter(|(_, leaf)| *leaf)
        .map(|(item, _)| item.id.as_str())
        .collect();
    assert!(leaves.len() >= 2);
    let first = leaves[0];
    let second = leaves[1];
    let text = format!(
        "---\nschema: progress/v1\ntask: {}\nstate: IN_PROGRESS\ncurrent: {second}\nlatest_event: 3\n---\n\n## Events\n- 1 | STARTED | {first}\n- 2 | DONE | {first}\n- 3 | STARTED | {second}\n\n## Handoff\nCURRENT_FAILURE: none\n",
        task.id()
    );
    let progress = crate::progress::ProgressFile::parse(&text, &task).unwrap();
    let summary = ProgressSummary::from_task(&task, Status::InProgress, Some(&progress));
    assert_eq!(summary.completed, 1);
    assert_eq!(summary.percentage, 100.0 / leaves.len() as f64);
    assert_eq!(summary.current_leaf.as_deref(), Some(second));
    assert_eq!(
        summary.latest_event,
        Some(format!("3 | Started | {second}"))
    );
    assert_eq!(summary.current_failure, None);
    let reopened = format!(
        "---\nschema: progress/v1\ntask: {}\nstate: IN_PROGRESS\ncurrent: {first}\nlatest_event: 3\n---\n\n## Events\n- 1 | STARTED | {first}\n- 2 | DONE | {first}\n- 3 | REOPENED | {first}\n\n## Handoff\nCURRENT_FAILURE: regression discovered\n",
        task.id()
    );
    let progress = crate::progress::ProgressFile::parse(&reopened, &task).unwrap();
    let summary = ProgressSummary::from_task(&task, Status::InProgress, Some(&progress));
    assert_eq!((summary.completed, summary.percentage), (0, 0.0));
    assert_eq!(
        summary.current_failure.as_deref(),
        Some("regression discovered")
    );
    assert_eq!(
        summary.latest_event,
        Some(format!("3 | Reopened | {first}"))
    );
}

#[test]
fn atomic_write_replaces_whole_file_without_temporary_debris() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("state");
    atomic_write(&path, b"old").unwrap();
    atomic_write(&path, b"new complete state").unwrap();
    assert_eq!(fs::read(path).unwrap(), b"new complete state");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn rejects_malformed_verifier_before_task_becomes_eligible() {
    let root = fixture(&[("001", "pending", &[])]);
    fs::write(
        root.path().join("Jackin/new-design/001/verify.toml"),
        "schema='verify/v2'\n",
    )
    .unwrap();
    assert!(Catalog::load(root.path()).is_err());
}

#[cfg(unix)]
#[test]
fn package_links_may_resolve_inside_package_but_never_escape() {
    let root = fixture(&[("001", "pending", &[])]);
    let task = root.path().join("Jackin/new-design/001");
    fs::write(task.join("AGENTS.md"), "fixture").unwrap();
    std::os::unix::fs::symlink("AGENTS.md", task.join("CLAUDE.md")).unwrap();
    Catalog::load(root.path()).unwrap();
    std::os::unix::fs::symlink(
        root.path().join("Jackin/README.md"),
        task.join("outside.md"),
    )
    .unwrap();
    assert!(
        Catalog::load(root.path())
            .unwrap_err()
            .to_string()
            .contains("escapes package")
    );
}

#[test]
fn group_scope_cannot_wait_for_an_unfinished_external_dependency() {
    let root = fixture(&[("001", "pending", &[])]);
    let second = root.path().join("other/build");
    fs::create_dir_all(&second).unwrap();
    fs::write(root.path().join("other/README.md"), "# Other").unwrap();
    fs::write(second.join("README.md"), "# Build").unwrap();
    let source = root.path().join("Jackin/new-design/001");
    let task = second.join("001");
    fs::create_dir(&task).unwrap();
    for file in ["README.md", "verify.toml"] {
        fs::copy(source.join(file), task.join(file)).unwrap();
    }
    fs::write(
        task.join("task.toml"),
        "schema='task-meta/v1'\nstatus='pending'\ndependencies=['jackin/new-design/001']",
    )
    .unwrap();
    let catalog = Catalog::load(root.path()).unwrap();
    assert!(!catalog.eligibility("other/build/001").allowed);
    assert!(
        catalog
            .validate_scope(&Scope::Project("other".into()))
            .is_err()
    );
    assert_eq!(
        catalog.topological_order().unwrap(),
        ["jackin/new-design/001", "other/build/001"]
    );
}
