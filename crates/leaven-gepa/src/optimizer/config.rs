use std::{collections::BTreeSet, sync::Arc};

use leaven_core::PartitionId;
use leaven_surface::EditSurface;
use serde::{Deserialize, Serialize};

use crate::{
    CandidateSelector, Gate, GepaEventSummary, GepaReferenceState, GepaReport, GepaReportProfile,
    PartSelector, PopulationBestFallback, RoundRobinPart, StrictImprovement,
    report::GepaReportInput,
    validation::{EpochShuffled, FullValidation},
};

use super::Gepa;

const DEFAULT_MAX_ITERATIONS: usize = 500;
const DEFAULT_PERFECT_SCORE: f64 = 1.0;
const FAST_CERTIFIED_MINIBATCH_SIZE: usize = 1;
const FAST_CERTIFIED_PROPOSAL_COUNT: usize = 2;

/// Named GEPA strategy profiles.
///
/// Profiles are convenience presets over the existing public strategy seams.
/// They do not hide a second optimizer loop: reference GEPA still owns state in
/// this crate, and opt-in profiles must preserve their certification claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GepaProfile {
    /// Upstream-reference GEPA defaults: epoch minibatch 3, one proposal per
    /// selected parent, skip-perfect enabled, and full validation before
    /// reference admission.
    Reference,
    /// Upstream optimize-anything GEPA defaults: epoch minibatch 3, one
    /// proposal per selected parent, skip-perfect disabled, and full
    /// validation before reference admission.
    ///
    /// This is the algorithm profile used by optimize-anything examples such
    /// as AIME. It is still not `DSPy`-default parity: `DSPy` merge and trace
    /// defaults require their own explicit profile.
    OptimizeAnything,
    /// Faster certified profile: smaller train probes and two serial proposal
    /// attempts per selected parent, while still requiring full validation
    /// before a child enters the GEPA reference state.
    ///
    /// This is not the future async/lazy-certification `FastGEPA` design; it is
    /// the currently implemented safe speed preset.
    FastCertified,
}

impl GepaProfile {
    /// Stable profile label for reports and operator logs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::OptimizeAnything => "optimize-anything",
            Self::FastCertified => "fast-certified",
        }
    }

    const fn minibatch_size(self) -> usize {
        match self {
            Self::Reference | Self::OptimizeAnything => 3,
            Self::FastCertified => FAST_CERTIFIED_MINIBATCH_SIZE,
        }
    }

    const fn proposal_count(self) -> usize {
        match self {
            Self::Reference | Self::OptimizeAnything => 1,
            Self::FastCertified => FAST_CERTIFIED_PROPOSAL_COUNT,
        }
    }

    const fn skip_perfect_score(self) -> bool {
        match self {
            Self::Reference | Self::FastCertified => true,
            Self::OptimizeAnything => false,
        }
    }
}

#[derive(Clone)]
pub(super) struct GepaEventSink(pub(super) Arc<dyn Fn(&GepaEventSummary) + Send + Sync>);

impl std::fmt::Debug for GepaEventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GepaEventSink(..)")
    }
}

#[derive(Clone)]
pub(super) struct GepaReportSink(pub(super) Arc<dyn Fn(&GepaReport) + Send + Sync>);

impl std::fmt::Debug for GepaReportSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GepaReportSink(..)")
    }
}

impl<S, Pop, Reflect> Gepa<S, Pop, Reflect> {
    /// Build GEPA with deterministic default strategies.
    #[must_use]
    pub fn new(surface: S, population: Pop, reflector: Reflect) -> Self {
        Self::with_strategies(
            surface,
            population,
            reflector,
            PopulationBestFallback,
            RoundRobinPart::new(),
            StrictImprovement,
        )
    }
}

