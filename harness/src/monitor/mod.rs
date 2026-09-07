//! Filesystem catalog and dependency-aware execution policy.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::taskfile::{self, TaskFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Pending,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskMetadata {
    pub schema: String,
    pub status: Status,
    pub dependencies: Vec<String>,
}

impl TaskMetadata {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let metadata: Self = toml::from_str(text).context("invalid task metadata")?;
        ensure!(
            metadata.schema == "task-meta/v1",
            "unsupported task metadata schema"
        );
        let mut unique = BTreeSet::new();
        for dependency in &metadata.dependencies {
            validate_id(dependency)?;
            ensure!(
                unique.insert(dependency),
                "duplicate dependency: {dependency}"
            );
        }
        Ok(metadata)
    }
}

pub fn validate_code(code: &str) -> anyhow::Result<()> {
    ensure!(
        !code.is_empty()
            && code.len() <= 80
            && code
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && code.as_bytes()[0].is_ascii_alphanumeric()
            && code.as_bytes()[code.len() - 1].is_ascii_alphanumeric(),
        "invalid canonical code: {code}"
    );
    Ok(())
}

pub fn validate_id(id: &str) -> anyhow::Result<()> {
    let parts: Vec<_> = id.split('/').collect();
    ensure!(parts.len() == 3, "invalid task ID: {id}");
    validate_code(parts[0])?;
    validate_code(parts[1])?;
    ensure!(
        parts[2].len() >= 3 && parts[2].len() <= 12 && parts[2].bytes().all(|b| b.is_ascii_digit()),
        "invalid task number: {}",
        parts[2]
    );
    Ok(())
}

/// Reject symlinks at every component beneath the configured root.
pub fn safe_path(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let relative = path
        .strip_prefix(root)
        .context("path outside configured root")?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        ensure!(
            matches!(component, std::path::Component::Normal(_)),
            "invalid path component"
        );
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("cannot inspect {}", current.display()))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "symlink forbidden: {}",
            current.display()
        );
    }
    ensure!(
        current.canonicalize()?.starts_with(root),
        "path escapes configured root"
    );
    Ok(current)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProgressSummary {
    pub completed: usize,
    pub total: usize,
    pub percentage: f64,
    pub current_leaf: Option<String>,
    pub current_failure: Option<String>,
    pub run_state: Option<String>,
    pub latest_event: Option<String>,
}

