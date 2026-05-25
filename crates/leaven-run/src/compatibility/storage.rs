use std::{fs, fs::OpenOptions, io, io::Write, path::Path};

use super::{ResumeCompatibilityError, RunCompatibilityManifest, compare_manifests};

const MANIFEST_FILE: &str = "compatibility.json";

pub fn store_fresh_manifest(
    run_dir: Option<&Path>,
    manifest: &RunCompatibilityManifest,
) -> Result<(), io::Error> {
    let Some(run_dir) = run_dir else {
        return Ok(());
    };
    fs::create_dir_all(run_dir)?;
    let path = run_dir.join(MANIFEST_FILE);
    let bytes = serde_json::to_vec_pretty(manifest)
        .expect("compatibility manifest contains only serializable fields");
    write_atomic(&path, &bytes)
}

pub fn compare_stored_manifest(
    run_dir: &Path,
    live: &RunCompatibilityManifest,
) -> Result<(), ResumeCompatibilityError> {
    let path = run_dir.join(MANIFEST_FILE);
    let bytes = fs::read(&path).map_err(|source| ResumeCompatibilityError::Read {
        path: path.clone(),
        source,
    })?;
    let stored: RunCompatibilityManifest = serde_json::from_slice(&bytes)
        .map_err(|source| ResumeCompatibilityError::Decode { path, source })?;
    compare_manifests(&stored, live)
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let temp = path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = write_atomic_inner(path, &temp, parent, bytes);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_atomic_inner(
    path: &Path,
    temp: &Path,
    parent: &Path,
    bytes: &[u8],
) -> Result<(), io::Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    let dir = OpenOptions::new().read(true).open(parent)?;
    dir.sync_all()
}
