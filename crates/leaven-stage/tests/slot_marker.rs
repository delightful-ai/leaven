use leaven_core::{Artifact, ArtifactIdentity, OptimizationProblem, ProposalBatch};
use leaven_stage::{ProposerSlot, SlotMarker};

#[test]
fn proposer_slot_output_is_proposal_batch() {
    fn assert_output<Slot>()
    where
        Slot: SlotMarker<TestProblem, Output = ProposalBatch<TestProblem>>,
    {
    }

    assert_output::<ProposerSlot<TestRequest>>();
}

#[derive(serde::Serialize)]
struct TestRequest;

#[derive(Clone)]
struct TestArtifact;

impl Artifact for TestArtifact {
    type Change = ();
    type ApplyError = std::convert::Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External("test".to_owned())
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

struct TestProblem;

#[derive(Clone, Debug)]
struct TestEvidence;

impl leaven_core::Evidence for TestEvidence {}

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}