impl<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
{
    /// Build GEPA with explicit strategy values.
    #[must_use]
    pub fn with_strategies(
        surface: S,
        population: Pop,
        reflector: Reflect,
        candidate_selector: CandidateSel,
        part_selector: PartSel,
        gate: GatePol,
    ) -> Self
    where
        Batch: Default,
        Validate: Default,
        Dataset: Default,
    {
        Self {
            surface,
            population,
            reflector,
            candidate_selector,
            part_selector,
            gate,
            batch_sampler: Batch::default(),
            validation_policy: Validate::default(),
            dataset: Dataset::default(),
            profile: GepaReportProfile {
                label: GepaProfile::Reference.label().to_owned(),
                train_minibatch_size: Some(GepaProfile::Reference.minibatch_size()),
                proposal_count: GepaProfile::Reference.proposal_count(),
                proposal_mode: "serial".to_owned(),
                validation_policy: "full-validation".to_owned(),
                certification_mode: "full-validation-before-admission".to_owned(),
                skip_perfect_score: GepaProfile::Reference.skip_perfect_score(),
                perfect_score: DEFAULT_PERFECT_SCORE.to_string(),
            },
            train_partition: PartitionId::from("TRAIN"),
            max_iterations: DEFAULT_MAX_ITERATIONS,
            proposal_count: 1,
            skip_perfect_score: true,
            perfect_score: DEFAULT_PERFECT_SCORE,
            completed_iterations: 0,
            best: None,
            validation_best: None,
            observed: BTreeSet::new(),
            candidate_history: Vec::new(),
            proposal_attempts: Vec::new(),
            reference_state: GepaReferenceState::default(),
            rng: crate::validation::GepaRandom::default(),
            events: Vec::new(),
            event_sink: None,
            report_sink: None,
        }
    }

    /// Surface owned by this optimizer.
    #[must_use]
    pub const fn surface(&self) -> &S {
        &self.surface
    }

    /// Population state owned by this optimizer.
    #[must_use]
    pub const fn population(&self) -> &Pop {
        &self.population
    }

    /// Mutable population state owned by this optimizer.
    #[must_use]
    pub const fn population_mut(&mut self) -> &mut Pop {
        &mut self.population
    }

    /// Mutable gate policy.
    #[must_use]
    pub const fn gate_mut(&mut self) -> &mut GatePol {
        &mut self.gate
    }

