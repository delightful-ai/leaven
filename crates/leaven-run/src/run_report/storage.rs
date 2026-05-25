use std::{fs, fs::OpenOptions, io::Write, path::Path};

use crate::{
    OptimizeError,
    result::{
        RunNotResumableReason, RunReportPaths, RunResumability, RunStorage, StandardRunSummary,
    },
    run_store::PreparedStore,
};

pub fn run_storage<P>(
    run_id: leaven_kernel::RunId,
    store: &PreparedStore<P>,
    latest_checkpoint: Option<leaven_kernel::CheckpointId>,
    has_compatibility_manifest: bool,
) -> RunStorage
where
    P: leaven_core::OptimizationProblem,
{
    if store.store.persistence().is_some() {
        RunStorage::Stored {
            run_id,
            run_dir: store.run_dir.clone(),
            latest_checkpoint,
            resumability: if store.run_dir.is_none() {
                RunResumability::NotResumable {
                    reason: RunNotResumableReason::ExplicitStoreWithoutLocalRunDir,
                }
            } else if latest_checkpoint.is_none() {
                RunResumability::NotResumable {
                    reason: RunNotResumableReason::MissingLatestCheckpoint,
                }
            } else if !has_compatibility_manifest {
                RunResumability::NotResumable {
                    reason: RunNotResumableReason::MissingCompatibilityManifest,
                }
            } else {
                RunResumability::Resumable
            },
        }
    } else {
        RunStorage::Ephemeral { run_id }
    }
}

pub fn report_paths_for(storage: &RunStorage) -> RunReportPaths {
    match storage {
        RunStorage::Stored {
            run_dir: Some(run_dir),
            ..
        } => RunReportPaths {
            summary_json: Some(run_dir.join("reports").join("summary.json")),
        },
        RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => RunReportPaths::default(),
    }
}

pub fn write_summary_report(summary: &StandardRunSummary) -> Result<(), OptimizeError> {
    let Some(path) = &summary.reports.summary_json else {
        return Ok(());
    };
    let parent = path
        .parent()
        .expect("summary report path has parent directory");
    fs::create_dir_all(parent).map_err(|source| OptimizeError::ReportStore {
        operation: "create report directory",
        source,
    })?;
    let bytes =
        serde_json::to_vec_pretty(summary).expect("standard run summary is JSON-serializable");
    write_report_atomic(path, &bytes, "write summary json")
}

pub(super) fn write_report_atomic(
    path: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), OptimizeError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| OptimizeError::ReportStore {
            operation,
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name"),
        })?;
    let temp = path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = write_report_atomic_inner(path, &temp, parent, bytes, operation);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_report_atomic_inner(
    path: &Path,
    temp: &Path,
    parent: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), OptimizeError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)
        .map_err(|source| OptimizeError::ReportStore { operation, source })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| OptimizeError::ReportStore { operation, source })?;
    drop(file);
    fs::rename(temp, path).map_err(|source| OptimizeError::ReportStore { operation, source })?;
    let dir = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|source| OptimizeError::ReportStore { operation, source })?;
    dir.sync_all()
        .map_err(|source| OptimizeError::ReportStore { operation, source })
}
