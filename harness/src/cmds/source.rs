//! Resolve operator package references without treating filesystem paths as run identities.

use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use sha2::{Digest, Sha256};

use crate::taskfile::TaskFile;

/// A package location and its independent immutable contract identity.
#[derive(Debug)]
pub struct TaskSource {
    pub package_dir: PathBuf,
    pub contract_id: String,
    /// One filesystem/container-safe component; legacy flat IDs retain their spelling.
    pub run_key: String,
}

/// A resolved package location, before the dispatch lifecycle validates its contract.
#[derive(Debug)]
pub struct TaskLocation {
    pub package_dir: PathBuf,
    pub run_key: String,
}

fn valid_contract_id(value: &str) -> bool {
    value.strip_prefix("TASK-").is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

impl TaskLocation {
    /// Locate a package and construct a safe run key without changing the legacy clone-before-
    /// contract-validation order. Returns an error for missing package locations or README files.
    pub fn resolve(tasks_dir: &Path, argument: &str) -> anyhow::Result<Self> {
        let resolved = super::resolve_task_arg(tasks_dir, argument)?;
        let directory = if resolved.is_file() {
            ensure!(
                resolved.file_name().is_some_and(|name| name == "README.md"),
                "task file must be README.md"
            );
            resolved.parent().context("task README has no parent")?
        } else {
            resolved.as_path()
        };
        let package_dir = directory.canonicalize().context("resolving task package")?;
        ensure!(
            package_dir.join("README.md").is_file(),
            "{} has no README.md",
            package_dir.display()
        );
        let run_key = if valid_contract_id(argument) {
            argument.to_owned()
        } else {
            let digest = Sha256::digest(package_dir.as_os_str().as_encoded_bytes());
            let suffix: String = digest[..8]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            format!("task-{suffix}")
        };
        Ok(Self {
            package_dir,
            run_key,
        })
    }

    /// Read the strict contract after the caller reaches its validation boundary.
    /// Returns an error when the contract or its task identity is invalid.
    pub fn load_contract(self) -> anyhow::Result<TaskSource> {
        let contract = TaskFile::load(&self.package_dir.join("README.md"))?;
        let contract_id = contract.frontmatter.id;
        ensure!(
            valid_contract_id(&contract_id),
            "task contract id must be TASK-<digits>"
        );
        Ok(TaskSource {
            package_dir: self.package_dir,
            contract_id,
            run_key: self.run_key,
        })
    }
}

impl TaskSource {
    /// Resolve and validate a CLI ID, directory, or README path. Browser callers must validate
    /// catalog ownership first. Returns an error for missing or malformed task contracts.
    pub fn resolve(tasks_dir: &Path, argument: &str) -> anyhow::Result<Self> {
        TaskLocation::resolve(tasks_dir, argument)?.load_contract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(root: &Path, path: &str) -> PathBuf {
        let directory = root.join(path);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("README.md"),
            "---\nschema: task/v5\nid: TASK-001\ntitle: Example\nkind: feature\n---\n",
        )
        .unwrap();
        directory
    }

    #[test]
    fn flat_identity_stays_compatible() {
        let root = tempfile::tempdir().unwrap();
        package(root.path(), "TASK-001");
        let source = TaskSource::resolve(root.path(), "TASK-001").unwrap();
        assert_eq!(source.run_key, "TASK-001");
        assert_eq!(source.contract_id, "TASK-001");
    }

    #[test]
    fn hierarchical_packages_with_same_contract_have_distinct_safe_run_keys() {
        let root = tempfile::tempdir().unwrap();
        let first = package(root.path(), "alpha/design/001");
        package(root.path(), "beta/design/001");
        let first_source = TaskSource::resolve(root.path(), first.to_str().unwrap()).unwrap();
        let readme_source =
            TaskSource::resolve(root.path(), first.join("README.md").to_str().unwrap()).unwrap();
        let second_source = TaskSource::resolve(root.path(), "beta/design/001").unwrap();
        assert_eq!(first_source.contract_id, second_source.contract_id);
        assert_ne!(first_source.run_key, second_source.run_key);
        assert_eq!(first_source.run_key, readme_source.run_key);
        assert!(
            first_source
                .run_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
    }

    #[test]
    fn path_like_contract_identity_is_rejected_before_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let directory = package(root.path(), "project/group/001");
        let readme = directory.join("README.md");
        let text = std::fs::read_to_string(&readme).unwrap();
        std::fs::write(&readme, text.replace("TASK-001", "TASK-../../escape")).unwrap();
        assert!(TaskSource::resolve(root.path(), directory.to_str().unwrap()).is_err());
    }

    #[test]
    fn cli_discovery_uses_hierarchical_catalog_validation() {
        let root = tempfile::tempdir().unwrap();
        let directory = package(root.path(), "project/group/001");
        for (path, text) in [
            ("project/README.md", "# Project\n"),
            ("project/group/README.md", "# Group\n"),
        ] {
            std::fs::write(root.path().join(path), text).unwrap();
        }
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example");
        for file in ["README.md", "verify.toml"] {
            std::fs::copy(example.join(file), directory.join(file)).unwrap();
        }
        let metadata = directory.join("task.toml");
        std::fs::write(
            &metadata,
            "schema = \"task-meta/v1\"\nstatus = \"pending\"\ndependencies = []\n",
        )
        .unwrap();
        assert_eq!(
            super::super::all_task_dirs(root.path()).unwrap(),
            vec![directory.canonicalize().unwrap()]
        );
        std::fs::write(metadata, "schema = \"task-meta/v1\"\nstatus = \"pending\"\ndependencies = [\"project/group/999\"]\n").unwrap();
        assert!(super::super::all_task_dirs(root.path()).is_err());
    }
}