    /// Set the train minibatch sampler used for parent and child screening.
    #[must_use]
    pub fn batch_sampler<NextBatch>(
        self,
        batch_sampler: NextBatch,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, NextBatch, Validate, Dataset> {
        Gepa {
            surface: self.surface,
            population: self.population,
            reflector: self.reflector,
            candidate_selector: self.candidate_selector,
            part_selector: self.part_selector,
            gate: self.gate,
            batch_sampler,
            validation_policy: self.validation_policy,
            dataset: self.dataset,
            profile: GepaReportProfile {
                label: "custom".to_owned(),
                train_minibatch_size: None,
                ..self.profile
            },
            train_partition: self.train_partition,
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            proposal_attempts: self.proposal_attempts,
            reference_state: self.reference_state,
            rng: self.rng,
            events: self.events,
            event_sink: self.event_sink,
            report_sink: self.report_sink,
        }
    }

    /// Set the validation policy used after accepted candidates.
    #[must_use]
    pub fn validation_policy<NextValidate>(
        self,
        validation_policy: NextValidate,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, NextValidate, Dataset> {
        Gepa {
            surface: self.surface,
            population: self.population,
            reflector: self.reflector,
            candidate_selector: self.candidate_selector,
            part_selector: self.part_selector,
            gate: self.gate,
            batch_sampler: self.batch_sampler,
            validation_policy,
            dataset: self.dataset,
            profile: validation_policy_profile::<NextValidate>(self.profile),
            train_partition: self.train_partition,
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            proposal_attempts: self.proposal_attempts,
            reference_state: self.reference_state,
            rng: self.rng,
            events: self.events,
            event_sink: self.event_sink,
            report_sink: self.report_sink,
        }
    }

    /// Swap the reflective-dataset builder used before each reflection step.
    ///
    /// The builder is the "what data does reflection see" seam. The default is
    /// [`GepaReflectiveDataset`](crate::reflection::GepaReflectiveDataset), a
    /// GEPA-parity per-case projection. A plain closure can be passed here via
    /// the closure blanket impl.
    #[must_use]
    pub fn reflective_dataset<NextDataset>(
        self,
        dataset: NextDataset,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, NextDataset> {
        Gepa {
            surface: self.surface,
            population: self.population,
            reflector: self.reflector,
            candidate_selector: self.candidate_selector,
            part_selector: self.part_selector,
            gate: self.gate,
            batch_sampler: self.batch_sampler,
            validation_policy: self.validation_policy,
            dataset,
            profile: self.profile,
            train_partition: self.train_partition,
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            proposal_attempts: self.proposal_attempts,
            reference_state: self.reference_state,
            rng: self.rng,
            events: self.events,
            event_sink: self.event_sink,
            report_sink: self.report_sink,
        }
    }

    /// Set maximum fixed-surface-edit iterations.
    #[must_use]
    pub const fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Apply a named GEPA strategy profile.
    ///
    /// Profiles are explicit presets, not hidden compatibility modes. Applying
    /// a profile replaces the train-batch sampler and accepted-child validation
    /// policy with the profile's certified settings.
    #[must_use]
    pub fn with_profile(
        self,
        profile: GepaProfile,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, EpochShuffled, FullValidation, Dataset>
    {
        self.batch_sampler(EpochShuffled::new(profile.minibatch_size()))
            .validation_policy(FullValidation)
            .proposal_count(profile.proposal_count())
            .skip_perfect_score(profile.skip_perfect_score())
            .with_resolved_profile(
                profile.label(),
                Some(profile.minibatch_size()),
                "full-validation",
                "full-validation-before-admission",
            )
    }

    /// Set how many serial proposal attempts to run for the selected candidate
    /// in each iteration.
    ///
    /// This is useful for proposal-throughput experiments, but it is not async
    /// island GEPA: attempts share the same selected parent and train
    /// minibatch, and they are processed through the reference admission path
    /// one at a time.
    #[must_use]
    pub fn proposal_count(mut self, proposal_count: usize) -> Self {
        let proposal_count = proposal_count.max(1);
        self.proposal_count = proposal_count;
        "custom".clone_into(&mut self.profile.label);
        self.profile.proposal_count = proposal_count;
        self
    }

    /// Enable or disable upstream GEPA's all-perfect parent-minibatch skip.
    #[must_use]
    pub fn skip_perfect_score(mut self, skip: bool) -> Self {
        self.skip_perfect_score = skip;
        "custom".clone_into(&mut self.profile.label);
        self.profile.skip_perfect_score = skip;
        self
    }

    /// Set the score threshold considered perfect by the skip-perfect policy.
    #[must_use]
    pub fn perfect_score(mut self, perfect_score: f64) -> Self {
        self.perfect_score = perfect_score;
        "custom".clone_into(&mut self.profile.label);
        self.profile.perfect_score = perfect_score.to_string();
        self
    }

    fn with_resolved_profile(
        mut self,
        label: &str,
        train_minibatch_size: Option<usize>,
        validation_policy: &str,
        certification_mode: &str,
    ) -> Self {
        label.clone_into(&mut self.profile.label);
        self.profile.train_minibatch_size = train_minibatch_size;
        validation_policy.clone_into(&mut self.profile.validation_policy);
        certification_mode.clone_into(&mut self.profile.certification_mode);
        self
    }

    /// Candidate observations tracked by GEPA's private state.
    #[must_use]
    pub fn candidate_history(&self) -> &[super::GepaCandidateHistoryEntry] {
        &self.candidate_history
    }

    /// GEPA reference state used for candidate indices and validation frontier reports.
    #[must_use]
    pub const fn reference_state(&self) -> &GepaReferenceState {
        &self.reference_state
    }

    /// Structured GEPA phase events emitted by this optimizer.
    #[must_use]
    pub fn events(&self) -> &[GepaEventSummary] {
        &self.events
    }

    /// Detailed GEPA report snapshot for accepted candidates and validation frontier state.
    #[must_use]
    pub fn report(&self) -> GepaReport {
        GepaReport::from_reference_state(&GepaReportInput {
            profile: &self.profile,
            reference_state: &self.reference_state,
            candidate_history: &self.candidate_history,
            proposal_attempts: &self.proposal_attempts,
            events: &self.events,
            best_candidate: self.best,
            validation_best_candidate: self.validation_best.as_ref().map(|best| best.candidate),
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
        })
    }

    /// Register a GEPA phase event observer.
    ///
    /// This observes optimizer-level GEPA phases without requiring callers to
    /// parse generic engine events. The sink is intentionally not checkpointed;
    /// resumed runs install the observer from the fresh builder configuration.
    #[must_use]
    pub fn on_event<F>(mut self, sink: F) -> Self
    where
        F: Fn(&GepaEventSummary) + Send + Sync + 'static,
    {
        self.event_sink = Some(GepaEventSink(Arc::new(sink)));
        self
    }

    /// Register a detailed GEPA report observer.
    ///
    /// The sink is called when GEPA reaches a terminal optimizer status. It is
    /// intentionally not checkpointed; resumed runs install observers from the
    /// fresh builder configuration.
    #[must_use]
    pub fn on_report<F>(mut self, sink: F) -> Self
    where
        F: Fn(&GepaReport) + Send + Sync + 'static,
    {
        self.report_sink = Some(GepaReportSink(Arc::new(sink)));
        self
    }

    pub(crate) fn record_event(&mut self, event: GepaEventSummary) {
        if let Some(sink) = &self.event_sink {
            (sink.0)(&event);
        }
        self.events.push(event);
    }

    pub(crate) fn emit_report(&self) {
        if let Some(sink) = &self.report_sink {
            (sink.0)(&self.report());
        }
    }

    /// Select the next candidate to mutate.
    pub fn select_candidate<P>(
        &mut self,
        graph: leaven_engine::RunGraphView<'_, P>,
    ) -> Option<leaven_kernel::CandidateId>
    where
        P: leaven_core::OptimizationProblem,
        CandidateSel: CandidateSelector<P, Pop, Selection = Option<leaven_kernel::CandidateId>>,
    {
        self.candidate_selector.select(&self.population, graph)
    }

    /// Select the next surface part to mutate.
    pub fn select_part<A>(
        &mut self,
        artifact: &A,
    ) -> Result<S::PartId, leaven_surface::SurfaceError>
    where
        A: leaven_core::Artifact,
        S: EditSurface<A>,
        PartSel: PartSelector<A, S>,
    {
        self.part_selector.select_part(artifact, &self.surface)
    }

    /// Lower a surface-native edit into an artifact-native change.
    pub fn change_part<A>(
        &self,
        artifact: &A,
        part: S::PartId,
        edit: S::Edit,
    ) -> Result<<A as leaven_core::Artifact>::Change, leaven_surface::SurfaceError>
    where
        A: leaven_core::Artifact,
        S: EditSurface<A>,
    {
        self.surface.change_part(artifact, part, edit)
    }

    /// Apply the configured gate to two scalar screening scores.
    pub fn decide(&mut self, parent_score: f64, candidate_score: f64) -> crate::GateDecision
    where
        GatePol: Gate,
    {
        self.gate.decide(parent_score, candidate_score)
    }
}

fn validation_policy_profile<Validate>(profile: GepaReportProfile) -> GepaReportProfile {
    if std::any::type_name::<Validate>() == std::any::type_name::<FullValidation>() {
        GepaReportProfile {
            validation_policy: "full-validation".to_owned(),
            certification_mode: "full-validation-before-admission".to_owned(),
            ..profile
        }
    } else {
        GepaReportProfile {
            label: "custom".to_owned(),
            validation_policy: std::any::type_name::<Validate>().to_owned(),
            certification_mode: "custom-validation-before-admission".to_owned(),
            ..profile
        }
    }
}
