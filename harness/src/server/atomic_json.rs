use std::io::Write;
use std::path::Path;

pub(super) fn write_bytes(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub(super) fn write(path: &Path, value: &impl serde::Serialize) -> anyhow::Result<()> {
    write_bytes(path, &serde_json::to_vec_pretty(value)?)
}
