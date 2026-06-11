//! Build a single-repo [`GitProgramArtifact`] from in-memory file content with
//! a deterministic seed commit, and read a revision's tracked files back to the
//! same flat file map.
//!
//! These are generic Git-program operations: a host that owns only flat content
//! (such as a wire projection of a Git-backed artifact) can construct a real
//! run-scoped store plus the typed artifact that names its seed revision, run
//! the agentic loop over that artifact, and read an evolved child revision back
//! into flat content for payloads and results. They carry no `AgentKit`, skill,
//! or provider knowledge; the projection from a domain artifact to this file map
//! belongs to the domain layer that owns that projection.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramLayout,
    GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_workspace_git::GitWorkspaceGitError;

use crate::git_ops::host_git_with;
use crate::{GitAgenticGitError, GitProgramStores};

/// Pinned identity for deterministic seed commits.
///
/// Identical file content committed with this fixed identity and timestamp
/// always produces the same commit id, so two runs that build the same kit seed
/// share a content-addressed revision instead of drifting on wall-clock time.
const SEED_IDENTITY_NAME: &str = "Leaven";
const SEED_IDENTITY_EMAIL: &str = "leaven@seed.invalid";
const SEED_IDENTITY_DATE: &str = "2000-01-01T00:00:00 +0000";
const SEED_COMMIT_MESSAGE: &str = "leaven seed";

/// A single-repo Git-program seed built from in-memory file content.
///
/// Holds the typed [`GitProgramArtifact`] that names the seed revision plus the
/// durable [`GitProgramStores`] that back it. The caller keeps `stores` alive
/// for the whole run: materialization, readback, and revision reads all resolve
/// through it.
#[derive(Clone, Debug)]
pub struct GitProgramSeed {
    artifact: GitProgramArtifact,
    stores: GitProgramStores,
    repo: RepoKey,
    revision: GitRevision,
}

impl GitProgramSeed {
    /// The seed artifact naming the run-scoped repo at its seed revision.
    #[must_use]
    pub const fn artifact(&self) -> &GitProgramArtifact {
        &self.artifact
    }

    /// The durable stores backing the seed (and every child) revision.
    #[must_use]
    pub const fn stores(&self) -> &GitProgramStores {
        &self.stores
    }

    /// The single program repo key.
    #[must_use]
    pub const fn repo(&self) -> &RepoKey {
        &self.repo
    }

    /// The deterministic seed commit revision.
    #[must_use]
    pub const fn revision(&self) -> &GitRevision {
        &self.revision
    }
}

/// Builds a run-scoped single-repo Git program from in-memory file content.
///
/// Creates a bare durable store under `store_root`, writes every file in
/// `files` as a blob, assembles them into one tree, and records a deterministic
/// seed commit (fixed identity and timestamp) so identical content yields an
/// identical commit id. The returned [`GitProgramSeed`] exposes the typed
/// artifact, its stores, the repo key, and the seed revision.
///
/// `repo` keys the single program repo; `layout` is the workspace subpath the
/// repo materializes under (for example `repos/agent_kit`). `store_root` must be
/// an empty or fresh directory the caller owns for the run's lifetime.
///
/// # Errors
///
/// Returns [`GitAgenticGitError`] when `files` is empty, a host `git` plumbing
/// command fails, the produced object id is malformed, or the assembled
/// artifact fails validation.
pub fn build_program_seed(
    repo: RepoKey,
    layout: GitPath,
    store_root: &Path,
    files: &BTreeMap<GitPath, Vec<u8>>,
) -> Result<GitProgramSeed, GitAgenticGitError> {
    if files.is_empty() {
        return Err(GitAgenticGitError::EmptyProgramSeed);
    }

    let store = store_root.join(format!("{repo}.git"));
    fs::create_dir_all(&store).map_err(|source| GitWorkspaceGitError::CommandIo {
        program: "create git program seed store",
        source,
    })?;
    let git_dir = store.as_os_str();
    host_git_with(
        None,
        "git init --bare",
        vec![
            OsString::from("init"),
            OsString::from("--bare"),
            store.as_os_str().to_os_string(),
        ],
        &[],
        None,
    )?;

    // A scratch index assembles the seed tree without a worktree, so the bare
    // store never gains a checkout.
    let index = store.join("leaven-seed.index");
    let index_env: &[(&str, &OsStr)] =
        &[("GIT_DIR", git_dir), ("GIT_INDEX_FILE", index.as_os_str())];

    for (path, bytes) in files {
        let blob = hash_object(git_dir, bytes)?;
        host_git_with(
            None,
            "git update-index --cacheinfo",
            vec![
                OsString::from("update-index"),
                OsString::from("--add"),
                OsString::from("--cacheinfo"),
                OsString::from(format!("100644,{blob},{}", path.as_str())),
            ],
            index_env,
            None,
        )?;
    }

    let tree = trim_object(host_git_with(
        None,
        "git write-tree",
        vec![OsString::from("write-tree")],
        index_env,
        None,
    )?)?;
    let _ = fs::remove_file(&index);

    let commit = trim_object(host_git_with(
        None,
        "git commit-tree",
        vec![
            OsString::from("commit-tree"),
            OsString::from(&tree),
            OsString::from("-m"),
            OsString::from(SEED_COMMIT_MESSAGE),
        ],
        &seed_commit_env(git_dir),
        None,
    )?)?;

    let revision = GitRevision::Commit(GitObjectId::new(commit)?);
    let stores = GitProgramStores::new(BTreeMap::from([(repo.clone(), store)]))?;
    let artifact = GitProgramArtifact::new(
        BTreeMap::from([(
            repo.clone(),
            GitRepoArtifact::new(
                RepoRef::global(repo.clone()),
                revision.clone(),
                None,
                GitArtifactIdentityMode::Commit,
            ),
        )]),
        GitProgramLayout::new(BTreeMap::from([(repo.clone(), layout)]))?,
    )?;

    Ok(GitProgramSeed {
        artifact,
        stores,
        repo,
        revision,
    })
}

