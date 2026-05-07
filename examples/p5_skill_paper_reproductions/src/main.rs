use std::collections::{BTreeMap, BTreeSet};

use futures::executor::block_on;
use leaven::{
    Artifact, ArtifactIdentity, Budget, ContentId, Cost, Evidence, MaterializationReport,
    MaterializeError, Materializer, MetadataBag, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_core::ExternalRef;
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_evidence::{
    AttributableEvidence, Attribution, CaseOutcome, CasewiseEvidence, ScalarEvidence,
};
use leaven_kernel::{CandidateId, CaseId, FiniteF64, Metered, RunId, StageId};
use leaven_population::ParetoFrontier;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let outcomes = vec![
            evoskill()?,
            trace2skill().await?,
            memento_skills()?,
            d2skill(),
            skill_reducer()?,
        ];
        for outcome in &outcomes {
            println!("p5 {}: {}", outcome.paper, outcome.proof);
        }
        assert_eq!(outcomes.len(), 5);
        Ok(())
    })
}

fn evoskill() -> Result<ScenarioOutcome, Box<dyn std::error::Error>> {
    let (mut graph, mut budget) = new_graph();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(registry([weak_skill()]), 0)?;
    let surface = SkillSurface;
    let parent = ctx.graph().artifact(seed).expect("seed exists").clone();

    let edit = surface.change_part(
        &parent,
        SkillPartId::Body("debugger".to_owned()),
        "Check failing assertion, inspect trace, then patch the exact failing precondition."
            .to_owned(),
    )?;
    let edited = apply_change(&mut ctx, seed, edit, "evoskill/edit")?;
    let created_registry = parent.with_skill(Skill {
        id: "repair-localizer".to_owned(),
        route: "Use when a failure trace names a precise broken precondition.".to_owned(),
        body: "Localize the violated precondition before editing code.".to_owned(),
        references: "fixture: failure traces 0 and 1".to_owned(),
        validation: "must pass held-out repair case".to_owned(),
    });
    let created = apply_create(&mut ctx, created_registry, "evoskill/create")?;

    let mut frontier = ParetoFrontier::by_case().build();
    frontier.observe_casewise_scalar(
        seed,
        leaven_kernel::AssessmentId::new(),
        &scores(&[(0, 0.2)]),
    );
    frontier.observe_casewise_scalar(
        edited,
        leaven_kernel::AssessmentId::new(),
        &scores(&[(0, 0.7)]),
    );
    frontier.observe_casewise_scalar(
        created,
        leaven_kernel::AssessmentId::new(),
        &scores(&[(0, 1.0)]),
    );

    assert!(frontier.contains(created));
    assert_eq!(frontier.best(), Some(created));
    Ok(ScenarioOutcome::new(
        "EvoSkill arx_2603.02766",
        "create-vs-edit proposals validated through casewise frontier",
    ))
}

async fn trace2skill() -> Result<ScenarioOutcome, Box<dyn std::error::Error>> {
    let (mut graph, mut budget) = new_graph();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let lesson_a = ctx.insert_seed(
        registry([Skill::lesson("trace-a", "Preserve table formulas.")]),
        0,
    )?;
    let lesson_b = ctx.insert_seed(
        registry([Skill::lesson("trace-b", "Check hidden merged cells.")]),
        0,
    )?;
    let consolidated = registry([Skill {
        id: "spreadsheet-auditor".to_owned(),
        route: "Use for spreadsheet repair after multiple trace analyses.".to_owned(),
        body: "Preserve formulas and check hidden merged cells before editing.".to_owned(),
        references: "trace-a; trace-b".to_owned(),
        validation: "passes transfer fixture".to_owned(),
    }]);
    let created = apply_aggregate(
        &mut ctx,
        vec![lesson_a, lesson_b],
        consolidated,
        "trace2skill/consolidate",
    )?;
    let parents = ctx.graph().parents(created);
    assert_eq!(parents.len(), 2);
    assert!(parents.contains(&lesson_a));
    assert!(parents.contains(&lesson_b));

    let factory = LocalWorkspaceFactory::default();
    let mut workspace = factory.allocate(WorkspaceConfig::default()).await?;
    let result = {
        let mut view = workspace.view();
        let artifact = ctx
            .graph()
            .artifact(created)
            .expect("artifact exists")
            .clone();
        SkillRegistryMaterializer
            .materialize_into(&artifact, &mut view, ctx.materialize_context())
            .await
    };
    workspace.cleanup().await?;
    assert_eq!(result?.value.files_written, 4);
    Ok(ScenarioOutcome::new(
        "Trace2Skill arx_2603.25158",
        "NAry lesson consolidation materializes one transferable skill",
    ))
}

