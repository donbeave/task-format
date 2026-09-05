//! CLI adapter for the non-promoting study subsystem.
pub fn run(
    config: &std::path::Path,
    root: &std::path::Path,
    task_dir: &std::path::Path,
    out: &std::path::Path,
) -> anyhow::Result<i32> {
    crate::study::run(config, root, task_dir, out)
}