/// Reads every tracked file at `revision` in `repo`'s durable store back into a
/// flat file map.
///
/// This is the inverse of [`build_program_seed`] for one revision: it lists the
/// commit's tree and reads each file body, so a host can project an evolved
/// child revision back into the same flat content it built the seed from.
///
/// # Errors
///
/// Returns [`GitAgenticGitError`] when the store is missing for `repo`, the
/// revision is not a commit, a host `git` command fails, or a tracked path is
/// not valid [`GitPath`] content.
pub fn read_revision_files(
    stores: &GitProgramStores,
    repo: &RepoKey,
    revision: &GitRevision,
) -> Result<BTreeMap<GitPath, Vec<u8>>, GitAgenticGitError> {
    let GitRevision::Commit(commit) = revision else {
        return Err(GitAgenticGitError::NonCommitReadback { repo: repo.clone() });
    };
    let store = stores.store_for(repo)?;
    let git_dir = store.as_os_str();
    let listed = host_git_with(
        Some(store),
        "git ls-tree",
        vec![
            OsString::from("ls-tree"),
            OsString::from("-r"),
            OsString::from("-z"),
            OsString::from("--name-only"),
            OsString::from(commit.as_str()),
        ],
        &[],
        None,
    )?;
    let mut files = BTreeMap::new();
    for raw in listed.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        let path = std::str::from_utf8(raw).map_err(|_| {
            GitAgenticGitError::Git(GitWorkspaceGitError::Command {
                program: "git ls-tree",
                status: None,
                stderr: "tracked path is not valid UTF-8".to_owned(),
            })
        })?;
        let git_path = GitPath::new(path)?;
        let bytes = host_git_with(
            None,
            "git cat-file blob",
            vec![
                OsString::from("cat-file"),
                OsString::from("blob"),
                OsString::from(format!("{commit}:{path}")),
            ],
            &[("GIT_DIR", git_dir)],
            None,
        )?;
        files.insert(git_path, bytes);
    }
    Ok(files)
}

fn hash_object(git_dir: &OsStr, bytes: &[u8]) -> Result<String, GitAgenticGitError> {
    trim_object(host_git_with(
        None,
        "git hash-object",
        vec![
            OsString::from("hash-object"),
            OsString::from("-w"),
            OsString::from("--stdin"),
        ],
        &[("GIT_DIR", git_dir)],
        Some(bytes),
    )?)
}

fn trim_object(stdout: Vec<u8>) -> Result<String, GitAgenticGitError> {
    Ok(String::from_utf8(stdout)?.trim().to_owned())
}

fn seed_commit_env(git_dir: &OsStr) -> [(&'static str, &OsStr); 9] {
    [
        ("GIT_DIR", git_dir),
        ("GIT_AUTHOR_NAME", OsStr::new(SEED_IDENTITY_NAME)),
        ("GIT_AUTHOR_EMAIL", OsStr::new(SEED_IDENTITY_EMAIL)),
        ("GIT_AUTHOR_DATE", OsStr::new(SEED_IDENTITY_DATE)),
        ("GIT_COMMITTER_NAME", OsStr::new(SEED_IDENTITY_NAME)),
        ("GIT_COMMITTER_EMAIL", OsStr::new(SEED_IDENTITY_EMAIL)),
        ("GIT_COMMITTER_DATE", OsStr::new(SEED_IDENTITY_DATE)),
        // Keep host config from leaking into the deterministic identity.
        ("GIT_CONFIG_GLOBAL", OsStr::new("/dev/null")),
        ("GIT_CONFIG_SYSTEM", OsStr::new("/dev/null")),
    ]
}