fn memento_skills() -> Result<ScenarioOutcome, Box<dyn std::error::Error>> {
    let (mut graph, mut budget) = new_graph();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(registry([weak_skill()]), 0)?;
    let mut live = seed;
    let surface = SkillSurface;

    let bad_change = surface.change_part(
        ctx.graph().artifact(live).expect("live exists"),
        SkillPartId::Body("debugger".to_owned()),
        "Delete all debugging constraints.".to_owned(),
    )?;
    let rejected = apply_change(&mut ctx, live, bad_change, "memento/bad-edit")?;
    assert!(ctx.graph().artifact(rejected).is_some());
    assert_eq!(live, seed);

    let good_change = surface.change_part(
        ctx.graph().artifact(live).expect("live exists"),
        SkillPartId::Body("debugger".to_owned()),
        "Inspect trace, patch the minimal failing condition, rerun the reproducer.".to_owned(),
    )?;
    let accepted = apply_change(&mut ctx, live, good_change, "memento/good-edit")?;
    if score_registry(ctx.graph().artifact(accepted).expect("accepted exists"))
        > score_registry(ctx.graph().artifact(live).expect("live exists"))
    {
        live = accepted;
    }

    assert_eq!(live, accepted);
    assert_ne!(live, rejected);
    Ok(ScenarioOutcome::new(
        "Memento-Skills arx_2603.18743",
        "failed edits remain in graph while live library rolls forward only on validation",
    ))
}

fn d2skill() -> ScenarioOutcome {
    let mut bank = UtilityBank::default();
    let baseline = 0.2;
    let task_skill = 0.8;
    let step_skill = 0.6;
    bank.observe(&SkillUtilityEvidence::new(BTreeMap::from([
        ("task:debugger".to_owned(), task_skill - baseline),
        ("step:inspect-trace".to_owned(), step_skill - baseline),
        ("step:guess".to_owned(), -0.1),
    ])));
    bank.prune_below(FiniteF64::new(0.0).expect("finite threshold"));

    assert!(bank.utility("task:debugger").as_f64() > 0.0);
    assert!(bank.utility("step:inspect-trace").as_f64() > 0.0);
    assert!(!bank.skills.contains("step:guess"));
    ScenarioOutcome::new(
        "D2Skill arx_2603.28716",
        "paired baseline-vs-skill utility updates task and step banks, then prunes",
    )
}

