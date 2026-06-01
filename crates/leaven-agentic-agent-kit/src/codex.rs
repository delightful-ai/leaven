use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use leaven_artifact_agent_kit::{AgentKitManifest, AgentKitManifestError, AgentKitPath};

/// Requested filesystem mount policy for AgentKit projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKitMountMode {
    /// Always copy files into the run workspace.
    Copy,
    /// Try symlinks first and fall back to copy when symlinks are unavailable.
    SymlinkPreferred,
}

/// Filesystem operation that was actually applied for one projected file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKitMountApplied {
    /// The file was copied.
    Copy,
    /// The file was symlinked.
    Symlink,
}

/// Report for one projected AgentKit file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentKitMountReport {
    /// Source path in the checked-out AgentKit subtree.
    pub source: PathBuf,
    /// Target path in the run workspace.
    pub target: PathBuf,
    /// Requested mount mode.
    pub requested: AgentKitMountMode,
    /// Applied mount operation.
    pub applied: AgentKitMountApplied,
    /// Whether a failed symlink attempt fell back to copy.
    pub symlink_fallback: bool,
    /// File bytes made visible through the target.
    pub bytes_written: u64,
}

/// Codex projection output for an AgentKit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAgentKitMaterialization {
    /// Provider instruction text read from the manifest's system prompt slot.
    pub system_prompt: Option<String>,
    /// Files projected into the run workspace.
    pub mounts: Vec<AgentKitMountReport>,
    /// Number of files projected into the run workspace.
    pub files_written: usize,
    /// Total bytes made visible through projected files.
    pub bytes_written: u64,
}

/// Materializes a checked-out AgentKit subtree into Codex's workspace ABI.
#[derive(Clone, Copy, Debug)]
pub struct CodexAgentKitMaterializer {
    mount_mode: AgentKitMountMode,
}

impl CodexAgentKitMaterializer {
    /// Constructs a Codex AgentKit materializer.
    #[must_use]
    pub const fn new(mount_mode: AgentKitMountMode) -> Self {
        Self { mount_mode }
    }

    /// Materializes a checked-out AgentKit subtree into a run workspace.
    ///
    /// # Errors
    ///
    /// Returns [`CodexAgentKitMaterializerError`] when the manifest is invalid
    /// or the source/target filesystem operation fails.
    pub fn materialize(
        &self,
        kit_root: impl AsRef<Path>,
        workspace_root: impl AsRef<Path>,
    ) -> Result<CodexAgentKitMaterialization, CodexAgentKitMaterializerError> {
        let kit_root = kit_root.as_ref();
        let workspace_root = workspace_root.as_ref();
        let manifest_path = kit_root.join("manifest.toml");
        let manifest_text =
            read_to_string(&manifest_path).map_err(|source| io_error(&manifest_path, source))?;
        let manifest = AgentKitManifest::from_toml_str(&manifest_text)?;
        fs::create_dir_all(workspace_root).map_err(|source| io_error(workspace_root, source))?;

        let system_prompt = match manifest.system_prompt.as_ref() {
            Some(path) => {
                let source = join_agent_path(kit_root, path);
                Some(read_to_string(&source).map_err(|err| io_error(&source, err))?)
            }
            None => None,
        };

        let mut mounts = Vec::new();
        if let Some(path) = manifest.agent_docs.as_ref() {
            let source = join_agent_path(kit_root, path);
            let target = join_agent_path(workspace_root, &manifest.profiles.codex.agent_docs_mount);
            project_path(&source, &target, self.mount_mode, &mut mounts)?;
        }
        if let Some(path) = manifest.skills.as_ref() {
            let source = join_agent_path(kit_root, path);
            let target = join_agent_path(workspace_root, &manifest.profiles.codex.skills_mount);
            project_path(&source, &target, self.mount_mode, &mut mounts)?;
        }

        let files_written = mounts.len();
        let bytes_written = mounts.iter().map(|mount| mount.bytes_written).sum();
        Ok(CodexAgentKitMaterialization {
            system_prompt,
            mounts,
            files_written,
            bytes_written,
        })
    }
}

/// Codex AgentKit materialization failure.
#[derive(Debug, thiserror::Error)]
pub enum CodexAgentKitMaterializerError {
    /// The AgentKit manifest is invalid.
    #[error("invalid AgentKit manifest")]
    Manifest(#[from] AgentKitManifestError),
    /// A filesystem operation failed.
    #[error("filesystem operation failed at {path}")]
    Io {
        /// Path involved in the failed operation.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: io::Error,
    },
}

fn project_path(
    source: &Path,
    target: &Path,
    requested: AgentKitMountMode,
    mounts: &mut Vec<AgentKitMountReport>,
) -> Result<(), CodexAgentKitMaterializerError> {
    let metadata = fs::metadata(source).map_err(|err| io_error(source, err))?;
    if metadata.is_dir() {
        fs::create_dir_all(target).map_err(|err| io_error(target, err))?;
        for entry in fs::read_dir(source).map_err(|err| io_error(source, err))? {
            let entry = entry.map_err(|err| io_error(source, err))?;
            project_path(&entry.path(), &target.join(entry.file_name()), requested, mounts)?;
        }
        return Ok(());
    }

    materialize_file(source, target, requested, mounts)
}

fn materialize_file(
    source: &Path,
    target: &Path,
    requested: AgentKitMountMode,
    mounts: &mut Vec<AgentKitMountReport>,
) -> Result<(), CodexAgentKitMaterializerError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| io_error(parent, err))?;
    }
    let bytes_written = fs::metadata(source)
        .map_err(|err| io_error(source, err))?
        .len();
    let mut applied = AgentKitMountApplied::Copy;
    let mut symlink_fallback = false;

    if requested == AgentKitMountMode::SymlinkPreferred {
        match try_symlink(source, target) {
            Ok(()) => {
                applied = AgentKitMountApplied::Symlink;
            }
            Err(_) => {
                symlink_fallback = true;
                fs::copy(source, target).map_err(|err| io_error(target, err))?;
            }
        }
    } else {
        fs::copy(source, target).map_err(|err| io_error(target, err))?;
    }

    mounts.push(AgentKitMountReport {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        requested,
        applied,
        symlink_fallback,
        bytes_written,
    });
    Ok(())
}

fn read_to_string(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

fn join_agent_path(root: &Path, path: &AgentKitPath) -> PathBuf {
    let mut joined = root.to_path_buf();
    for component in path.as_str().split('/') {
        joined.push(component);
    }
    joined
}

fn io_error(path: &Path, source: io::Error) -> CodexAgentKitMaterializerError {
    CodexAgentKitMaterializerError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(unix)]
fn try_symlink(source: &Path, target: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

#[cfg(not(unix))]
fn try_symlink(_source: &Path, _target: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "symlink projection is unavailable on this platform",
    ))
}
