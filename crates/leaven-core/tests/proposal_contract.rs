use leaven_core::{
    Artifact, ArtifactIdentity, CausalInputs, ExternalRef, InfoRef, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics, ProposalEffect, ProposalProvenance,
};
use leaven_kernel::{CandidateId, ContentId, MetadataBag, MetadataValue, ProposalId};

#[test]
fn proposal_builders_preserve_causal_and_informational_lineage() {
    let parent = CandidateId::new();
    let right = CandidateId::new();
    let external = InfoRef::External(ExternalRef {
        kind: "paper".to_owned(),
        id: "arxiv:test".to_owned(),
    });
    let metadata = {
        let mut bag = MetadataBag::new();
        bag.insert("worker", MetadataValue::String("local".to_owned()));
        bag
    };

    let created = Proposal::<TestProblem>::create(TestArtifact("fresh".to_owned()))
        .informed_by([InfoRef::Candidate(parent), external.clone()])
        .annotations(TestAnnotations { label: "create" })
        .metadata(metadata)
        .build();
    let mutated = Proposal::<TestProblem>::mutate(parent, TestChange("edit".to_owned())).build();
    let merged =
        Proposal::<TestProblem>::merge(parent, right, TestChange("merge".to_owned())).build();
    let aggregated =
        Proposal::<TestProblem>::aggregate(vec![parent, right], TestArtifact("n".to_owned()))
            .build();

    assert!(matches!(created.effect, ProposalEffect::Create { .. }));
    assert_eq!(created.provenance.causal(), &CausalInputs::None);
    assert_eq!(
        created.provenance.informed_by_refs(),
        [InfoRef::Candidate(parent), external]
    );
    assert_eq!(created.annotations.label, "create");
    assert!(matches!(
        created.metadata.get(&"worker".into()),
        Some(MetadataValue::String(worker)) if worker == "local"
    ));
    assert_eq!(mutated.provenance.causal(), &CausalInputs::Single(parent));
    assert_eq!(
        merged.provenance.causal(),
        &CausalInputs::Pair(parent, right)
    );
    assert_eq!(
        aggregated.provenance.causal(),
        &CausalInputs::NAry(vec![parent, right])
    );
}

#[test]
fn causal_inputs_answer_membership_and_iterate_in_order() {
    let left = CandidateId::new();
    let right = CandidateId::new();
    let third = CandidateId::new();

    assert!(!CausalInputs::None.contains_candidate(left));
    assert!(CausalInputs::Single(left).contains_candidate(left));
    assert!(CausalInputs::Pair(left, right).contains_candidate(right));
    assert!(!CausalInputs::Pair(left, right).contains_candidate(third));
    assert!(CausalInputs::NAry(vec![left, right, third]).contains_candidate(third));
    assert_eq!(
        CausalInputs::NAry(vec![left, right, third])
            .iter()
            .collect::<Vec<_>>(),
        vec![left, right, third]
    );
}

#[test]
fn proposal_clone_preserves_effect_and_batch_context() {
    let target = CandidateId::new();
    let builder = Proposal::<TestProblem>::mutate(target, TestChange("x".to_owned()))
        .informed_by([InfoRef::Proposal(ProposalId::new())])
        .annotations(TestAnnotations { label: "mutation" });
    let proposal = builder.clone().build();
    let proposal_from_original = builder.build();
    let proposal_clone = proposal.clone();
    let batch = ProposalBatch {
        proposals: vec![proposal],
        semantics: ProposalBatchSemantics::CandidatePool,
        metadata: MetadataBag::new(),
    };
    let batch_clone = batch.clone();

    assert_eq!(proposal_from_original.annotations.label, "mutation");
    assert!(matches!(
        proposal_clone.effect,
        ProposalEffect::Change { target: cloned_target, .. } if cloned_target == target
    ));
    assert_eq!(batch.proposals.len(), 1);
    assert_eq!(batch_clone.proposals.len(), 1);
    assert_eq!(batch_clone.semantics, ProposalBatchSemantics::CandidatePool);
}

#[test]
fn provenance_builder_accumulates_bibliographic_refs() {
    let candidate = CandidateId::new();
    let proposal = ProposalId::new();
    let provenance = ProposalProvenance::new(CausalInputs::None)
        .informed_by([InfoRef::Candidate(candidate)])
        .informed_by([InfoRef::Proposal(proposal)]);

    assert_eq!(
        provenance.informed_by_refs(),
        [InfoRef::Candidate(candidate), InfoRef::Proposal(proposal)]
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestArtifact(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestChange(String);

#[derive(Debug)]
struct TestError;

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test artifact error")
    }
}

impl std::error::Error for TestError {}

impl Artifact for TestArtifact {
    type Change = TestChange;
    type ApplyError = TestError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = [0; ContentId::BYTES];
        let raw = self.0.as_bytes();
        bytes[..raw.len().min(ContentId::BYTES)]
            .copy_from_slice(&raw[..raw.len().min(ContentId::BYTES)]);
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(format!("{}{}", self.0, change.0)))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TestAnnotations {
    label: &'static str,
}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = TestAnnotations;
}

#[derive(Clone, Debug)]
struct TestEvidence;

impl leaven_core::Evidence for TestEvidence {}
