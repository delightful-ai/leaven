use leaven_core::OptimizationProblem;
use leaven_engine::{Optimizer, OptimizerError, RestoreContext, RunCheckpoint, RunGraphView};

use crate::{OptimizeError, run_store::PreparedStore};

pub(super) fn restore_optimizer_checkpoint<P, O>(
    optimizer: &mut O,
    checkpoint: &RunCheckpoint,
    prepared_store: &PreparedStore<P>,
    view: RunGraphView<'_, P>,
) -> Result<(), OptimizeError>
where
    P: OptimizationProblem,
    O: Optimizer<P>,
{
    let reader = prepared_store.local_persistence.as_ref().ok_or_else(|| {
        OptimizeError::Optimizer(OptimizerError::Message(
            "stored run resume requires a readable local persistence store".to_owned(),
        ))
    })?;
    optimizer.restore_checkpoint_state(checkpoint, reader, RestoreContext::new(view))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use leaven_core::{Artifact, ArtifactIdentity, Evidence, OptimizationProblem};
    use leaven_engine::{Engine, GraphSnapshotRef, StateFormat, StepStatus};
    use leaven_kernel::{
        BlobRef, Budget, BudgetSnapshot, CandidateId, ContentId, Fingerprint, RunId, now,
    };
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::{OptimizeStore, run_store::StoreStart};

    #[test]
    fn restore_optimizer_checkpoint_requires_local_persistence_store() {
        let run_id = RunId::new();
        let checkpoint = RunCheckpoint::new(
            run_id,
            now(),
            GraphSnapshotRef {
                schema: Fingerprint::from_bytes([1; 32]),
                format: StateFormat::Json,
                bytes: BlobRef {
                    store: "file".to_owned(),
                    key: "graph.json".to_owned(),
                },
            },
            BudgetSnapshot {
                limit: Budget::unlimited(),
                ..BudgetSnapshot::default()
            },
        );
        let prepared = PreparedStore::<TestProblem> {
            store: OptimizeStore::inline("inline"),
            run_dir: None,
            local_persistence: None,
            evaluation_cache: None,
            start: StoreStart::Fresh { run_id },
        };
        let engine = Engine::<TestProblem>::builder().run_id(run_id).build();
        let mut optimizer = TestOptimizer;

        let error =
            restore_optimizer_checkpoint(&mut optimizer, &checkpoint, &prepared, engine.view())
                .unwrap_err();

        assert!(error.to_string().contains("stored run resume requires"));
    }

    struct TestOptimizer;

    impl leaven_engine::Optimizer<TestProblem> for TestOptimizer {
        async fn step(
            &mut self,
            _ctx: &mut leaven_engine::RunContext<'_, TestProblem>,
        ) -> Result<StepStatus, OptimizerError> {
            Ok(StepStatus::Done)
        }

        fn best_candidate(
            &self,
            _graph: leaven_engine::RunGraphView<'_, TestProblem>,
        ) -> Option<CandidateId> {
            None
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestArtifact;

    impl Artifact for TestArtifact {
        type Change = ();
        type ApplyError = Infallible;

        fn identity(&self) -> ArtifactIdentity {
            ArtifactIdentity::Content(ContentId::from_bytes([1; 32]))
        }

        fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
            Ok(Self)
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestEvidence;

    impl Evidence for TestEvidence {}

    struct TestProblem;

    impl OptimizationProblem for TestProblem {
        type Artifact = TestArtifact;
        type Case = ();
        type Evidence = TestEvidence;
        type ProposalAnnotations = ();
    }
}
