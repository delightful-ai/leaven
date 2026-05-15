use std::collections::BTreeSet;

use futures::executor::block_on;
use leaven::extend::{
    Arity, CachePolicy, CausalInputs, EvaluationRequest, Evaluator, InfoRef, Optimizer,
    ProposalBatchSemantics, ProposalContext, ProposalEffect, Proposer, RunEvent, RunGraphView,
    StepStatus, TrustPolicy,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, Budget, Cost,
    OptimizationProblem, Proposal, ProposalBatch,
};
use leaven_core::{
    EvaluationPurpose, EvaluationSet, ExternalRef, PartitionId, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CaseSet, EvaluationContext, EvaluationError, OptimizerError, ProposalError};
use leaven_evidence::ScalarEvidence;
use leaven_kernel::{
    AssessmentId, CandidateId, CaseId, EvaluatorId, Fingerprint, MetadataBag, Metered, ProposerId,
};
use leaven_population::KeepBest;
use leaven_store_inline::InlineEvidenceStore;

const SEARCH: &str = "SEARCH";
const VALIDATION: &str = "VALIDATION";
const TEST: &str = "TEST";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let cases = langprobe_lite_cases();
        let case_set = langprobe_lite_case_set(&cases);
        let evidence_store = InlineEvidenceStore::<PolicyEvaluationEvidence>::new("p6-inline");
        let evaluator = PolicyEvaluator {
            cases: cases.clone(),
        };
        let mut engine = leaven::engine::optimize::<OptimizerPolicyProblem>()
            .budget(Budget::metric_calls(80))
            .trust_policy(
                TrustPolicy::default()
                    .hide_from_proposers([PartitionId::from(VALIDATION), PartitionId::from(TEST)])
                    .hide_from_optimizers([PartitionId::from(TEST)]),
            )
            .evaluator(evaluator)
            .build();
        let seed = engine.insert_seed(OptimizerPolicy::seed(), 0)?;
        let mut optimizer = PolicySelfOptimizer {
            seed,
            proposer: LangProBeLitePolicyProposer,
            population: KeepBest::new(),
            best: None,
            done: false,
            child: None,
            hidden_test_refused: false,
            search_assessment: None,
            validation_assessment: None,
        };

        let result = engine
            .run(&mut optimizer, &case_set, &evidence_store)
            .await?;
        let best = result.best.expect("P6 optimizer produces a best candidate");
        let graph = engine.view();
        let best_policy = graph.artifact(best).expect("best policy exists");
        let test_audit =
            evaluate_policy(best_policy, &cases, Split::Test).expect("test partition has cases");

        assert_eq!(best, optimizer.child.expect("child recorded"));
        assert!(
            best_policy
                .rules
                .contains(&PolicyRule::ConditionalAnswerFormat)
        );
        assert!(best_policy.rules.contains(&PolicyRule::UseTraceFeedback));
        assert_eq!(best_policy.selector, CandidateSelectorPolicy::ParetoByCase);
        assert!((test_audit.average.score() - 1.0).abs() < f64::EPSILON);
        assert!(optimizer.hidden_test_refused);
        assert_eq!(graph.proposal_count(), 1);
        assert_eq!(graph.evaluation_request_count(), 4);
        assert_eq!(graph.assessment_count(), 4);

        println!(
            "p6 optimizer-policy self-opt: seed={seed} best={best} validation_score={:.2} test_audit={:.2} hidden_test_refused=true dataset=LangProBe-Lite/HotPotQA-Conditional",
            optimizer
                .population
                .best_score()
                .expect("validation score recorded"),
            test_audit.average.score()
        );
        Ok(())
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OptimizerPolicy {
    grounding: GroundingPolicy,
    selector: CandidateSelectorPolicy,
    minibatch_size: usize,
    instruction_weight: u8,
    demo_weight: u8,
    rules: BTreeSet<PolicyRule>,
}