fn skill_reducer() -> Result<ScenarioOutcome, Box<dyn std::error::Error>> {
    let (mut graph, mut budget) = new_graph();
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(registry([Skill {
        id: "debugger".to_owned(),
        route: "Use when a coding task fails and you need to inspect the exact failing trace before editing.".to_owned(),
        body: "First inspect the failing assertion. Then identify the smallest violated precondition. Then patch only that precondition and rerun the reproducer.".to_owned(),
        references: "long examples omitted".to_owned(),
        validation: "must preserve trace-first behavior".to_owned(),
    }]), 0)?;
    let surface = SkillSurface;
    let parent = ctx.graph().artifact(seed).expect("seed exists").clone();
    let destructive = surface.change_part(
        &parent,
        SkillPartId::Body("debugger".to_owned()),
        "Patch quickly.".to_owned(),
    )?;
    let rejected = parent.apply_change(&destructive)?;
    assert!(!preserves_behavior(&rejected));

    let route_compression = surface.change_part(
        &parent,
        SkillPartId::Route("debugger".to_owned()),
        "Use for trace-localized coding failures.".to_owned(),
    )?;
    let accepted = apply_change(&mut ctx, seed, route_compression, "skillreducer/route")?;
    let accepted_artifact = ctx.graph().artifact(accepted).expect("accepted exists");
    assert!(preserves_behavior(accepted_artifact));
    assert!(tokenish_len(accepted_artifact) < tokenish_len(&parent));
    Ok(ScenarioOutcome::new(
        "SkillReducer arx_2603.29919",
        "destructive compression rejected; faithful routing compression accepted",
    ))
}

fn new_graph() -> (RunGraph<SkillProblem>, BudgetLedger) {
    (
        RunGraph::new(RunId::new()),
        BudgetLedger::new(Budget::metric_calls(100)),
    )
}

