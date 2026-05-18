//! Local atomic file-write helpers.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use leaven_store::StoreError;

pub fn write_atomic(
    path: &Path,
    bytes: impl AsRef<[u8]>,
    operation: &'static str,
) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp = temp_path(path)?;
    let result = write_temp_then_rename(&temp, path, parent, bytes.as_ref(), operation);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_temp_then_rename(
    temp: &Path,
    path: &Path,
    parent: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), StoreError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)
        .map_err(|err| operation_failed(operation, temp, &err))?;
    file.write_all(bytes)
        .map_err(|err| operation_failed(operation, temp, &err))?;
    file.sync_all()
        .map_err(|err| operation_failed(operation, temp, &err))?;
    drop(file);
    fs::rename(temp, path).map_err(|err| operation_failed(operation, path, &err))?;
    sync_parent(parent, operation)
}

fn temp_path(path: &Path) -> Result<PathBuf, StoreError> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(StoreError::OperationFailed {
            store: path.display().to_string(),
            operation: "atomic_path",
            reason: "path has no file name".to_owned(),
            retryable: Some(false),
        });
    };
    Ok(path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4())))
}

fn sync_parent(parent: &Path, operation: &'static str) -> Result<(), StoreError> {
    let dir = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|err| operation_failed(operation, parent, &err))?;
    dir.sync_all()
        .map_err(|err| operation_failed(operation, parent, &err))
}

fn operation_failed(operation: &'static str, path: &Path, err: &std::io::Error) -> StoreError {
    StoreError::OperationFailed {
        store: path.display().to_string(),
        operation,
        reason: err.to_string(),
        retryable: Some(false),
    }
}

#[cfg(test)]
mod tests {
    use leaven_store::StoreError;

    use super::write_atomic;

    #[test]
    fn write_atomic_rejects_paths_without_file_names() {
        let error = write_atomic(std::path::Path::new(""), b"payload", "put")
            .expect_err("empty path must not create a temp file");

        assert!(matches!(
            error,
            StoreError::OperationFailed {
                operation: "atomic_path",
                retryable: Some(false),
                ..
            }
        ));
    }
}