impl OptimizerPolicy {
    fn seed() -> Self {
        Self {
            grounding: GroundingPolicy::ScoresOnly,
            selector: CandidateSelectorPolicy::BestAggregate,
            minibatch_size: 1,
            instruction_weight: 1,
            demo_weight: 3,
            rules: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OptimizerPolicyChange {
    grounding: GroundingPolicy,
    selector: CandidateSelectorPolicy,
    minibatch_size: usize,
    instruction_weight: u8,
    demo_weight: u8,
    add_rules: BTreeSet<PolicyRule>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GroundingPolicy {
    ScoresOnly,
    SearchTracesAndDataSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateSelectorPolicy {
    BestAggregate,
    ParetoByCase,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PolicyRule {
    ConditionalAnswerFormat,
    UseTraceFeedback,
    PreferCompactInstructions,
}

#[derive(Debug)]
struct OptimizerPolicyError;

impl std::fmt::Display for OptimizerPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("optimizer policy change is invalid")
    }
}

impl std::error::Error for OptimizerPolicyError {}

impl Artifact for OptimizerPolicy {
    type Change = OptimizerPolicyChange;
    type ApplyError = OptimizerPolicyError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(format!("{self:?}").as_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        if change.minibatch_size == 0 {
            return Err(OptimizerPolicyError);
        }
        let mut next = self.clone();
        next.grounding = change.grounding;
        next.selector = change.selector;
        next.minibatch_size = change.minibatch_size;
        next.instruction_weight = change.instruction_weight;
        next.demo_weight = change.demo_weight;
        next.rules.extend(change.add_rules.iter().copied());
        Ok(next)
    }
}

struct OptimizerPolicyProblem;

impl OptimizationProblem for OptimizerPolicyProblem {
    type Artifact = OptimizerPolicy;
    type Case = ConditionalQaCase;
    type Evidence = PolicyEvaluationEvidence;
    type ProposalAnnotations = PolicyProposalAnnotations;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PolicyProposalAnnotations {
    rationale: String,
    dataset_family: String,
    hidden_assessments_filtered: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct PolicyEvaluationEvidence {
    average: ScalarEvidence,
    runtime_cost_score: ScalarEvidence,
    outcomes: Vec<PolicyCaseOutcome>,
}

impl leaven::prelude::Evidence for PolicyEvaluationEvidence {}

#[derive(Clone, Debug, PartialEq)]
struct PolicyCaseOutcome {
    case: CaseId,
    predicted: String,
    expected: String,
    score: ScalarEvidence,
    trace_summary: String,
}

struct PolicySelfOptimizer {
    seed: CandidateId,
    proposer: LangProBeLitePolicyProposer,
    population: KeepBest,
    best: Option<CandidateId>,
    done: bool,
    child: Option<CandidateId>,
    hidden_test_refused: bool,
    search_assessment: Option<AssessmentId>,
    validation_assessment: Option<AssessmentId>,
}

impl Optimizer<OptimizerPolicyProblem> for PolicySelfOptimizer {
    async fn step(
        &mut self,
        ctx: &mut leaven::extend::RunContext<'_, OptimizerPolicyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.done {
            return Ok(StepStatus::Done);
        }

        let seed_search = evaluate_one(ctx, self.seed, SEARCH, EvaluationPurpose::Search)
            .await
            .map_err(|error| OptimizerError::with_source("seed search evaluation failed", error))?;
        let seed_validation =
            evaluate_one(ctx, self.seed, VALIDATION, EvaluationPurpose::Validation)
                .await
                .map_err(|error| {
                    OptimizerError::with_source("seed validation evaluation failed", error)
                })?;
        self.observe_validation(ctx, self.seed, seed_validation)?;

        let proposal_report = ctx
            .propose(
                &self.proposer,
                PolicyProposalRequest {
                    parent: self.seed,
                    assessment_refs: vec![seed_search, seed_validation],
                },
            )
            .await
            .map_err(|error| OptimizerError::with_source("policy proposal failed", error))?;
        let apply_report = ctx
            .apply_batch(proposal_report.batch_id)
            .map_err(|error| OptimizerError::with_source("policy proposal apply failed", error))?;
        let child = apply_report.successful_candidates().next().ok_or_else(|| {
            OptimizerError::Message("optimizer policy proposal did not apply".to_owned())
        })?;

        assert_eq!(ctx.graph().parents(child), vec![self.seed]);
        let proposal = ctx
            .graph()
            .proposal_that_created(child)
            .expect("child candidate has proposal");
        assert!(matches!(
            proposal.effect(),
            ProposalEffect::Change { target, .. } if *target == self.seed
        ));
        assert!(matches!(
            proposal.provenance().causal(),
            CausalInputs::Single(parent) if *parent == self.seed
        ));
        assert!(
            ctx.graph()
                .informed_by(child)
                .contains(&InfoRef::Assessment(seed_search))
        );
        assert!(
            !ctx.graph()
                .informed_by(child)
                .contains(&InfoRef::Assessment(seed_validation))
        );
        assert_eq!(proposal.annotations().hidden_assessments_filtered, 1);

        let child_search = evaluate_one(ctx, child, SEARCH, EvaluationPurpose::Search)
            .await
            .map_err(|error| {
                OptimizerError::with_source("child search evaluation failed", error)
            })?;
        let child_validation = evaluate_one(ctx, child, VALIDATION, EvaluationPurpose::Selection)
            .await
            .map_err(|error| {
                OptimizerError::with_source("child validation evaluation failed", error)
            })?;
        self.observe_validation(ctx, child, child_validation)?;

        let hidden_refusal = evaluate_one(ctx, child, TEST, EvaluationPurpose::FinalTest)
            .await
            .expect_err("TEST is hidden from optimizer contexts");
        assert!(hidden_refusal.to_string().contains("hidden"));

        self.best = self.population.best();
        self.child = Some(child);
        self.hidden_test_refused = true;
        self.search_assessment = Some(child_search);
        self.validation_assessment = Some(child_validation);
        self.done = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: RunGraphView<'_, OptimizerPolicyProblem>,
    ) -> Option<CandidateId> {
        self.best
    }
}

impl PolicySelfOptimizer {
    fn observe_validation(
        &mut self,
        ctx: &mut leaven::extend::RunContext<'_, OptimizerPolicyProblem>,
        candidate: CandidateId,
        assessment: AssessmentId,
    ) -> Result<(), OptimizerError> {
        let evidence = ctx.assessment_evidence(assessment).map_err(|error| {
            OptimizerError::with_source("validation evidence lookup failed", error)
        })?;
        emit_population_events(
            ctx,
            self.population
                .observe(candidate, assessment, evidence.average),
        );
        Ok(())
    }
}

async fn evaluate_one(
    ctx: &mut leaven::extend::RunContext<'_, OptimizerPolicyProblem>,
    candidate: CandidateId,
    partition: &'static str,
    purpose: EvaluationPurpose,
) -> Result<AssessmentId, leaven_engine::RunContextError> {
    Ok(ctx
        .evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![candidate],
                set: EvaluationSet::Partition(PartitionId::from(partition)),
                granularity: AssessmentGranularity::Aggregate,
                purpose,
            },
        )
        .await?
        .assessment_ids[0])
}

fn emit_population_events(
    ctx: &mut leaven::extend::RunContext<'_, OptimizerPolicyProblem>,
    events: Vec<leaven_engine::PopulationEvent>,
) {
    if !events.is_empty() {
        ctx.emit(RunEvent::PopulationUpdated {
            population_id: match &events[0] {
                leaven_engine::PopulationEvent::Inserted { population, .. }
                | leaven_engine::PopulationEvent::Replaced { population, .. }
                | leaven_engine::PopulationEvent::Removed { population, .. }
                | leaven_engine::PopulationEvent::Ignored { population, .. }
                | leaven_engine::PopulationEvent::Reweighted { population, .. } => *population,
            },
            events,
        });
    }
}

struct LangProBeLitePolicyProposer;

impl Proposer<OptimizerPolicyProblem> for LangProBeLitePolicyProposer {
    type Request = PolicyProposalRequest;

    fn id(&self) -> ProposerId {
        ProposerId::from("p6/langprobe-lite-policy-proposer")
    }

    fn arity(&self) -> Arity {
        Arity::Single
    }

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, OptimizerPolicyProblem>,
    ) -> Result<Metered<ProposalBatch<OptimizerPolicyProblem>>, ProposalError> {
        let parent = ctx
            .graph()
            .artifact(request.parent)
            .ok_or_else(|| ProposalError::Message("parent policy is not visible".to_owned()))?;
        assert_eq!(parent.grounding, GroundingPolicy::ScoresOnly);

        let mut visible_assessments = Vec::new();
        let mut filtered = 0_usize;
        for assessment in &request.assessment_refs {
            if ctx.graph().assessment(*assessment).is_some() {
                visible_assessments.push(*assessment);
            } else {
                filtered += 1;
            }
        }
        assert_eq!(visible_assessments.len(), 1);
        assert_eq!(filtered, 1);

        let change = OptimizerPolicyChange {
            grounding: GroundingPolicy::SearchTracesAndDataSummary,
            selector: CandidateSelectorPolicy::ParetoByCase,
            minibatch_size: 2,
            instruction_weight: 4,
            demo_weight: 1,
            add_rules: BTreeSet::from([
                PolicyRule::ConditionalAnswerFormat,
                PolicyRule::UseTraceFeedback,
                PolicyRule::PreferCompactInstructions,
            ]),
        };
        let mut refs = vec![
            InfoRef::Candidate(request.parent),
            InfoRef::External(ExternalRef {
                kind: "paper".to_owned(),
                id: "LangProBe/Tan25".to_owned(),
            }),
            InfoRef::External(ExternalRef {
                kind: "paper".to_owned(),
                id: "MIPRO/Ops24".to_owned(),
            }),
        ];
        refs.extend(visible_assessments.into_iter().map(InfoRef::Assessment));
        let annotations = PolicyProposalAnnotations {
            rationale: "HotPotQA-Conditional needs explicit answer-format rules; search traces and per-case Pareto selection are the first optimizer-policy knobs to mutate.".to_owned(),
            dataset_family: "LangProBe-Lite/HotPotQA-Conditional".to_owned(),
            hidden_assessments_filtered: filtered,
        };
        let proposal = Proposal::mutate(request.parent, change)
            .informed_by(refs)
            .annotations(annotations)
            .build();
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![proposal],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::llm_calls(1),
        ))
    }
}