fn apply_change(
    ctx: &mut RunContext<'_, SkillProblem>,
    target: CandidateId,
    change: SkillChange,
    stage: &'static str,
) -> Result<CandidateId, Box<dyn std::error::Error>> {
    let report = ctx.record_proposal_batch(
        StageId::custom(stage),
        ProposalBatch {
            proposals: vec![Proposal::mutate(target, change).build()],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::metric_calls(1),
    )?;
    let applied = ctx.apply_batch(report.batch_id)?;
    Ok(applied
        .successful_candidates()
        .next()
        .expect("change proposal should apply"))
}

fn apply_create(
    ctx: &mut RunContext<'_, SkillProblem>,
    artifact: SkillRegistry,
    stage: &'static str,
) -> Result<CandidateId, Box<dyn std::error::Error>> {
    let report = ctx.record_proposal_batch(
        StageId::custom(stage),
        ProposalBatch {
            proposals: vec![
                Proposal::create(artifact)
                    .informed_by([leaven::InfoRef::External(ExternalRef {
                        kind: "paper".to_owned(),
                        id: "skill-reproduction".to_owned(),
                    })])
                    .build(),
            ],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::metric_calls(1),
    )?;
    let applied = ctx.apply_batch(report.batch_id)?;
    Ok(applied
        .successful_candidates()
        .next()
        .expect("create proposal should apply"))
}

fn apply_aggregate(
    ctx: &mut RunContext<'_, SkillProblem>,
    parents: Vec<CandidateId>,
    artifact: SkillRegistry,
    stage: &'static str,
) -> Result<CandidateId, Box<dyn std::error::Error>> {
    let report = ctx.record_proposal_batch(
        StageId::custom(stage),
        ProposalBatch {
            proposals: vec![Proposal::aggregate(parents, artifact).build()],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::metric_calls(1),
    )?;
    let applied = ctx.apply_batch(report.batch_id)?;
    Ok(applied
        .successful_candidates()
        .next()
        .expect("aggregate proposal should apply"))
}

fn registry(skills: impl IntoIterator<Item = Skill>) -> SkillRegistry {
    SkillRegistry {
        skills: skills
            .into_iter()
            .map(|skill| (skill.id.clone(), skill))
            .collect(),
    }
}

fn weak_skill() -> Skill {
    Skill {
        id: "debugger".to_owned(),
        route: "Use for coding failures.".to_owned(),
        body: "Try a fix.".to_owned(),
        references: "none".to_owned(),
        validation: "must pass a reproducer".to_owned(),
    }
}

fn scores(scores: &[(u64, f64)]) -> CasewiseEvidence<ScalarEvidence> {
    CasewiseEvidence::new(
        scores
            .iter()
            .map(|(case, score)| {
                CaseOutcome::new(CaseId::new(*case), ScalarEvidence::new(*score).unwrap())
            })
            .collect(),
    )
}

fn score_registry(registry: &SkillRegistry) -> usize {
    registry
        .skills
        .values()
        .filter(|skill| skill.body.contains("trace") || skill.body.contains("precondition"))
        .count()
}

fn preserves_behavior(registry: &SkillRegistry) -> bool {
    registry
        .skills
        .values()
        .any(|skill| skill.body.contains("failing assertion") || skill.body.contains("trace"))
}

fn tokenish_len(registry: &SkillRegistry) -> usize {
    registry
        .skills
        .values()
        .map(|skill| {
            skill.route.split_whitespace().count()
                + skill.body.split_whitespace().count()
                + skill.references.split_whitespace().count()
        })
        .sum()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillRegistry {
    skills: BTreeMap<String, Skill>,
}

impl SkillRegistry {
    fn with_skill(mut self, skill: Skill) -> Self {
        self.skills.insert(skill.id.clone(), skill);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Skill {
    id: String,
    route: String,
    body: String,
    references: String,
    validation: String,
}

impl Skill {
    fn lesson(id: &str, body: &str) -> Self {
        Self {
            id: id.to_owned(),
            route: format!("Use lesson {id} during consolidation."),
            body: body.to_owned(),
            references: "trace fixture".to_owned(),
            validation: "lesson only".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillChange {
    skill: String,
    field: SkillField,
    value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SkillField {
    Route,
    Body,
    References,
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum SkillPartId {
    Route(String),
    Body(String),
    References(String),
    Validation(String),
}

#[derive(Debug)]
struct SkillError;

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("skill registry error")
    }
}

impl std::error::Error for SkillError {}

impl Artifact for SkillRegistry {
    type Change = SkillChange;
    type ApplyError = SkillError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = Vec::new();
        for skill in self.skills.values() {
            bytes.extend_from_slice(skill.id.as_bytes());
            bytes.extend_from_slice(skill.route.as_bytes());
            bytes.extend_from_slice(skill.body.as_bytes());
            bytes.extend_from_slice(skill.references.as_bytes());
            bytes.extend_from_slice(skill.validation.as_bytes());
        }
        ArtifactIdentity::Content(content_id(&bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        let mut next = self.clone();
        let skill = next.skills.get_mut(&change.skill).ok_or(SkillError)?;
        match change.field {
            SkillField::Route => skill.route.clone_from(&change.value),
            SkillField::Body => skill.body.clone_from(&change.value),
            SkillField::References => skill.references.clone_from(&change.value),
            SkillField::Validation => skill.validation.clone_from(&change.value),
        }
        Ok(next)
    }
}

struct SkillSurface;

impl EditSurface<SkillRegistry> for SkillSurface {
    type PartId = SkillPartId;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(leaven_kernel::Fingerprint::from_bytes([5; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a SkillRegistry,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        let mut parts = Vec::new();
        for skill in artifact.skills.values() {
            parts.push(part(
                &skill.id,
                "route",
                SkillPartId::Route(skill.id.clone()),
                &skill.route,
            ));
            parts.push(part(
                &skill.id,
                "body",
                SkillPartId::Body(skill.id.clone()),
                &skill.body,
            ));
            parts.push(part(
                &skill.id,
                "references",
                SkillPartId::References(skill.id.clone()),
                &skill.references,
            ));
            parts.push(part(
                &skill.id,
                "validation",
                SkillPartId::Validation(skill.id.clone()),
                &skill.validation,
            ));
        }
        Ok(parts)
    }

    fn change_part(
        &self,
        artifact: &SkillRegistry,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<SkillChange, SurfaceError> {
        let (skill, field) = match id {
            SkillPartId::Route(skill) => (skill, SkillField::Route),
            SkillPartId::Body(skill) => (skill, SkillField::Body),
            SkillPartId::References(skill) => (skill, SkillField::References),
            SkillPartId::Validation(skill) => (skill, SkillField::Validation),
        };
        if artifact.skills.contains_key(&skill) {
            Ok(SkillChange {
                skill,
                field,
                value: edit,
            })
        } else {
            Err(SurfaceError::UnknownPart)
        }
    }
}

fn part<'a>(
    skill: &str,
    field: &str,
    id: SkillPartId,
    view: &'a str,
) -> Part<SkillPartId, PartAddress, &'a str> {
    Part {
        id,
        address: PartAddress(format!("skills/{skill}/{field}.md")),
        view,
    }
}

struct SkillProblem;

impl OptimizationProblem for SkillProblem {
    type Artifact = SkillRegistry;
    type Case = ();
    type Evidence = SkillUtilityEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug, Default)]
struct SkillUtilityEvidence {
    utilities: BTreeMap<String, FiniteF64>,
}

impl SkillUtilityEvidence {
    fn new(values: BTreeMap<String, f64>) -> Self {
        Self {
            utilities: values
                .into_iter()
                .map(|(skill, utility)| {
                    (
                        skill,
                        FiniteF64::new(utility).expect("utility evidence is finite"),
                    )
                })
                .collect(),
        }
    }
}

impl Evidence for SkillUtilityEvidence {}

impl AttributableEvidence<String> for SkillUtilityEvidence {
    fn attributions(&self) -> Vec<Attribution<String>> {
        self.utilities
            .iter()
            .map(|(key, weight)| Attribution {
                key: key.clone(),
                weight: Some(*weight),
                note: Some("paired skill utility".to_owned()),
            })
            .collect()
    }

    fn evidence_for(&self, key: &String) -> Option<String> {
        self.utilities
            .get(key)
            .map(|utility| format!("utility={}", utility.as_f64()))
    }
}

#[derive(Default)]
struct UtilityBank {
    skills: BTreeSet<String>,
    utilities: BTreeMap<String, FiniteF64>,
}

impl UtilityBank {
    fn observe(&mut self, evidence: &SkillUtilityEvidence) {
        for attribution in evidence.attributions() {
            self.skills.insert(attribution.key.clone());
            if let Some(weight) = attribution.weight {
                self.utilities.insert(attribution.key, weight);
            }
        }
    }

    fn prune_below(&mut self, threshold: FiniteF64) {
        self.skills.retain(|skill| {
            self.utilities
                .get(skill)
                .is_some_and(|utility| *utility >= threshold)
        });
    }

    fn utility(&self, skill: &str) -> FiniteF64 {
        *self.utilities.get(skill).unwrap_or(&FiniteF64::ZERO)
    }
}

struct SkillRegistryMaterializer;

impl Materializer<SkillProblem, SkillRegistry> for SkillRegistryMaterializer {
    async fn materialize_into(
        &self,
        value: &SkillRegistry,
        workspace: &mut WorkspaceView<'_>,
        _ctx: leaven::MaterializeContext<'_, SkillProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let mut files_written = 0;
        let mut bytes_written = 0_u64;
        for skill in value.skills.values() {
            for (name, content) in [
                ("ROUTING.md", &skill.route),
                ("BODY.md", &skill.body),
                ("REFERENCES.md", &skill.references),
                ("VALIDATION.md", &skill.validation),
            ] {
                let path = WorkspacePath::new(format!("skills/{}/{}", skill.id, name))?;
                workspace.write_file(&path, content.as_bytes())?;
                files_written += 1;
                bytes_written += u64::try_from(content.len()).expect("content length fits u64");
            }
        }
        Ok(Metered::new(
            MaterializationReport {
                files_written,
                bytes_written,
                truncations: Vec::new(),
            },
            Cost::metric_calls(1),
        ))
    }
}

struct ScenarioOutcome {
    paper: &'static str,
    proof: &'static str,
}

impl ScenarioOutcome {
    const fn new(paper: &'static str, proof: &'static str) -> Self {
        Self { paper, proof }
    }
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
