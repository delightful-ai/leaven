use std::collections::BTreeMap;

use futures::executor::block_on;
use leaven_artifact_jj::{JjArtifact, JjChange};
use leaven_stage::MaterializableArtifact;
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

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