impl ProgressSummary {
    pub fn from_task(
        task: &TaskFile,
        status: Status,
        progress: Option<&crate::progress::ProgressFile>,
    ) -> Self {
        let items = taskfile::parse_checklist(&task.checklist);
        let leaves: BTreeSet<_> = items
            .iter()
            .zip(taskfile::leaf_flags(&items))
            .filter(|(_, leaf)| *leaf)
            .map(|(item, _)| item.id.as_str())
            .collect();
        let total = leaves.len();
        let progress = progress.filter(|p| p.task == task.id());
        let completed = progress.map_or(0, |p| {
            p.completed
                .iter()
                .filter(|id| leaves.contains(id.as_str()))
                .count()
        });
        Self {
            completed,
            total,
            percentage: if status == Status::Done {
                100.0
            } else if total == 0 {
                0.0
            } else {
                100.0 * completed as f64 / total as f64
            },
            current_leaf: progress.and_then(|p| p.current.clone()),
            current_failure: progress.and_then(|p| {
                p.handoff.iter().find_map(|line| {
                    line.strip_prefix("CURRENT_FAILURE: ")
                        .filter(|s| *s != "none")
                        .map(str::to_owned)
                })
            }),
            run_state: progress.map(|p| p.state.as_str().to_owned()),
            latest_event: progress.and_then(|p| {
                p.events
                    .last()
                    .map(|e| format!("{} | {:?} | {}", e.sequence, e.status, e.leaf))
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRecord {
    pub id: String,
    pub project_code: String,
    pub group_code: String,
    pub number: String,
    pub title: String,
    pub contract_id: String,
    pub readme: String,
    pub metadata: TaskMetadata,
    pub dependents: Vec<String>,
    pub progress: ProgressSummary,
    #[serde(skip)]
    pub path: PathBuf,
}
#[derive(Debug, Clone, Serialize)]
pub struct ProjectRecord {
    pub code: String,
    pub name: String,
    pub readme: String,
    pub groups: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GroupRecord {
    pub id: String,
    pub project_code: String,
    pub code: String,
    pub name: String,
    pub readme: String,
    pub tasks: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    #[serde(skip)]
    pub root: PathBuf,
    pub projects: BTreeMap<String, ProjectRecord>,
    pub groups: BTreeMap<String, GroupRecord>,
    pub tasks: BTreeMap<String, TaskRecord>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Eligibility {
    pub allowed: bool,
    pub reason: Option<String>,
}
impl Eligibility {
    fn denied(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.into()),
        }
    }
    fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }
}
#[derive(Debug, Clone)]
pub enum Scope {
    Task(String),
    Group { project: String, group: String },
    Project(String),
}
#[derive(Debug, Default, Clone, Serialize)]
pub struct Aggregate {
    pub task_count: usize,
    pub draft: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub done: usize,
    pub blocked: usize,
    pub progress: f64,
}

fn directories(root: &Path, parent: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(parent)? {
        let path = entry?.path();
        safe_path(root, &path)?;
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}
fn directory_name(path: &Path) -> anyhow::Result<String> {
    Ok(path
        .file_name()
        .and_then(|v| v.to_str())
        .context("non-UTF8 catalog directory")?
        .to_owned())
}
fn description(root: &Path, path: &Path) -> anyhow::Result<(String, String)> {
    let text = fs::read_to_string(safe_path(root, &path.join("README.md"))?)?;
    let name = text
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .filter(|s| !s.trim().is_empty())
        .context("README requires a nonempty H1 display name")?
        .trim()
        .to_owned();
    Ok((name, text))
}

impl Catalog {
    /// Load one canonical hierarchy. Any malformed package invalidates the catalog.
    pub fn load(root: &Path) -> anyhow::Result<Self> {
        ensure!(
            !fs::symlink_metadata(root)?.file_type().is_symlink(),
            "task root cannot be a symlink"
        );
        let root = root.canonicalize()?;
        let mut catalog = Self {
            root: root.clone(),
            projects: BTreeMap::new(),
            groups: BTreeMap::new(),
            tasks: BTreeMap::new(),
        };
        for project_path in directories(&root, &root)? {
            let code = directory_name(&project_path)?.to_ascii_lowercase();
            validate_code(&code)?;
            let (name, readme) = description(&root, &project_path)?;
            let mut project = ProjectRecord {
                code: code.clone(),
                name,
                readme,
                groups: Vec::new(),
            };
            for group_path in directories(&root, &project_path)? {
                let group_code = directory_name(&group_path)?.to_ascii_lowercase();
                validate_code(&group_code)?;
                let id = format!("{code}/{group_code}");
                let (name, readme) = description(&root, &group_path)?;
                let mut group = GroupRecord {
                    id: id.clone(),
                    project_code: code.clone(),
                    code: group_code.clone(),
                    name,
                    readme,
                    tasks: Vec::new(),
                };
                for task_path in directories(&root, &group_path)? {
                    for entry in walkdir::WalkDir::new(&task_path).follow_links(false) {
                        let entry = entry?;
                        let resolved = entry.path().canonicalize()?;
                        ensure!(
                            resolved.starts_with(&task_path),
                            "task package symlink escapes package: {}",
                            entry.path().display()
                        );
                    }
                    let number = directory_name(&task_path)?;
                    let task_id = format!("{id}/{number}");
                    validate_id(&task_id)?;
                    let readme_path = safe_path(&root, &task_path.join("README.md"))?;
                    safe_path(&root, &task_path.join("verify.toml"))?;
                    let report = crate::lint::lint_path(&task_path);
                    ensure!(report.passed(), "invalid task package: {}", report.render());
                    let metadata = TaskMetadata::parse(&fs::read_to_string(safe_path(
                        &root,
                        &task_path.join("task.toml"),
                    )?)?)?;
                    let readme = fs::read_to_string(&readme_path)?;
                    let task = TaskFile::parse(readme.clone(), &readme_path)?;
                    let progress = ProgressSummary::from_task(&task, metadata.status, None);
                    let record = TaskRecord {
                        id: task_id.clone(),
                        project_code: code.clone(),
                        group_code: group_code.clone(),
                        number,
                        title: task.frontmatter.title.clone(),
                        contract_id: task.id().to_owned(),
                        readme,
                        metadata,
                        dependents: Vec::new(),
                        progress,
                        path: task_path,
                    };
                    ensure!(
                        catalog.tasks.insert(task_id.clone(), record).is_none(),
                        "duplicate task ID: {task_id}"
                    );
                    group.tasks.push(task_id);
                }
                ensure!(
                    catalog.groups.insert(id.clone(), group).is_none(),
                    "duplicate group ID: {id}"
                );
                project.groups.push(id);
            }
            ensure!(
                catalog.projects.insert(code.clone(), project).is_none(),
                "duplicate project code: {code}"
            );
        }
        let edges: Vec<_> = catalog
            .tasks
            .values()
            .flat_map(|task| {
                task.metadata
                    .dependencies
                    .iter()
                    .map(|dependency| (task.id.clone(), dependency.clone()))
            })
            .collect();
        for (id, dependency) in edges {
            ensure!(id != dependency, "self dependency: {id}");
            let target = catalog
                .tasks
                .get_mut(&dependency)
                .with_context(|| format!("missing dependency {dependency} for {id}"))?;
            target.dependents.push(id);
        }
        catalog.topological_order()?;
        Ok(catalog)
    }

    pub fn topological_order(&self) -> anyhow::Result<Vec<String>> {
        let mut incoming: BTreeMap<_, _> = self
            .tasks
            .iter()
            .map(|(id, t)| (id.clone(), t.metadata.dependencies.len()))
            .collect();
        let mut ready: BTreeSet<_> = incoming
            .iter()
            .filter(|(_, n)| **n == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut order = Vec::new();
        while let Some(id) = ready.pop_first() {
            for dependent in &self.tasks[&id].dependents {
                let count = incoming
                    .get_mut(dependent)
                    .context("invalid dependency graph")?;
                *count -= 1;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
            order.push(id);
        }
        ensure!(
            order.len() == self.tasks.len(),
            "dependency cycle in task catalog"
        );
        Ok(order)
    }
    pub fn eligibility(&self, id: &str) -> Eligibility {
        let Some(task) = self.tasks.get(id) else {
            return Eligibility::denied("Task does not exist");
        };
        if task.metadata.status != Status::Pending {
            return Eligibility::denied(format!(
                "Task must be pending (currently {:?})",
                task.metadata.status
            ));
        }
        let waiting: Vec<_> = task
            .metadata
            .dependencies
            .iter()
            .filter(|id| {
                !matches!(
                    self.tasks[*id].metadata.status,
                    Status::Done | Status::Draft
                )
            })
            .cloned()
            .collect();
        if !waiting.is_empty() {
            return Eligibility::denied(format!("Waiting for {}", waiting.join(", ")));
        }
        Eligibility::allowed()
    }
    pub fn scope_tasks(&self, scope: &Scope) -> anyhow::Result<Vec<String>> {
        Ok(match scope {
            Scope::Task(id) => {
                ensure!(self.tasks.contains_key(id), "unknown task: {id}");
                vec![id.clone()]
            }
            Scope::Group { project, group } => self
                .groups
                .get(&format!("{project}/{group}"))
                .context("unknown group")?
                .tasks
                .clone(),
            Scope::Project(project) => {
                ensure!(self.projects.contains_key(project), "unknown project");
                self.tasks
                    .values()
                    .filter(|t| &t.project_code == project)
                    .map(|t| t.id.clone())
                    .collect()
            }
        })
    }
    pub fn validate_scope(&self, scope: &Scope) -> anyhow::Result<Vec<String>> {
        let ids: Vec<_> = self
            .scope_tasks(scope)?
            .into_iter()
            .filter(|id| self.tasks[id].metadata.status != Status::Draft)
            .collect();
        ensure!(!ids.is_empty(), "No executable tasks in scope");
        for id in &ids {
            ensure!(
                self.tasks[id].metadata.status == Status::Pending,
                "Every executable task must be pending: {id}"
            );
        }
        if matches!(scope, Scope::Task(_)) {
            let eligibility = self.eligibility(&ids[0]);
            ensure!(
                eligibility.allowed,
                "{}",
                eligibility.reason.unwrap_or_default()
            );
        }
        for id in &ids {
            for dependency in &self.tasks[id].metadata.dependencies {
                ensure!(
                    ids.contains(dependency)
                        || matches!(
                            self.tasks[dependency].metadata.status,
                            Status::Draft | Status::Done
                        ),
                    "Unfinished dependency outside execution scope: {dependency}"
                );
            }
        }
        Ok(ids)
    }
    pub fn ready_tasks(&self, ids: &[String]) -> Vec<String> {
        ids.iter()
            .filter(|id| self.eligibility(id).allowed)
            .cloned()
            .collect()
    }
    /// Validate immutable task and verifier identities against the authoritative taskfmt gate.
    pub fn verify_done(
        &self,
        id: &str,
        manifest: &crate::runstate::Manifest,
    ) -> anyhow::Result<()> {
        let task = self.tasks.get(id).context("unknown task")?;
        verify_task_evidence(task, manifest)
    }
    pub fn aggregate(&self, ids: &[String]) -> Aggregate {
        let mut result = Aggregate::default();
        for task in ids.iter().filter_map(|id| self.tasks.get(id)) {
            result.task_count += 1;
            result.progress += task.progress.percentage;
            match task.metadata.status {
                Status::Draft => result.draft += 1,
                Status::Pending => {
                    result.pending += 1;
                    if !self.eligibility(&task.id).allowed {
                        result.blocked += 1;
                    }
                }
                Status::InProgress => result.in_progress += 1,
                Status::Done => result.done += 1,
            }
        }
        if result.task_count > 0 {
            result.progress /= result.task_count as f64;
        }
        result
    }
    pub(crate) fn start(&mut self, id: &str) -> anyhow::Result<()> {
        let eligibility = self.eligibility(id);
        ensure!(
            eligibility.allowed,
            "{}",
            eligibility.reason.unwrap_or_default()
        );
        self.persist_status(id, Status::InProgress)
    }
    /// Adapter-only transition: `verified` must come from the taskfmt authoritative gate.
    pub(crate) fn finish(&mut self, id: &str, verified: bool) -> anyhow::Result<()> {
        ensure!(
            self.tasks.get(id).context("unknown task")?.metadata.status == Status::InProgress,
            "task is not in progress"
        );
        self.persist_status(
            id,
            if verified {
                Status::Done
            } else {
                Status::Pending
            },
        )
    }
    pub(crate) fn recover(&mut self, id: &str, verified: bool) -> anyhow::Result<()> {
        self.finish(id, verified)
    }
    fn persist_status(&mut self, id: &str, status: Status) -> anyhow::Result<()> {
        let task = self.tasks.get_mut(id).context("unknown task")?;
        let path = safe_path(&self.root, &task.path.join("task.toml"))?;
        let mut metadata = task.metadata.clone();
        metadata.status = status;
        atomic_write(&path, toml::to_string_pretty(&metadata)?.as_bytes())?;
        task.metadata = metadata;
        if status == Status::Done {
            task.progress.percentage = 100.0;
        }
        Ok(())
    }
}

/// Verify completion against the current package; never infer success from status alone.
pub fn verify_task_evidence(
    task: &TaskRecord,
    manifest: &crate::runstate::Manifest,
) -> anyhow::Result<()> {
    let gate = manifest
        .gate
        .as_ref()
        .context("done requires a taskfmt gate")?;
    ensure!(
        manifest.task == task.contract_id,
        "run belongs to another task contract"
    );
    ensure!(
        gate.promotable() && gate.exit == 0 && gate.last_line == "DONE",
        "done requires successful authoritative gate/v3 verification"
    );
    for (filename, expected) in [
        ("README.md", &gate.task_sha256),
        ("verify.toml", &gate.verifier_sha256),
    ] {
        let path = safe_path(&task.path, &task.path.join(filename))?;
        ensure!(
            crate::selfhost::hash::digest_file(&path)? == *expected,
            "verified {filename} identity differs from current task contract"
        );
    }
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("file has no parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

/// Exclusive OS lock survives stale files and releases automatically on process death.
#[derive(Debug)]
pub struct RunLock {
    _file: File,
}
impl RunLock {
    pub fn acquire(root: &Path) -> anyhow::Result<Self> {
        let path = root.join(".monitor.lock");
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            ensure!(!metadata.file_type().is_symlink(), "lock cannot be symlink");
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        if file.try_lock().is_err() {
            bail!("Another monitor owns the execution root");
        }
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests;
