//! Snapshots every kit candidate's Git revision from graph truth so the result
//! projection can read each frontier candidate's flat kit parts back.
//!
//! The prompt path snapshots a candidate's template string directly from the
//! artifact; the kit path snapshots the candidate's `GitProgramArtifact`
//! revision instead, because the flat kit parts only become available by reading
//! that revision out of the durable Git store.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use leaven_artifact_git::{GitProgramArtifact, RepoKey};
use leaven_engine::{Callback, RunEvent, RunGraphView};
use leaven_kernel::CandidateId;
use leaven_run::RunProblem;
use serde_json::Value;

type KitProblem = RunProblem<GitProgramArtifact, Value, Value>;

/// Snapshot of every kit candidate's `GitProgramArtifact` from graph truth.
#[derive(Clone, Default)]
pub(in crate::optimize_run_service) struct KitArtifacts {
    artifacts: Arc<Mutex<BTreeMap<CandidateId, GitProgramArtifact>>>,
}

impl KitArtifacts {
    pub(in crate::optimize_run_service) fn new() -> Self {
        Self::default()
    }

    pub(in crate::optimize_run_service) fn artifact(
        &self,
        candidate: CandidateId,
    ) -> Option<GitProgramArtifact> {
        self.artifacts
            .lock()
            .expect("kit artifact snapshot lock poisoned")
            .get(&candidate)
            .cloned()
    }

    fn snapshot(&self, candidate: CandidateId, view: &RunGraphView<'_, KitProblem>) {
        if let Some(artifact) = view.artifact(candidate) {
            self.artifacts
                .lock()
                .expect("kit artifact snapshot lock poisoned")
                .insert(candidate, artifact.clone());
        }
    }

    fn snapshot_all(&self, view: &RunGraphView<'_, KitProblem>) {
        let mut artifacts = self
            .artifacts
            .lock()
            .expect("kit artifact snapshot lock poisoned");
        for candidate in view.candidate_tree().roots() {
            collect_subtree(view, candidate, &mut artifacts);
        }
    }
}

fn collect_subtree(
    view: &RunGraphView<'_, KitProblem>,
    candidate: CandidateId,
    artifacts: &mut BTreeMap<CandidateId, GitProgramArtifact>,
) {
    if let Some(artifact) = view.artifact(candidate) {
        artifacts.insert(candidate, artifact.clone());
    }
    for child in view.children(candidate) {
        collect_subtree(view, child, artifacts);
    }
}

/// Engine callback that snapshots kit candidate artifacts from graph truth.
pub(in crate::optimize_run_service) struct KitArtifactSnapshot {
    artifacts: KitArtifacts,
}

impl KitArtifactSnapshot {
    pub(in crate::optimize_run_service) fn new(artifacts: KitArtifacts) -> Self {
        Self { artifacts }
    }
}

impl Callback<KitProblem> for KitArtifactSnapshot {
    fn on_event(&mut self, event: &RunEvent, graph: RunGraphView<'_, KitProblem>) {
        match event {
            RunEvent::ApplySucceeded { candidate_id, .. } => {
                self.artifacts.snapshot(*candidate_id, &graph);
            }
            RunEvent::OptimizationStarted { .. } | RunEvent::OptimizationEnded { .. } => {
                self.artifacts.snapshot_all(&graph);
            }
            _ => {}
        }
    }
}

/// The single kit program repo key (`agent_kit`).
pub(in crate::optimize_run_service) fn kit_repo_key() -> RepoKey {
    RepoKey::new("agent_kit").expect("static kit repo key is valid")
}
