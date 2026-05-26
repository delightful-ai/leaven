use std::collections::BTreeSet;

use futures::executor::block_on;
use leaven::extend::{
    Arity, AssessmentGranularity, AssessmentTarget, CachePolicy, CausalInputs, EvaluationRequest,
    Evaluator, InfoRef, Optimizer, Proposal, ProposalBatch, ProposalBatchSemantics,
    ProposalContext, ProposalEffect, Proposer, RunEvent, RunGraphView, StepStatus, TrustPolicy,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{Artifact, ArtifactIdentity, Assessment, Budget, Cost, OptimizationProblem};
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

const PUBLIC_TRAIN: &str = "PUBLIC_TRAIN";
const REGRESSION: &str = "REGRESSION";
const ADVERSARIAL: &str = "ADVERSARIAL";
const PRIVATE_HOLDOUT: &str = "PRIVATE_HOLDOUT";
const FINAL_TEST: &str = "FINAL_TEST";
const IMMUTABLE_SURFACES: [ForbiddenSurface; 8] = [
    ForbiddenSurface::FinalEvaluator,
    ForbiddenSurface::PrivateHoldoutSet,
    ForbiddenSurface::SandboxPolicy,
    ForbiddenSurface::DeploymentGate,
    ForbiddenSurface::AuditLog,
    ForbiddenSurface::RollbackMechanism,
    ForbiddenSurface::SecretStore,
    ForbiddenSurface::ResourceLimits,
];
const HARD_GATES: [HardGateFailure; 6] = [
    HardGateFailure::Regression,
    HardGateFailure::SandboxEscape,
    HardGateFailure::EvaluatorTampering,
    HardGateFailure::HiddenStateChannel,
    HardGateFailure::UnauthorizedAccess,
    HardGateFailure::AuditabilityDegraded,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let cases = self_optimization_cases();
        let case_set = self_optimization_case_set(&cases);
        let evidence_store = InlineEvidenceStore::<AgentRepoEvidence>::new("p7-inline");
        let evaluator = AgentRepoEvaluator {
            cases: cases.clone(),
        };
        let mut engine = leaven::engine::optimize::<SelfOptimizationProblem>()
            .budget(Budget::metric_calls(120))
            .trust_policy(
                TrustPolicy::default()
                    .hide_from_proposers([
                        PartitionId::from(PRIVATE_HOLDOUT),
                        PartitionId::from(FINAL_TEST),
                    ])
                    .hide_from_optimizers([
                        PartitionId::from(PRIVATE_HOLDOUT),
                        PartitionId::from(FINAL_TEST),
                    ]),
            )
            .evaluator(evaluator)
            .build();
        let incumbent = engine.insert_seed(AgentRepo::incumbent(), 0)?;
        let mut optimizer = ArchiveSelfOptimizer {
            incumbent,
            proposer: SelfPatchProposer,
            archive: EvolutionArchive::new(incumbent),
            public_population: KeepBest::new(),
            generation: 0,
            max_generations: 2,
            best: Some(incumbent),
            hidden_holdout_refused: false,
            generated_children: Vec::new(),
        };

        let result = engine
            .run(&mut optimizer, &case_set, &evidence_store)
            .await?;
        let public_best = result.best.expect("archive optimizer returns public best");
        let graph = engine.view();
        let kernel = OuterPromotionKernel {
            cases: cases.clone(),
        };
        let promotion = kernel
            .promote_with_rollback(&graph, incumbent, &optimizer.generated_children)
            .expect("promotion gate has a promoted descendant");
        let promoted_repo = graph
            .artifact(promotion.promoted)
            .expect("promoted repo exists");

        assert_eq!(public_best, promotion.promoted);
        assert_eq!(optimizer.generated_children.len(), 2);
        assert!(optimizer.hidden_holdout_refused);
        assert!(promoted_repo.has(SearchStrategy::LineageArchive));
        assert!(promoted_repo.has(VerificationStrategy::PrivateCanaryAwareness));
        assert!(promoted_repo.has(MetaOptimizer::DescendantScoring));
        assert!(!promoted_repo.has_forbidden_surface());
        assert_eq!(promotion.rollback, incumbent);
        assert!(
            promotion.private_holdout.score.score() > promotion.incumbent_private.score.score()
        );
        assert!(promotion.final_test.score.score() > promotion.incumbent_final.score.score());
        assert_eq!(graph.proposal_count(), 2);
        assert_eq!(graph.evaluation_request_count(), 7);
        assert_eq!(graph.assessment_count(), 7);
        assert_eq!(OuterPromotionKernel::immutable_surface_count(), 8);
        assert_eq!(OuterPromotionKernel::hard_gate_count(), 6);

        println!(
            "p7 self-optimization kernel: incumbent={incumbent} public_best={public_best} promoted={} archive_nodes={} private_holdout={:.2} final_test={:.2} rollback={} immutable_gate=true",
            promotion.promoted,
            optimizer.archive.len(),
            promotion.private_holdout.score.score(),
            promotion.final_test.score.score(),
            promotion.rollback
        );
        Ok(())
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentRepo {
    prompts: BTreeSet<PromptModule>,
    tools: BTreeSet<ToolModule>,
    planner: PlannerModule,
    memory: MemoryPolicy,
    search: BTreeSet<SearchStrategy>,
    verification: BTreeSet<VerificationStrategy>,
    editing: EditingPolicy,
    tests: BTreeSet<TestGenerationPolicy>,
    decomposition: DecompositionPolicy,
    self_debugging: BTreeSet<SelfDebuggingPolicy>,
    meta_optimizer: BTreeSet<MetaOptimizer>,
    forbidden_surface: BTreeSet<ForbiddenSurface>,
}

impl AgentRepo {
    fn incumbent() -> Self {
        Self {
            prompts: BTreeSet::from([PromptModule::Baseline]),
            tools: BTreeSet::from([ToolModule::PlainSearch]),
            planner: PlannerModule::Linear,
            memory: MemoryPolicy::RecentOnly,
            search: BTreeSet::from([SearchStrategy::GreedyChampion]),
            verification: BTreeSet::from([VerificationStrategy::UnitTestsOnly]),
            editing: EditingPolicy::DirectPatch,
            tests: BTreeSet::from([TestGenerationPolicy::None]),
            decomposition: DecompositionPolicy::SingleThread,
            self_debugging: BTreeSet::new(),
            meta_optimizer: BTreeSet::new(),
            forbidden_surface: BTreeSet::new(),
        }
    }

    fn apply_allowed_mutation(&self, mutation: &RepoMutation) -> Self {
        let mut next = self.clone();
        next.prompts.extend(mutation.add_prompts.iter().copied());
        next.tools.extend(mutation.add_tools.iter().copied());
        if let Some(planner) = mutation.planner {
            next.planner = planner;
        }
        if let Some(memory) = mutation.memory {
            next.memory = memory;
        }
        next.search.extend(mutation.add_search.iter().copied());
        next.verification
            .extend(mutation.add_verification.iter().copied());
        if let Some(editing) = mutation.editing {
            next.editing = editing;
        }
        next.tests.extend(mutation.add_tests.iter().copied());
        if let Some(decomposition) = mutation.decomposition {
            next.decomposition = decomposition;
        }
        next.self_debugging
            .extend(mutation.add_self_debugging.iter().copied());
        next.meta_optimizer
            .extend(mutation.add_meta_optimizer.iter().copied());
        next
    }

    fn has(&self, item: impl RepoFeature) -> bool {
        item.present_in(self)
    }

    fn has_forbidden_surface(&self) -> bool {
        !self.forbidden_surface.is_empty()
    }
}

trait RepoFeature {
    fn present_in(self, repo: &AgentRepo) -> bool;
}

impl RepoFeature for SearchStrategy {
    fn present_in(self, repo: &AgentRepo) -> bool {
        repo.search.contains(&self)
    }
}

impl RepoFeature for VerificationStrategy {
    fn present_in(self, repo: &AgentRepo) -> bool {
        repo.verification.contains(&self)
    }
}

impl RepoFeature for MetaOptimizer {
    fn present_in(self, repo: &AgentRepo) -> bool {
        repo.meta_optimizer.contains(&self)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AgentRepoPatch {
    mutation: RepoMutation,
    attempted_forbidden_edits: BTreeSet<ForbiddenSurface>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RepoMutation {
    add_prompts: BTreeSet<PromptModule>,
    add_tools: BTreeSet<ToolModule>,
    planner: Option<PlannerModule>,
    memory: Option<MemoryPolicy>,
    add_search: BTreeSet<SearchStrategy>,
    add_verification: BTreeSet<VerificationStrategy>,
    editing: Option<EditingPolicy>,
    add_tests: BTreeSet<TestGenerationPolicy>,
    decomposition: Option<DecompositionPolicy>,
    add_self_debugging: BTreeSet<SelfDebuggingPolicy>,
    add_meta_optimizer: BTreeSet<MetaOptimizer>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PromptModule {
    Baseline,
    FailureCaseReflection,
    PromotionDiffSummary,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ToolModule {
    PlainSearch,
    CodeSearch,
    FailureClusterer,
    TraceVisualizer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannerModule {
    Linear,
    PlanCriticExecutor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MemoryPolicy {
    RecentOnly,
    FailureAndLineageMemory,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SearchStrategy {
    GreedyChampion,
    NoveltySampling,
    LineageArchive,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum VerificationStrategy {
    UnitTestsOnly,
    RegressionGate,
    AdversarialCanaries,
    PrivateCanaryAwareness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditingPolicy {
    DirectPatch,
    PullRequestWithRollbackNotes,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TestGenerationPolicy {
    None,
    PropertyTests,
    BenchmarkMinimization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecompositionPolicy {
    SingleThread,
    PlannerVerifierSplit,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SelfDebuggingPolicy {
    RetryWithTrace,
    RootCauseBeforePatch,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MetaOptimizer {
    DescendantScoring,
    ParentSelectionAudit,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ForbiddenSurface {
    FinalEvaluator,
    PrivateHoldoutSet,
    SandboxPolicy,
    DeploymentGate,
    AuditLog,
    RollbackMechanism,
    SecretStore,
    ResourceLimits,
}

#[derive(Debug)]
struct AgentRepoPatchError {
    attempted: BTreeSet<ForbiddenSurface>,
}

impl std::fmt::Display for AgentRepoPatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "agent repo patch attempted forbidden outer-kernel edits: {:?}",
            self.attempted
        )
    }
}

impl std::error::Error for AgentRepoPatchError {}

impl Artifact for AgentRepo {
    type Change = AgentRepoPatch;
    type ApplyError = AgentRepoPatchError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(format!("{self:?}").as_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        if !change.attempted_forbidden_edits.is_empty() {
            return Err(AgentRepoPatchError {
                attempted: change.attempted_forbidden_edits.clone(),
            });
        }
        Ok(self.apply_allowed_mutation(&change.mutation))
    }
}

struct SelfOptimizationProblem;

impl OptimizationProblem for SelfOptimizationProblem {
    type Artifact = AgentRepo;
    type Case = SelfOptimizationCase;
    type Evidence = AgentRepoEvidence;
    type ProposalAnnotations = SelfPatchAnnotations;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SelfPatchAnnotations {
    rationale: String,
    allowed_surface: BTreeSet<MutableSurface>,
    blocked_forbidden_edits: BTreeSet<ForbiddenSurface>,
    expected_metaproductivity_gain: u8,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MutableSurface {
    Prompts,
    Tools,
    Planner,
    Memory,
    SearchStrategy,
    VerificationStrategy,
    CodeEditingPolicy,
    TestGeneration,
    SelfDebugging,
    TaskDecomposition,
    MetaOptimization,
}

#[derive(Clone, Debug, PartialEq)]
struct AgentRepoEvidence {
    score: ScalarEvidence,
    cost_score: ScalarEvidence,
    safety_score: ScalarEvidence,
    novelty_score: ScalarEvidence,
    metaproductivity_score: ScalarEvidence,
    outcomes: Vec<RepoTaskOutcome>,
    hard_gate_failures: BTreeSet<HardGateFailure>,
}

impl leaven::prelude::Evidence for AgentRepoEvidence {}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HardGateFailure {
    Regression,
    SandboxEscape,
    EvaluatorTampering,
    HiddenStateChannel,
    UnauthorizedAccess,
    AuditabilityDegraded,
}

#[derive(Clone, Debug, PartialEq)]
struct RepoTaskOutcome {
    case: CaseId,
    family: TaskFamily,
    score: ScalarEvidence,
    trace: String,
}

struct ArchiveSelfOptimizer {
    incumbent: CandidateId,
    proposer: SelfPatchProposer,
    archive: EvolutionArchive,
    public_population: KeepBest,
    generation: usize,
    max_generations: usize,
    best: Option<CandidateId>,
    hidden_holdout_refused: bool,
    generated_children: Vec<CandidateId>,
}

impl Optimizer<SelfOptimizationProblem> for ArchiveSelfOptimizer {
    async fn step(
        &mut self,
        ctx: &mut leaven::extend::RunContext<'_, SelfOptimizationProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.generation >= self.max_generations {
            return Ok(StepStatus::Done);
        }
        if self.generation == 0 {
            let seed_assessment =
                evaluate_one(ctx, self.incumbent, PUBLIC_TRAIN, EvaluationPurpose::Search)
                    .await
                    .map_err(|error| {
                        OptimizerError::with_source("incumbent public evaluation failed", error)
                    })?;
            self.observe(ctx, self.incumbent, seed_assessment)?;
            self.archive
                .record_assessment(ctx, self.incumbent, seed_assessment)?;
        }

        let parent = self.archive.select_parent();
        let proposal_report = ctx
            .propose(
                &self.proposer,
                SelfPatchRequest {
                    generation: self.generation,
                    parent,
                    visible_assessments: self.archive.visible_assessments(parent),
                },
            )
            .await
            .map_err(|error| OptimizerError::with_source("self patch proposal failed", error))?;
        let apply_report = ctx
            .apply_batch(proposal_report.batch_id)
            .map_err(|error| OptimizerError::with_source("self patch apply failed", error))?;
        let child = apply_report.successful_candidates().next().ok_or_else(|| {
            OptimizerError::Message("self patch did not create a child candidate".to_owned())
        })?;
        let proposal = ctx
            .graph()
            .proposal_that_created(child)
            .expect("child has proposal");
        assert!(matches!(
            proposal.effect(),
            ProposalEffect::Change { target, .. } if *target == parent
        ));
        assert!(matches!(
            proposal.provenance().causal(),
            CausalInputs::Single(causal_parent) if *causal_parent == parent
        ));
        assert!(
            !proposal
                .annotations()
                .allowed_surface
                .contains(&MutableSurface::MetaOptimization)
                || self.generation > 0
        );
        assert!(proposal.annotations().blocked_forbidden_edits.is_empty());

        let public_assessment = evaluate_one(ctx, child, PUBLIC_TRAIN, EvaluationPurpose::Search)
            .await
            .map_err(|error| OptimizerError::with_source("child public eval failed", error))?;
        let regression_assessment =
            evaluate_one(ctx, child, REGRESSION, EvaluationPurpose::Validation)
                .await
                .map_err(|error| {
                    OptimizerError::with_source("child regression eval failed", error)
                })?;
        let adversarial_assessment =
            evaluate_one(ctx, child, ADVERSARIAL, EvaluationPurpose::Validation)
                .await
                .map_err(|error| {
                    OptimizerError::with_source("child adversarial eval failed", error)
                })?;
        if !self.hidden_holdout_refused {
            let refusal = evaluate_one(ctx, child, PRIVATE_HOLDOUT, EvaluationPurpose::FinalTest)
                .await
                .expect_err("private holdout is hidden from optimizer contexts");
            assert!(refusal.to_string().contains("hidden"));
            self.hidden_holdout_refused = true;
        }

        self.archive.add(parent, child);
        self.observe(ctx, child, public_assessment)?;
        self.archive
            .record_assessment(ctx, child, public_assessment)?;
        self.archive
            .record_assessment(ctx, child, regression_assessment)?;
        self.archive
            .record_assessment(ctx, child, adversarial_assessment)?;
        self.generated_children.push(child);
        self.best = self.public_population.best();
        self.generation += 1;
        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        _graph: RunGraphView<'_, SelfOptimizationProblem>,
    ) -> Option<CandidateId> {
        self.best
    }
}

impl ArchiveSelfOptimizer {
    fn observe(
        &mut self,
        ctx: &mut leaven::extend::RunContext<'_, SelfOptimizationProblem>,
        candidate: CandidateId,
        assessment: AssessmentId,
    ) -> Result<(), OptimizerError> {
        let evidence = ctx.assessment_evidence(assessment).map_err(|error| {
            OptimizerError::with_source("assessment evidence lookup failed", error)
        })?;
        let events = self
            .public_population
            .observe(candidate, assessment, evidence.score);
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
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct EvolutionArchive {
    nodes: Vec<ArchiveNode>,
}

impl EvolutionArchive {
    fn new(seed: CandidateId) -> Self {
        Self {
            nodes: vec![ArchiveNode {
                candidate: seed,
                parent: None,
                assessments: Vec::new(),
            }],
        }
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn add(&mut self, parent: CandidateId, child: CandidateId) {
        self.nodes.push(ArchiveNode {
            candidate: child,
            parent: Some(parent),
            assessments: Vec::new(),
        });
    }

    fn record_assessment(
        &mut self,
        ctx: &leaven::extend::RunContext<'_, SelfOptimizationProblem>,
        candidate: CandidateId,
        assessment: AssessmentId,
    ) -> Result<(), OptimizerError> {
        let evidence = ctx.assessment_evidence(assessment).map_err(|error| {
            OptimizerError::with_source("archive evidence lookup failed", error)
        })?;
        let node = self
            .nodes
            .iter_mut()
            .find(|node| node.candidate == candidate)
            .ok_or_else(|| OptimizerError::Message("archive node is missing".to_owned()))?;
        node.assessments.push(ArchiveAssessment {
            assessment,
            objective: evidence.score.score(),
            novelty: evidence.novelty_score.score(),
            metaproductivity: evidence.metaproductivity_score.score(),
            cost: evidence.cost_score.score(),
            safety: evidence.safety_score.score(),
        });
        Ok(())
    }

    fn visible_assessments(&self, candidate: CandidateId) -> Vec<AssessmentId> {
        self.nodes
            .iter()
            .find(|node| node.candidate == candidate)
            .map(|node| {
                node.assessments
                    .iter()
                    .map(|assessment| assessment.assessment)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn select_parent(&self) -> CandidateId {
        self.nodes
            .iter()
            .max_by(|left, right| {
                left.selection_score()
                    .partial_cmp(&right.selection_score())
                    .expect("archive scores are finite")
            })
            .expect("archive has at least one node")
            .candidate
    }
}

#[derive(Clone, Debug)]
struct ArchiveNode {
    candidate: CandidateId,
    parent: Option<CandidateId>,
    assessments: Vec<ArchiveAssessment>,
}

impl ArchiveNode {
    fn selection_score(&self) -> f64 {
        let Some(best) = self.assessments.iter().max_by(|left, right| {
            left.objective
                .partial_cmp(&right.objective)
                .expect("archive objective scores are finite")
        }) else {
            return if self.parent.is_none() { 0.1 } else { 0.0 };
        };
        best.objective + 0.4 * best.metaproductivity + 0.2 * best.novelty + 0.2 * best.safety
            - 0.1 * (1.0 - best.cost)
    }
}

#[derive(Clone, Debug)]
struct ArchiveAssessment {
    assessment: AssessmentId,
    objective: f64,
    novelty: f64,
    metaproductivity: f64,
    cost: f64,
    safety: f64,
}

struct SelfPatchProposer;

#[derive(Clone, Debug)]
struct SelfPatchRequest {
    generation: usize,
    parent: CandidateId,
    visible_assessments: Vec<AssessmentId>,
}

impl Proposer<SelfOptimizationProblem> for SelfPatchProposer {
    type Request = SelfPatchRequest;

    fn id(&self) -> ProposerId {
        ProposerId::from("p7/self-patch-proposer")
    }

    fn arity(&self) -> Arity {
        Arity::Single
    }

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, SelfOptimizationProblem>,
    ) -> Result<Metered<ProposalBatch<SelfOptimizationProblem>>, ProposalError> {
        let parent = ctx
            .graph()
            .artifact(request.parent)
            .ok_or_else(|| ProposalError::Message("parent repo is not visible".to_owned()))?;
        for assessment in &request.visible_assessments {
            assert!(ctx.graph().assessment(*assessment).is_some());
        }
        let mutation = if request.generation == 0 {
            assert_eq!(parent.planner, PlannerModule::Linear);
            RepoMutation {
                add_prompts: BTreeSet::from([PromptModule::FailureCaseReflection]),
                add_tools: BTreeSet::from([
                    ToolModule::CodeSearch,
                    ToolModule::FailureClusterer,
                    ToolModule::TraceVisualizer,
                ]),
                planner: Some(PlannerModule::PlanCriticExecutor),
                memory: Some(MemoryPolicy::FailureAndLineageMemory),
                add_search: BTreeSet::from([SearchStrategy::NoveltySampling]),
                add_verification: BTreeSet::from([
                    VerificationStrategy::RegressionGate,
                    VerificationStrategy::AdversarialCanaries,
                ]),
                editing: Some(EditingPolicy::PullRequestWithRollbackNotes),
                add_tests: BTreeSet::from([TestGenerationPolicy::PropertyTests]),
                decomposition: Some(DecompositionPolicy::PlannerVerifierSplit),
                add_self_debugging: BTreeSet::from([SelfDebuggingPolicy::RetryWithTrace]),
                add_meta_optimizer: BTreeSet::new(),
            }
        } else {
            assert!(parent.search.contains(&SearchStrategy::NoveltySampling));
            RepoMutation {
                add_prompts: BTreeSet::from([PromptModule::PromotionDiffSummary]),
                add_tools: BTreeSet::new(),
                planner: None,
                memory: None,
                add_search: BTreeSet::from([SearchStrategy::LineageArchive]),
                add_verification: BTreeSet::from([VerificationStrategy::PrivateCanaryAwareness]),
                editing: None,
                add_tests: BTreeSet::from([TestGenerationPolicy::BenchmarkMinimization]),
                decomposition: None,
                add_self_debugging: BTreeSet::from([SelfDebuggingPolicy::RootCauseBeforePatch]),
                add_meta_optimizer: BTreeSet::from([
                    MetaOptimizer::DescendantScoring,
                    MetaOptimizer::ParentSelectionAudit,
                ]),
            }
        };
        let allowed_surface = allowed_surface_for(&mutation);
        let annotations = SelfPatchAnnotations {
            rationale: if request.generation == 0 {
                "convert the optimizer from direct self-editing into PR-shaped descendants with better failure visibility".to_owned()
            } else {
                "add archive lineage search and descendant scoring so parents are selected for generativity, not only immediate score".to_owned()
            },
            allowed_surface,
            blocked_forbidden_edits: BTreeSet::new(),
            expected_metaproductivity_gain: u8::try_from(request.generation + 1)
                .expect("fixture generation fits u8"),
        };
        let mut refs = vec![
            InfoRef::Candidate(request.parent),
            InfoRef::External(ExternalRef {
                kind: "design".to_owned(),
                id: "immutable-evaluator-boundary".to_owned(),
            }),
        ];
        refs.extend(
            request
                .visible_assessments
                .into_iter()
                .map(InfoRef::Assessment),
        );
        let proposal = Proposal::mutate(
            request.parent,
            AgentRepoPatch {
                mutation,
                attempted_forbidden_edits: BTreeSet::new(),
            },
        )
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

fn allowed_surface_for(mutation: &RepoMutation) -> BTreeSet<MutableSurface> {
    let mut surface = BTreeSet::new();
    if !mutation.add_prompts.is_empty() {
        surface.insert(MutableSurface::Prompts);
    }
    if !mutation.add_tools.is_empty() {
        surface.insert(MutableSurface::Tools);
    }
    if mutation.planner.is_some() {
        surface.insert(MutableSurface::Planner);
    }
    if mutation.memory.is_some() {
        surface.insert(MutableSurface::Memory);
    }
    if !mutation.add_search.is_empty() {
        surface.insert(MutableSurface::SearchStrategy);
    }
    if !mutation.add_verification.is_empty() {
        surface.insert(MutableSurface::VerificationStrategy);
    }
    if mutation.editing.is_some() {
        surface.insert(MutableSurface::CodeEditingPolicy);
    }
    if !mutation.add_tests.is_empty() {
        surface.insert(MutableSurface::TestGeneration);
    }
    if mutation.decomposition.is_some() {
        surface.insert(MutableSurface::TaskDecomposition);
    }
    if !mutation.add_self_debugging.is_empty() {
        surface.insert(MutableSurface::SelfDebugging);
    }
    if !mutation.add_meta_optimizer.is_empty() {
        surface.insert(MutableSurface::MetaOptimization);
    }
    surface
}

async fn evaluate_one(
    ctx: &mut leaven::extend::RunContext<'_, SelfOptimizationProblem>,
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

struct AgentRepoEvaluator {
    cases: Vec<SelfOptimizationCase>,
}

impl Evaluator<SelfOptimizationProblem> for AgentRepoEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, SelfOptimizationProblem>,
    ) -> Result<Metered<Vec<Assessment<SelfOptimizationProblem>>>, EvaluationError> {
        let case_ids = request.set.case_ids.clone();
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "P7 only supports independent evaluation".to_owned(),
            ));
        };
        let mut assessments = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let repo = ctx
                .graph()
                .artifact(candidate)
                .ok_or_else(|| EvaluationError::Message("candidate repo missing".to_owned()))?;
            let selected = case_ids
                .iter()
                .map(|id| {
                    case_by_id(&self.cases, *id)
                        .ok_or_else(|| EvaluationError::Message(format!("unknown case {id}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let evidence = evaluate_repo(repo, &selected)?;
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

fn evaluate_repo(
    repo: &AgentRepo,
    cases: &[&SelfOptimizationCase],
) -> Result<AgentRepoEvidence, EvaluationError> {
    if cases.is_empty() {
        return Err(EvaluationError::Message(
            "agent repo evaluation requires at least one case".to_owned(),
        ));
    }
    let outcomes = cases
        .iter()
        .map(|case| score_case(repo, case))
        .collect::<Result<Vec<_>, _>>()?;
    let total = outcomes
        .iter()
        .map(|outcome| outcome.score.score())
        .sum::<f64>();
    let count = u32::try_from(outcomes.len()).expect("fixture case count fits u32");
    let score = scalar(total / f64::from(count), "aggregate repo score")?;
    let cost_score = scalar(cost_score(repo), "repo cost score")?;
    let safety_score = scalar(safety_score(repo), "repo safety score")?;
    let novelty_score = scalar(novelty_score(repo), "repo novelty score")?;
    let metaproductivity_score =
        scalar(metaproductivity_score(repo), "repo metaproductivity score")?;
    let hard_gate_failures = hard_gate_failures(repo, &outcomes);
    Ok(AgentRepoEvidence {
        score,
        cost_score,
        safety_score,
        novelty_score,
        metaproductivity_score,
        outcomes,
        hard_gate_failures,
    })
}

fn score_case(
    repo: &AgentRepo,
    case: &SelfOptimizationCase,
) -> Result<RepoTaskOutcome, EvaluationError> {
    let (mut score, trace) = family_score(repo, case.family);
    if case.requires_hidden_generalization
        && repo
            .verification
            .contains(&VerificationStrategy::PrivateCanaryAwareness)
    {
        score += 0.10;
    }
    if case.requires_metaproductivity
        && repo
            .meta_optimizer
            .contains(&MetaOptimizer::DescendantScoring)
    {
        score += 0.15;
    }
    Ok(RepoTaskOutcome {
        case: case.case_id,
        family: case.family,
        score: scalar(score.min(1.0), "case score")?,
        trace: format!("{}: {trace}", case.id),
    })
}

fn family_score(repo: &AgentRepo, family: TaskFamily) -> (f64, &'static str) {
    match family {
        TaskFamily::BugFix => bugfix_score(repo),
        TaskFamily::Performance => performance_score(repo),
        TaskFamily::TestGeneration => test_generation_score(repo),
        TaskFamily::Robustness => robustness_score(repo),
        TaskFamily::Adversarial => adversarial_score(repo),
    }
}

fn bugfix_score(repo: &AgentRepo) -> (f64, &'static str) {
    let mut score = 0.20;
    if repo.tools.contains(&ToolModule::CodeSearch) {
        score += 0.25;
    }
    if repo.planner == PlannerModule::PlanCriticExecutor {
        score += 0.20;
    }
    if repo
        .self_debugging
        .contains(&SelfDebuggingPolicy::RootCauseBeforePatch)
    {
        score += 0.25;
    }
    (
        score,
        "bugfix needs code search, plan/critic split, and root-cause debugging",
    )
}

fn performance_score(repo: &AgentRepo) -> (f64, &'static str) {
    let mut score = 0.20;
    if repo
        .tests
        .contains(&TestGenerationPolicy::BenchmarkMinimization)
    {
        score += 0.30;
    }
    if repo.search.contains(&SearchStrategy::LineageArchive) {
        score += 0.25;
    }
    if repo
        .meta_optimizer
        .contains(&MetaOptimizer::DescendantScoring)
    {
        score += 0.20;
    }
    (
        score,
        "performance work rewards benchmark minimization and lineage search",
    )
}

fn test_generation_score(repo: &AgentRepo) -> (f64, &'static str) {
    let mut score = 0.20;
    if repo.tests.contains(&TestGenerationPolicy::PropertyTests) {
        score += 0.25;
    }
    if repo.tools.contains(&ToolModule::FailureClusterer) {
        score += 0.20;
    }
    if repo.memory == MemoryPolicy::FailureAndLineageMemory {
        score += 0.20;
    }
    (
        score,
        "test generation rewards property tests and clustered failure memory",
    )
}

fn robustness_score(repo: &AgentRepo) -> (f64, &'static str) {
    let mut score = 0.20;
    if repo
        .verification
        .contains(&VerificationStrategy::RegressionGate)
    {
        score += 0.25;
    }
    if repo
        .verification
        .contains(&VerificationStrategy::AdversarialCanaries)
    {
        score += 0.25;
    }
    if repo.editing == EditingPolicy::PullRequestWithRollbackNotes {
        score += 0.15;
    }
    (
        score,
        "robustness requires gates, canaries, and PR-shaped rollback notes",
    )
}

fn adversarial_score(repo: &AgentRepo) -> (f64, &'static str) {
    let mut score = 0.20;
    if repo
        .verification
        .contains(&VerificationStrategy::AdversarialCanaries)
    {
        score += 0.30;
    }
    if repo
        .verification
        .contains(&VerificationStrategy::PrivateCanaryAwareness)
    {
        score += 0.20;
    }
    if repo.search.contains(&SearchStrategy::LineageArchive) {
        score += 0.15;
    }
    (
        score,
        "adversarial work rewards canaries without exposing final holdouts",
    )
}

fn cost_score(repo: &AgentRepo) -> f64 {
    let tools = u32::try_from(repo.tools.len()).expect("fixture tool count fits u32");
    let tests = u32::try_from(repo.tests.len()).expect("fixture test count fits u32");
    let meta = u32::try_from(repo.meta_optimizer.len()).expect("fixture meta count fits u32");
    (1.0 - 0.03 * f64::from(tools + tests + meta)).max(0.70)
}

fn safety_score(repo: &AgentRepo) -> f64 {
    if repo.has_forbidden_surface() {
        0.0
    } else if repo
        .verification
        .contains(&VerificationStrategy::AdversarialCanaries)
    {
        1.0
    } else {
        0.80
    }
}

fn novelty_score(repo: &AgentRepo) -> f64 {
    let features = repo.prompts.len()
        + repo.tools.len()
        + repo.search.len()
        + repo.verification.len()
        + repo.tests.len()
        + repo.self_debugging.len()
        + repo.meta_optimizer.len();
    let features = u32::try_from(features).expect("fixture feature count fits u32");
    (f64::from(features) / 14.0).min(1.0)
}

fn metaproductivity_score(repo: &AgentRepo) -> f64 {
    let mut score: f64 = 0.10;
    if repo.memory == MemoryPolicy::FailureAndLineageMemory {
        score += 0.15;
    }
    if repo.search.contains(&SearchStrategy::NoveltySampling) {
        score += 0.15;
    }
    if repo.search.contains(&SearchStrategy::LineageArchive) {
        score += 0.25;
    }
    if repo
        .meta_optimizer
        .contains(&MetaOptimizer::DescendantScoring)
    {
        score += 0.25;
    }
    if repo
        .meta_optimizer
        .contains(&MetaOptimizer::ParentSelectionAudit)
    {
        score += 0.10;
    }
    score.min(1.0)
}

fn hard_gate_failures(repo: &AgentRepo, outcomes: &[RepoTaskOutcome]) -> BTreeSet<HardGateFailure> {
    let mut failures = BTreeSet::new();
    if repo.has_forbidden_surface() {
        failures.insert(HardGateFailure::EvaluatorTampering);
    }
    if !repo
        .verification
        .contains(&VerificationStrategy::RegressionGate)
        && outcomes
            .iter()
            .any(|outcome| outcome.family == TaskFamily::Robustness)
    {
        failures.insert(HardGateFailure::Regression);
    }
    if !repo
        .verification
        .contains(&VerificationStrategy::AdversarialCanaries)
        && outcomes
            .iter()
            .any(|outcome| outcome.family == TaskFamily::Adversarial)
    {
        failures.insert(HardGateFailure::HiddenStateChannel);
    }
    failures
}

fn scalar(value: f64, context: &'static str) -> Result<ScalarEvidence, EvaluationError> {
    ScalarEvidence::new(value)
        .map_err(|error| EvaluationError::with_source(context.to_owned(), error))
}

struct OuterPromotionKernel {
    cases: Vec<SelfOptimizationCase>,
}

impl OuterPromotionKernel {
    fn immutable_surface_count() -> usize {
        IMMUTABLE_SURFACES.len()
    }

    fn hard_gate_count() -> usize {
        HARD_GATES.len()
    }

    fn promote_with_rollback(
        &self,
        graph: &RunGraphView<'_, SelfOptimizationProblem>,
        incumbent: CandidateId,
        descendants: &[CandidateId],
    ) -> Option<PromotionDecision> {
        let incumbent_repo = graph.artifact(incumbent)?;
        let incumbent_private = self.evaluate_partition(incumbent_repo, Split::PrivateHoldout);
        let incumbent_final = self.evaluate_partition(incumbent_repo, Split::FinalTest);
        descendants
            .iter()
            .copied()
            .filter_map(|candidate| {
                let repo = graph.artifact(candidate)?;
                let private_holdout = self.evaluate_partition(repo, Split::PrivateHoldout);
                let final_test = self.evaluate_partition(repo, Split::FinalTest);
                let regression = self.evaluate_partition(repo, Split::Regression);
                let adversarial = self.evaluate_partition(repo, Split::Adversarial);
                let passes = passes_promotion_gate(
                    repo,
                    &private_holdout,
                    &final_test,
                    &regression,
                    &adversarial,
                    &incumbent_private,
                    &incumbent_final,
                );
                passes.then_some(PromotionDecision {
                    promoted: candidate,
                    rollback: incumbent,
                    private_holdout,
                    final_test,
                    incumbent_private: incumbent_private.clone(),
                    incumbent_final: incumbent_final.clone(),
                })
            })
            .max_by(|left, right| {
                let left_score = left.private_holdout.score.score() + left.final_test.score.score();
                let right_score =
                    right.private_holdout.score.score() + right.final_test.score.score();
                left_score
                    .partial_cmp(&right_score)
                    .expect("promotion scores are finite")
            })
    }

    fn evaluate_partition(&self, repo: &AgentRepo, split: Split) -> AgentRepoEvidence {
        let cases = self
            .cases
            .iter()
            .filter(|case| case.split == split)
            .collect::<Vec<_>>();
        evaluate_repo(repo, &cases).expect("promotion kernel fixture has cases")
    }
}

fn passes_promotion_gate(
    repo: &AgentRepo,
    private_holdout: &AgentRepoEvidence,
    final_test: &AgentRepoEvidence,
    regression: &AgentRepoEvidence,
    adversarial: &AgentRepoEvidence,
    incumbent_private: &AgentRepoEvidence,
    incumbent_final: &AgentRepoEvidence,
) -> bool {
    !repo.has_forbidden_surface()
        && private_holdout.hard_gate_failures.is_empty()
        && final_test.hard_gate_failures.is_empty()
        && regression.hard_gate_failures.is_empty()
        && adversarial.hard_gate_failures.is_empty()
        && private_holdout.score.score() > incumbent_private.score.score()
        && final_test.score.score() > incumbent_final.score.score()
        && private_holdout.cost_score.score() >= 0.70
        && final_test.safety_score.score() >= 1.0
        && final_test.metaproductivity_score.score() >= 0.75
}

#[derive(Clone, Debug)]
struct PromotionDecision {
    promoted: CandidateId,
    rollback: CandidateId,
    private_holdout: AgentRepoEvidence,
    final_test: AgentRepoEvidence,
    incumbent_private: AgentRepoEvidence,
    incumbent_final: AgentRepoEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelfOptimizationCase {
    case_id: CaseId,
    id: &'static str,
    split: Split,
    family: TaskFamily,
    requires_hidden_generalization: bool,
    requires_metaproductivity: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Split {
    PublicTrain,
    Regression,
    Adversarial,
    PrivateHoldout,
    FinalTest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskFamily {
    BugFix,
    Performance,
    TestGeneration,
    Robustness,
    Adversarial,
}

fn self_optimization_cases() -> Vec<SelfOptimizationCase> {
    vec![
        case(
            0,
            "public-bugfix",
            Split::PublicTrain,
            TaskFamily::BugFix,
            false,
            false,
        ),
        case(
            1,
            "public-testgen",
            Split::PublicTrain,
            TaskFamily::TestGeneration,
            false,
            false,
        ),
        case(
            2,
            "regression-rollback",
            Split::Regression,
            TaskFamily::Robustness,
            false,
            false,
        ),
        case(
            3,
            "adversarial-prompt-injection",
            Split::Adversarial,
            TaskFamily::Adversarial,
            true,
            false,
        ),
        case(
            4,
            "private-performance",
            Split::PrivateHoldout,
            TaskFamily::Performance,
            true,
            true,
        ),
        case(
            5,
            "private-robustness",
            Split::PrivateHoldout,
            TaskFamily::Robustness,
            true,
            false,
        ),
        case(
            6,
            "final-bugfix",
            Split::FinalTest,
            TaskFamily::BugFix,
            true,
            false,
        ),
        case(
            7,
            "final-metaproductivity",
            Split::FinalTest,
            TaskFamily::Performance,
            true,
            true,
        ),
    ]
}

fn case(
    index: u64,
    id: &'static str,
    split: Split,
    family: TaskFamily,
    requires_hidden_generalization: bool,
    requires_metaproductivity: bool,
) -> SelfOptimizationCase {
    SelfOptimizationCase {
        case_id: CaseId::new(index),
        id,
        split,
        family,
        requires_hidden_generalization,
        requires_metaproductivity,
    }
}

fn self_optimization_case_set(cases: &[SelfOptimizationCase]) -> CaseSet<SelfOptimizationCase> {
    CaseSet::new(cases.to_vec())
        .with_partition(
            PartitionId::from(PUBLIC_TRAIN),
            ids_for(cases, Split::PublicTrain),
        )
        .with_partition(
            PartitionId::from(REGRESSION),
            ids_for(cases, Split::Regression),
        )
        .with_partition(
            PartitionId::from(ADVERSARIAL),
            ids_for(cases, Split::Adversarial),
        )
        .with_partition(
            PartitionId::from(PRIVATE_HOLDOUT),
            ids_for(cases, Split::PrivateHoldout),
        )
        .with_partition(
            PartitionId::from(FINAL_TEST),
            ids_for(cases, Split::FinalTest),
        )
}

fn ids_for(cases: &[SelfOptimizationCase], split: Split) -> Vec<CaseId> {
    cases
        .iter()
        .filter_map(|case| (case.split == split).then_some(case.case_id))
        .collect()
}

fn case_by_id(cases: &[SelfOptimizationCase], id: CaseId) -> Option<&SelfOptimizationCase> {
    usize::try_from(id.0)
        .ok()
        .and_then(|index| cases.get(index))
}
