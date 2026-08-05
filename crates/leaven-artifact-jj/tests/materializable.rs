use std::collections::BTreeMap;

use futures::executor::block_on;
use leaven_artifact_jj::{JjArtifact, JjChange};
use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};
use leaven_stage::MaterializableArtifact;
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

const REMOVED_PLACEHOLDER_JJ_NAMES: &[&str] = &[
    "JjArtifactIdentityMode",
    "JjOp",
    "ConflictRegion",
    "ConflictRegionId",
    "OperationId",
    "OperationSummary",
    "JjChangesetSurface",
    "JjConflictSurface",
    "JjPathSurface",
];

#[test]
fn jj_artifact_materializes_files_and_reads_patch_change() {
    block_on(async {
        let mut files = BTreeMap::new();
        files.insert(
            WorkspacePath::new("src/lib.rs").unwrap(),
            b"pub fn old() {}".to_vec(),
        );
        let artifact = JjArtifact::new(files);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace.slot(WorkspacePath::root()).unwrap();
            artifact.write_to(&mut slot).await.unwrap();
            assert_eq!(artifact.files().len(), 1);
            assert_eq!(
                artifact
                    .apply_change(&JjChange::Patch("ignored".to_owned()))
                    .unwrap(),
                artifact
            );
            match (artifact.identity(), artifact.cache_identity().unwrap()) {
                (ArtifactIdentity::Content(identity), CacheIdentity::Content(cache)) => {
                    assert_eq!(identity, cache);
                }
                other => panic!("unexpected jj identity shape: {other:?}"),
            }
            assert_eq!(
                slot.read_file(&WorkspacePath::new("src/lib.rs").unwrap())
                    .unwrap(),
                b"pub fn old() {}"
            );
            slot.write_file(
                &WorkspacePath::new(".leaven/jj/change.patch").unwrap(),
                b"diff --git a/src/lib.rs b/src/lib.rs",
            )
            .unwrap();
            assert_eq!(
                artifact.read_back_change(&slot).await.unwrap(),
                Some(JjChange::Patch(
                    "diff --git a/src/lib.rs b/src/lib.rs".to_owned()
                ))
            );
        }
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn jj_artifact_reports_absent_or_invalid_patch_changes() {
    block_on(async {
        let artifact = JjArtifact::new(BTreeMap::new());
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace.slot(WorkspacePath::root()).unwrap();
            assert_eq!(artifact.read_back_change(&slot).await.unwrap(), None);
            slot.write_file(
                &WorkspacePath::new(".leaven/jj/change.patch").unwrap(),
                &[0xFF, 0xFE],
            )
            .unwrap();
            assert!(artifact.read_back_change(&slot).await.is_err());
        }
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn crate_root_does_not_export_empty_jj_reservation_names() {
    let lib = std::fs::read_to_string("src/lib.rs").expect("read jj artifact crate root");

    for symbol in REMOVED_PLACEHOLDER_JJ_NAMES {
        assert!(
            !lib.contains(symbol),
            "`{symbol}` must not be reintroduced without JJ behavior and contract tests"
        );
    }
}

#[test]
fn jj_content_id_separates_path_and_content_length_boundaries() {
    // Without path length framing, path bytes can absorb a neighboring
    // little-endian content-length field:
    //   A: path = "x" || u64le(8), content = []
    //   B: path = "x", content = [0; 8]
    let mut path_a = String::from("x");
    path_a.push_str(std::str::from_utf8(&(8u64).to_le_bytes()).unwrap());
    let a_path = WorkspacePath::new(&path_a).expect("NUL-bearing path parses today");
    let b_path = WorkspacePath::new("x").unwrap();

    let mut a_files = BTreeMap::new();
    a_files.insert(a_path, Vec::new());
    let mut b_files = BTreeMap::new();
    b_files.insert(b_path, vec![0u8; 8]);

    let a = JjArtifact::new(a_files);
    let b = JjArtifact::new(b_files);
    assert_ne!(a.files(), b.files(), "file maps must be distinct inputs");
    assert_ne!(
        a.identity(),
        b.identity(),
        "distinct JJ file maps must not share ArtifactIdentity"
    );
    assert_ne!(
        a.cache_identity(),
        b.cache_identity(),
        "distinct JJ file maps must not share CacheIdentity"
    );
}
