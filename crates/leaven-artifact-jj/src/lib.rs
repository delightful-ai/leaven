//! JJ artifact vocabulary.

pub mod artifact {
    pub struct JjArtifact;
    pub enum JjArtifactIdentityMode {
        Change,
        Commit,
    }
}
pub mod change {
    pub struct JjChange;
    pub struct JjOp;
}
pub mod conflict {
    pub struct ConflictRegion;
    pub struct ConflictRegionId;
}
pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum JjArtifactError {
        #[error("jj artifact failed")]
        Message,
    }
}
pub mod operation_log {
    pub struct OperationId;
    pub struct OperationSummary;
}
pub mod surface {
    pub struct JjChangesetSurface;
    pub struct JjConflictSurface;
    pub struct JjPathSurface;
}
pub mod tracked_run {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct JjTrackedRun {
        pub run_label: String,
        pub proof_denominator: String,
        pub spec_paths: Vec<String>,
        pub snapshots: Vec<JjSnapshotRecord>,
        pub evaluations: Vec<JjEvaluationRecord>,
    }

    impl JjTrackedRun {
        #[must_use]
        pub fn for_goal(
            run_label: impl Into<String>,
            proof_denominator: impl Into<String>,
        ) -> Self {
            Self {
                run_label: run_label.into(),
                proof_denominator: proof_denominator.into(),
                spec_paths: Vec::new(),
                snapshots: Vec::new(),
                evaluations: Vec::new(),
            }
        }

        pub fn record_snapshot(&mut self, snapshot: JjSnapshotRecord) -> usize {
            let index = self.snapshots.len();
            self.snapshots.push(snapshot);
            index
        }

        pub fn record_evaluation(&mut self, evaluation: JjEvaluationRecord) {
            self.evaluations.push(evaluation);
        }

        #[must_use]
        pub fn latest_snapshot(&self) -> Option<&JjSnapshotRecord> {
            self.snapshots.last()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct JjSnapshotRecord {
        pub kind: JjSnapshotKind,
        pub label: String,
        pub change_id: Option<String>,
        pub commit_id: Option<String>,
        pub operation_id: Option<String>,
        pub description: Option<String>,
        pub dirty: bool,
    }

    impl JjSnapshotRecord {
        #[must_use]
        pub fn new(kind: JjSnapshotKind, label: impl Into<String>) -> Self {
            Self {
                kind,
                label: label.into(),
                change_id: None,
                commit_id: None,
                operation_id: None,
                description: None,
                dirty: false,
            }
        }

        #[must_use]
        pub fn with_change_id(mut self, change_id: impl Into<String>) -> Self {
            self.change_id = Some(change_id.into());
            self
        }

        #[must_use]
        pub fn with_commit_id(mut self, commit_id: impl Into<String>) -> Self {
            self.commit_id = Some(commit_id.into());
            self
        }

        #[must_use]
        pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
            self.operation_id = Some(operation_id.into());
            self
        }

        #[must_use]
        pub fn with_description(mut self, description: impl Into<String>) -> Self {
            self.description = Some(description.into());
            self
        }

        #[must_use]
        pub const fn with_dirty(mut self, dirty: bool) -> Self {
            self.dirty = dirty;
            self
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum JjSnapshotKind {
        Initial,
        PreAgent,
        PostAgent,
        PostEvaluation,
        Final,
        Blocked,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct JjEvaluationRecord {
        pub name: String,
        pub command: String,
        pub snapshot_index: usize,
        pub exit_code: Option<i32>,
        pub stdout_path: Option<String>,
        pub stderr_path: Option<String>,
    }

    impl JjEvaluationRecord {
        #[must_use]
        pub fn new(
            name: impl Into<String>,
            command: impl Into<String>,
            snapshot_index: usize,
        ) -> Self {
            Self {
                name: name.into(),
                command: command.into(),
                snapshot_index,
                exit_code: None,
                stdout_path: None,
                stderr_path: None,
            }
        }

        #[must_use]
        pub const fn with_exit_code(mut self, exit_code: i32) -> Self {
            self.exit_code = Some(exit_code);
            self
        }

        #[must_use]
        pub fn with_stdout_path(mut self, stdout_path: impl Into<String>) -> Self {
            self.stdout_path = Some(stdout_path.into());
            self
        }

        #[must_use]
        pub fn with_stderr_path(mut self, stderr_path: impl Into<String>) -> Self {
            self.stderr_path = Some(stderr_path.into());
            self
        }

        #[must_use]
        pub fn passed(&self) -> bool {
            self.exit_code == Some(0)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct JjSnapshotPolicy {
        pub required: bool,
        pub points: Vec<JjSnapshotPoint>,
    }

    impl JjSnapshotPolicy {
        #[must_use]
        pub fn goal_loop() -> Self {
            Self {
                required: true,
                points: vec![
                    JjSnapshotPoint::new(
                        JjSnapshotKind::Initial,
                        "pre-goal",
                        "before handing the stage to the persistent agent goal",
                    ),
                    JjSnapshotPoint::new(
                        JjSnapshotKind::PostAgent,
                        "post-agent",
                        "after the agent has made a coherent edit slice",
                    ),
                    JjSnapshotPoint::new(
                        JjSnapshotKind::PostEvaluation,
                        "post-eval",
                        "after the verification commands have run",
                    ),
                    JjSnapshotPoint::new(
                        JjSnapshotKind::Final,
                        "final",
                        "after the proof denominator is satisfied or the run is blocked",
                    ),
                ],
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct JjSnapshotPoint {
        pub kind: JjSnapshotKind,
        pub label: String,
        pub reason: String,
    }

    impl JjSnapshotPoint {
        #[must_use]
        pub fn new(
            kind: JjSnapshotKind,
            label: impl Into<String>,
            reason: impl Into<String>,
        ) -> Self {
            Self {
                kind,
                label: label.into(),
                reason: reason.into(),
            }
        }
    }
}
pub use artifact::{JjArtifact, JjArtifactIdentityMode};
pub use change::{JjChange, JjOp};
pub use conflict::{ConflictRegion, ConflictRegionId};
pub use error::JjArtifactError;
pub use operation_log::{OperationId, OperationSummary};
pub use surface::{JjChangesetSurface, JjConflictSurface, JjPathSurface};
pub use tracked_run::{
    JjEvaluationRecord, JjSnapshotKind, JjSnapshotPoint, JjSnapshotPolicy, JjSnapshotRecord,
    JjTrackedRun,
};