struct PolicyProposalRequest {
    parent: CandidateId,
    assessment_refs: Vec<AssessmentId>,
}

struct PolicyEvaluator {
    cases: Vec<ConditionalQaCase>,
}

impl Evaluator<OptimizerPolicyProblem> for PolicyEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([6; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, OptimizerPolicyProblem>,
    ) -> Result<Metered<Vec<Assessment<OptimizerPolicyProblem>>>, EvaluationError> {
        let case_ids = request.set.case_ids.clone();
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "P6 evaluator only handles independent requests".to_owned(),
            ));
        };
        let mut assessments = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let policy = ctx
                .graph()
                .artifact(candidate)
                .ok_or_else(|| EvaluationError::Message("candidate not visible".to_owned()))?;
            let evidence = evaluate_policy_ids(policy, &self.cases, &case_ids)?;
            assessments.push(Assessment::Independent {
                candidate,
                target: AssessmentTarget::EvaluationSet(leaven_kernel::EvaluationSetId::new()),
                evidence,
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            });
        }
        Ok(Metered::new(assessments, Cost::metric_calls(1)))
    }
}

fn evaluate_policy_ids(
    policy: &OptimizerPolicy,
    cases: &[ConditionalQaCase],
    case_ids: &[CaseId],
) -> Result<PolicyEvaluationEvidence, EvaluationError> {
    let selected = case_ids
        .iter()
        .map(|id| {
            case_by_id(cases, *id)
                .ok_or_else(|| EvaluationError::Message(format!("unknown case id {id}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    evaluate_cases(policy, &selected)
}

fn evaluate_policy(
    policy: &OptimizerPolicy,
    cases: &[ConditionalQaCase],
    split: Split,
) -> Result<PolicyEvaluationEvidence, EvaluationError> {
    let selected = cases
        .iter()
        .filter(|case| case.split == split)
        .collect::<Vec<_>>();
    evaluate_cases(policy, &selected)
}

fn evaluate_cases(
    policy: &OptimizerPolicy,
    cases: &[&ConditionalQaCase],
) -> Result<PolicyEvaluationEvidence, EvaluationError> {
    if cases.is_empty() {
        return Err(EvaluationError::Message(
            "policy evaluation requires at least one case".to_owned(),
        ));
    }
    let outcomes = cases
        .iter()
        .map(|case| score_case(policy, case))
        .collect::<Result<Vec<_>, _>>()?;
    let total = outcomes
        .iter()
        .map(|outcome| outcome.score.score())
        .sum::<f64>();
    let count = u32::try_from(outcomes.len()).expect("fixture case count fits u32");
    let average = ScalarEvidence::new(total / f64::from(count)).map_err(|error| {
        EvaluationError::with_source("average score construction failed", error)
    })?;
    let runtime_cost = runtime_cost_score(policy)?;
    Ok(PolicyEvaluationEvidence {
        average,
        runtime_cost_score: runtime_cost,
        outcomes,
    })
}

fn score_case(
    policy: &OptimizerPolicy,
    case: &ConditionalQaCase,
) -> Result<PolicyCaseOutcome, EvaluationError> {
    let expected = case.expected_output();
    let has_format = policy.rules.contains(&PolicyRule::ConditionalAnswerFormat);
    let has_trace = policy.rules.contains(&PolicyRule::UseTraceFeedback);
    let has_grounding = policy.grounding == GroundingPolicy::SearchTracesAndDataSummary;
    let has_selector = policy.selector == CandidateSelectorPolicy::ParetoByCase;
    let answer_visible = !case.requires_multi_hop || (has_grounding && has_trace);
    let hard_case_handled = case.difficulty != Difficulty::Hard || has_selector;

    let mut score: f64 = 0.2;
    if has_format {
        score += 0.35;
    }
    if answer_visible {
        score += 0.25;
    }
    if hard_case_handled {
        score += 0.15;
    }
    if policy.minibatch_size >= 2 {
        score += 0.05;
    }
    let score = score.min(1.0);
    let predicted = if answer_visible && has_format {
        expected.clone()
    } else if answer_visible {
        case.answer.to_owned()
    } else {
        "unknown".to_owned()
    };
    let trace_summary = format!(
        "case={} answer_visible={answer_visible} conditional_format={has_format} selector={:?}",
        case.id, policy.selector
    );
    Ok(PolicyCaseOutcome {
        case: case.case_id,
        predicted,
        expected,
        score: ScalarEvidence::new(score)
            .map_err(|error| EvaluationError::with_source("case score failed", error))?,
        trace_summary,
    })
}

fn runtime_cost_score(policy: &OptimizerPolicy) -> Result<ScalarEvidence, EvaluationError> {
    let rule_count = u32::try_from(policy.rules.len()).expect("fixture rule count fits u32");
    let prompt_tokens = 1.0
        + f64::from(policy.instruction_weight)
        + f64::from(policy.demo_weight)
        + f64::from(rule_count);
    let score = 1.0 / prompt_tokens;
    ScalarEvidence::new(score)
        .map_err(|error| EvaluationError::with_source("runtime cost score failed", error))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConditionalQaCase {
    case_id: CaseId,
    id: &'static str,
    split: Split,
    question: &'static str,
    answer: &'static str,
    answer_kind: AnswerKind,
    difficulty: Difficulty,
    requires_multi_hop: bool,
}

impl ConditionalQaCase {
    fn expected_output(&self) -> String {
        format!("{}: {}", self.answer_kind.label(), self.answer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Split {
    Search,
    Validation,
    Test,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Difficulty {
    Easy,
    Hard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnswerKind {
    Person,
    Date,
    Place,
    Number,
}

impl AnswerKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Person => "PERSON",
            Self::Date => "DATE",
            Self::Place => "PLACE",
            Self::Number => "NUMBER",
        }
    }
}

fn langprobe_lite_cases() -> Vec<ConditionalQaCase> {
    vec![
        case(CaseFixture {
            index: 0,
            id: "search-person",
            split: Split::Search,
            question: "Who wrote the notes for the Analytical Engine?",
            answer: "Ada Lovelace",
            answer_kind: AnswerKind::Person,
            difficulty: Difficulty::Easy,
            requires_multi_hop: false,
        }),
        case(CaseFixture {
            index: 1,
            id: "search-place",
            split: Split::Search,
            question: "Which city hosted the fair where the Ferris wheel debuted?",
            answer: "Chicago",
            answer_kind: AnswerKind::Place,
            difficulty: Difficulty::Hard,
            requires_multi_hop: true,
        }),
        case(CaseFixture {
            index: 2,
            id: "search-date",
            split: Split::Search,
            question: "What year followed the treaty that ended the War of 1812?",
            answer: "1815",
            answer_kind: AnswerKind::Date,
            difficulty: Difficulty::Hard,
            requires_multi_hop: true,
        }),
        case(CaseFixture {
            index: 3,
            id: "validation-number",
            split: Split::Validation,
            question: "How many moons did Mars have in the classical astronomy prompt?",
            answer: "2",
            answer_kind: AnswerKind::Number,
            difficulty: Difficulty::Easy,
            requires_multi_hop: false,
        }),
        case(CaseFixture {
            index: 4,
            id: "validation-person",
            split: Split::Validation,
            question: "Who discovered penicillin after observing mold contamination?",
            answer: "Alexander Fleming",
            answer_kind: AnswerKind::Person,
            difficulty: Difficulty::Hard,
            requires_multi_hop: true,
        }),
        case(CaseFixture {
            index: 5,
            id: "test-place",
            split: Split::Test,
            question: "What city contains the museum that houses the Mona Lisa?",
            answer: "Paris",
            answer_kind: AnswerKind::Place,
            difficulty: Difficulty::Hard,
            requires_multi_hop: true,
        }),
        case(CaseFixture {
            index: 6,
            id: "test-date",
            split: Split::Test,
            question: "What year did Apollo 11 land on the Moon?",
            answer: "1969",
            answer_kind: AnswerKind::Date,
            difficulty: Difficulty::Easy,
            requires_multi_hop: false,
        }),
    ]
}

#[derive(Clone, Copy)]
struct CaseFixture {
    index: usize,
    id: &'static str,
    split: Split,
    question: &'static str,
    answer: &'static str,
    answer_kind: AnswerKind,
    difficulty: Difficulty,
    requires_multi_hop: bool,
}

fn case(input: CaseFixture) -> ConditionalQaCase {
    ConditionalQaCase {
        case_id: CaseId::from_index(input.index),
        id: input.id,
        split: input.split,
        question: input.question,
        answer: input.answer,
        answer_kind: input.answer_kind,
        difficulty: input.difficulty,
        requires_multi_hop: input.requires_multi_hop,
    }
}

fn langprobe_lite_case_set(cases: &[ConditionalQaCase]) -> CaseSet<ConditionalQaCase> {
    CaseSet::new(cases.to_vec())
        .with_partition(PartitionId::from(SEARCH), ids_for(cases, Split::Search))
        .with_partition(
            PartitionId::from(VALIDATION),
            ids_for(cases, Split::Validation),
        )
        .with_partition(PartitionId::from(TEST), ids_for(cases, Split::Test))
}

fn ids_for(cases: &[ConditionalQaCase], split: Split) -> Vec<CaseId> {
    cases
        .iter()
        .filter_map(|case| (case.split == split).then_some(case.case_id))
        .collect()
}

fn case_by_id(cases: &[ConditionalQaCase], id: CaseId) -> Option<&ConditionalQaCase> {
    usize::try_from(id.0)
        .ok()
        .and_then(|index| cases.get(index))
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
