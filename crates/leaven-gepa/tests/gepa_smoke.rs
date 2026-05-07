use std::collections::BTreeMap;

use leaven_core::{Artifact, ArtifactIdentity, Evidence, OptimizationProblem};
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_gepa::{Gepa, ReflectiveMutation, SurfaceProposer};
use leaven_kernel::{ContentId, RunId};
use leaven_population::ParetoFrontier;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};

#[test]
fn gepa_owns_surface_and_lowers_selected_part_edits() {
    let artifact = PartMapArtifact(BTreeMap::from([
        ("answer".to_owned(), "draft".to_owned()),
        ("search".to_owned(), "query".to_owned()),
    ]));
    let mut gepa = Gepa::<SmokeProblem, PartMapSurface, ParetoFrontier>::new(
        PartMapSurface,
        ParetoFrontier::by_case().build(),
    );
    let mut proposer = ReflectiveMutation::new("improved".to_owned());

    let part = gepa.select_part(&artifact).unwrap();
    let edit = proposer
        .propose_edit(&artifact, gepa.surface(), &part)
        .unwrap();
    let change = gepa.change_part(&artifact, part.clone(), edit).unwrap();
    let changed = artifact.apply_change(&change).unwrap();

    assert_eq!(part, "answer");
    assert_eq!(artifact.0.get("answer").unwrap(), "draft");
    assert_eq!(changed.0.get("answer").unwrap(), "improved");
    assert_eq!(changed.0.get("search").unwrap(), "query");
}

#[test]
fn gepa_candidate_selector_is_population_backed() {
    let mut graph = RunGraph::<SmokeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx
        .insert_seed(PartMapArtifact(BTreeMap::new()), 0)
        .unwrap();
    let mut frontier = ParetoFrontier::by_case().build();
    frontier.observe_casewise_scalar(
        seed,
        leaven_kernel::AssessmentId::new(),
        &leaven_evidence::CasewiseEvidence::new(vec![leaven_evidence::CaseOutcome::new(
            leaven_kernel::CaseId::new(0),
            leaven_evidence::ScalarEvidence::new(1.0).unwrap(),
        )]),
    );
    let mut gepa =
        Gepa::<SmokeProblem, PartMapSurface, ParetoFrontier>::new(PartMapSurface, frontier);

    assert_eq!(gepa.select_candidate(ctx.graph()), Some(seed));
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartMapArtifact(BTreeMap<String, String>);

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartMapChange {
    part: String,
    value: String,
}

#[derive(Debug)]
struct PartMapError;

impl std::fmt::Display for PartMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("part map error")
    }
}

impl std::error::Error for PartMapError {}

impl Artifact for PartMapArtifact {
    type Change = PartMapChange;
    type ApplyError = PartMapError;

    fn identity(&self) -> ArtifactIdentity {
        let bytes = self
            .0
            .iter()
            .flat_map(|(key, value)| [key.as_bytes(), value.as_bytes()].concat())
            .collect::<Vec<_>>();
        ArtifactIdentity::Content(content_id(&bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        let mut next = self.0.clone();
        if !next.contains_key(&change.part) {
            return Err(PartMapError);
        }
        next.insert(change.part.clone(), change.value.clone());
        Ok(Self(next))
    }
}

struct SmokeProblem;

impl OptimizationProblem for SmokeProblem {
    type Artifact = PartMapArtifact;
    type Case = ();
    type Evidence = SmokeEvidence;
    type ProposalAnnotations = ();
}

struct SmokeEvidence;

impl Evidence for SmokeEvidence {}

#[derive(Clone, Debug)]
struct PartMapSurface;

impl EditSurface<PartMapArtifact> for PartMapSurface {
    type PartId = String;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(leaven_kernel::Fingerprint::from_bytes([3; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a PartMapArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(artifact
            .0
            .iter()
            .map(|(id, value)| Part {
                id: id.clone(),
                address: PartAddress(id.clone()),
                view: value.as_str(),
            })
            .collect())
    }

    fn change_part(
        &self,
        artifact: &PartMapArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<<PartMapArtifact as Artifact>::Change, SurfaceError> {
        if artifact.0.contains_key(&id) {
            Ok(PartMapChange {
                part: id,
                value: edit,
            })
        } else {
            Err(SurfaceError::UnknownPart)
        }
    }
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
