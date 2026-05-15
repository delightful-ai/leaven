use leaven::engine::{BudgetLedger, RunContext, RunGraph};
use leaven::extend::ProposalBatchSemantics;
use leaven::kernel::{RunId, StageId};
use leaven::plumbing::{ContentId, MetadataBag};
use leaven::prelude::{
    Artifact, ArtifactIdentity, Budget, Cost, Evidence, OptimizationProblem, Proposal,
    ProposalBatch,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = RunGraph::<TextProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::metric_calls(10));
    let mut ctx = RunContext::new(&mut graph, &mut budget);

    let seed = ctx.insert_seed(TextArtifact("seed".to_owned()), 0)?;
    let create_report = ctx.record_proposal_batch(
        StageId::custom("p0/create"),
        ProposalBatch {
            proposals: vec![Proposal::create(TextArtifact("fresh".to_owned())).build()],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::metric_calls(1),
    )?;
    let create_apply = ctx.apply_batch(create_report.batch_id)?;
    let created = create_apply
        .successful_candidates()
        .next()
        .expect("create proposal should insert a candidate");

    let mutate_report = ctx.record_proposal_batch(
        StageId::custom("p0/mutate"),
        ProposalBatch {
            proposals: vec![Proposal::mutate(seed, TextChange::Append("-mutated")).build()],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::metric_calls(1),
    )?;
    let mutate_apply = ctx.apply_batch(mutate_report.batch_id)?;
    let mutated = mutate_apply
        .successful_candidates()
        .next()
        .expect("mutation proposal should insert a candidate");

    let view = ctx.graph();
    assert_eq!(view.artifact(seed).expect("seed exists").0, "seed");
    assert_eq!(view.artifact(created).expect("created exists").0, "fresh");
    assert_eq!(
        view.artifact(mutated).expect("mutated exists").0,
        "seed-mutated"
    );
    assert!(view.parents(created).is_empty());
    assert_eq!(view.parents(mutated), vec![seed]);
    assert_eq!(view.children(seed), vec![mutated]);

    println!(
        "p0 graph skeleton: seed={seed} created={created} mutated={mutated} events={}",
        view.event_count()
    );
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(String);

#[derive(Clone, Debug, Eq, PartialEq)]
enum TextChange {
    Append(&'static str),
}

#[derive(Debug)]
struct TextError;

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextError {}

impl Artifact for TextArtifact {
    type Change = TextChange;
    type ApplyError = TextError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(self.0.as_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        match change {
            TextChange::Append(suffix) => Ok(Self(format!("{}{suffix}", self.0))),
        }
    }
}

struct TextProblem;

impl OptimizationProblem for TextProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = NoEvidence;
    type ProposalAnnotations = ();
}

struct NoEvidence;

impl Evidence for NoEvidence {}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
