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

/// Atomically writes `bytes` to `path`, refusing to replace an existing file.
///
/// Evidence keys are allocated by scanning the store root, so two processes can
/// choose the same next key. Plain rename would silently overwrite the first
/// payload; this helper claims the destination with `create_new` before the
/// durable rename so a colliding writer must pick another key.
pub fn write_atomic_exclusive(
    path: &Path,
    bytes: impl AsRef<[u8]>,
    operation: &'static str,
) -> Result<(), StoreError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(claim) => drop(claim),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(StoreError::OperationFailed {
                store: path.display().to_string(),
                operation,
                reason: format!("evidence key already exists at {}", path.display()),
                retryable: Some(true),
            });
        }
        Err(err) => return Err(operation_failed(operation, path, &err)),
    }
    match write_atomic(path, bytes, operation) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(path);
            Err(err)
        }
    }
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
    use std::fs;

    use leaven_store::StoreError;

    use super::{write_atomic, write_atomic_exclusive};

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

    #[test]
    fn write_atomic_exclusive_refuses_existing_destination() {
        let root = std::env::temp_dir().join(format!(
            "leaven-store-file-atomic-exclusive-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("0.json");
        fs::write(&path, br#"{"message":"first"}"#).unwrap();

        let error = write_atomic_exclusive(&path, br#"{"message":"second"}"#, "put")
            .expect_err("exclusive write must refuse an existing evidence key");

        assert!(matches!(
            error,
            StoreError::OperationFailed {
                operation: "put",
                retryable: Some(true),
                ..
            }
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"message":"first"}"#);
        let _ = fs::remove_dir_all(&root);
    }
}
