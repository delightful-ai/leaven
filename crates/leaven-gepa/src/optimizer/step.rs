use super::{
    BatchSampler, CandidateId, CandidateSelector, EditSurface, EvaluationPurpose, EvaluationSet,
    Gate, Gepa, GepaAssessment, GepaCandidateIndex, GepaCaseEvidence, GepaEventSummary,
    GepaPopulation, GepaProposalAttempt, GepaReflector, OptimizationProblem, OptimizerError,
    PartSelector, ReflectiveDatasetBuilder, RunContext, RunGraphView, ValidationPolicy,
};

impl<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
{
    pub(super) fn finish_if_iteration_limit(&mut self) -> Option<leaven_engine::StepStatus>
    where
        Pop: GepaPopulation,
    {
        if self.completed_iterations < self.max_iterations {
            return None;
        }
        self.best = self
            .reference_state
            .best_candidate()
            .or_else(|| self.population.best());
        let best = self
            .best
            .and_then(|candidate| self.reference_state.index_of(candidate));
        self.record_event(GepaEventSummary::OptimizationEnded { best });
        self.emit_report();
        Some(leaven_engine::StepStatus::Done)
    }

    pub(super) fn finish_for_budget_stop(&mut self) -> leaven_engine::StepStatus
    where
        Pop: GepaPopulation,
    {
        self.best = self
            .validation_best
            .as_ref()
            .map(|best| best.candidate)
            .or_else(|| self.reference_state.best_candidate())
            .or_else(|| self.population.best());
        let best = self
            .best
            .and_then(|candidate| self.reference_state.index_of(candidate));
        self.record_event(GepaEventSummary::OptimizationEnded { best });
        self.emit_report();
        leaven_engine::StepStatus::Stopped(leaven_engine::StopReason::BudgetReached)
    }

    pub(super) fn finish_for_candidate_cap_stop(&mut self) -> leaven_engine::StepStatus
    where
        Pop: GepaPopulation,
    {
        self.best = self
            .validation_best
            .as_ref()
            .map(|best| best.candidate)
            .or_else(|| self.reference_state.best_candidate())
            .or_else(|| self.population.best());
        let best = self
            .best
            .and_then(|candidate| self.reference_state.index_of(candidate));
        self.record_event(GepaEventSummary::OptimizationEnded { best });
        self.emit_report();
        leaven_engine::StepStatus::Stopped(leaven_engine::StopReason::CandidateCapReached)
    }

    /// Whether the graph's total spawned candidate count (seed plus every
    /// loop-authored child, accepted or rejected) has reached the configured
    /// candidate-pool cap. The graph candidate count is the truthful counter:
    /// every authored child is a graph candidate, while a skipped proposal that
    /// never authors a child does not move it.
    pub(super) fn candidate_cap_reached<P>(&self, ctx: &RunContext<'_, P>) -> bool
    where
        P: OptimizationProblem,
    {
        self.max_candidates
            .is_some_and(|cap| ctx.graph().candidate_count() >= cap.get())
    }

    pub(super) fn finish_for_engine_stop(&mut self)
    where
        Pop: GepaPopulation,
    {
        self.best = self
            .validation_best
            .as_ref()
            .map(|best| best.candidate)
            .or_else(|| self.reference_state.best_candidate())
            .or_else(|| self.population.best());
        let best = self
            .best
            .and_then(|candidate| self.reference_state.index_of(candidate));
        self.record_event(GepaEventSummary::OptimizationEnded { best });
        self.emit_report();
    }

    pub(super) fn seed_candidate<P>(ctx: &RunContext<'_, P>) -> Result<CandidateId, OptimizerError>
    where
        P: OptimizationProblem,
    {
        ctx.graph()
            .candidate_tree()
            .roots()
            .first()
            .copied()
            .ok_or_else(|| {
                OptimizerError::Message("GEPA requires at least one seed candidate".to_owned())
            })
    }

    pub(super) async fn run_iteration<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        seed: CandidateId,
    ) -> Result<(), OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        P::ProposalAnnotations: Default,
        S: EditSurface<P::Artifact> + Sync,
        S::PartId: std::fmt::Debug,
        Pop: GepaPopulation + Sync,
        Reflect: GepaReflector<P, S> + Sync,
        CandidateSel: CandidateSelector<P, Pop, Selection = Option<CandidateId>> + Sync,
        PartSel: PartSelector<P::Artifact, S> + Sync,
        GatePol: Gate + Sync,
        Batch: BatchSampler + Sync,
        Validate: ValidationPolicy + Sync,
        Dataset: ReflectiveDatasetBuilder<P, S> + Sync,
    {
        self.record_event(GepaEventSummary::IterationStarted {
            iteration: self.completed_iterations + 1,
        });
        let (parent_index, parent) = self.select_reference_parent(ctx.graph(), seed)?;
        let train_set = EvaluationSet::Partition(self.train_partition.clone());
        let train_cases = ctx
            .resolve_optimizer_evaluation_set(&train_set)
            .map_err(|error| {
                OptimizerError::with_source("GEPA could not resolve train minibatch cases", error)
            })?
            .case_ids;
        let evaluation_set = self
            .batch_sampler
            .sample_train_with_gepa_rng(&self.train_partition, &train_cases, &mut self.rng)
            .map_err(|error| {
                OptimizerError::with_source("GEPA could not sample train minibatch", error)
            })?;
        let sampled_cases = ctx
            .resolve_optimizer_evaluation_set(&evaluation_set)
            .map_err(|error| {
                OptimizerError::with_source("GEPA could not resolve train minibatch", error)
            })?
            .case_ids;
        self.record_event(GepaEventSummary::TrainMinibatchSampled {
            cases: sampled_cases,
        });
        let parent_screening = self
            .screen_parent(ctx, parent, evaluation_set.clone())
            .await?;

        if self.observed.insert(parent) {
            self.candidate_history
                .push(parent_screening.history_entry(parent));
            self.observe_train_candidate(ctx, parent, &parent_screening);
        }
        for _ in 0..self.proposal_count {
            // Honor the candidate-pool cap inside the iteration: once an
            // authored child reaches the cap, do not author further proposals.
            if self.candidate_cap_reached(ctx) {
                break;
            }
            self.process_proposal(
                ctx,
                parent,
                parent_index,
                &parent_screening,
                evaluation_set.clone(),
            )
            .await?;
        }
        Ok(())
    }

    fn select_reference_parent<P>(
        &mut self,
        graph: RunGraphView<'_, P>,
        seed: CandidateId,
    ) -> Result<(GepaCandidateIndex, CandidateId), OptimizerError>
    where
        P: OptimizationProblem,
        CandidateSel: CandidateSelector<P, Pop, Selection = Option<CandidateId>>,
    {
        let selected = self
            .reference_state
            .select_by_validation_frontier_frequency_with_rng(&mut self.rng)
            .or_else(|| {
                let parent = self.select_candidate(graph).unwrap_or(seed);
                self.reference_state
                    .index_of(parent)
                    .map(|index| (index, parent))
            })
            .ok_or_else(|| {
                OptimizerError::Message("GEPA selected a parent outside reference state".to_owned())
            })?;
        self.record_event(GepaEventSummary::ParentSelected {
            candidate_index: selected.0,
        });
        Ok(selected)
    }

    async fn screen_parent<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        evaluation_set: EvaluationSet,
    ) -> Result<GepaAssessment, OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        S: Sync,
        Pop: Sync,
        Reflect: Sync,
        CandidateSel: Sync,
        PartSel: Sync,
        GatePol: Sync,
        Batch: Sync,
        Validate: Sync,
        Dataset: Sync,
    {
        let parent_screening = self
            .evaluate_casewise(ctx, parent, evaluation_set, EvaluationPurpose::SeedBaseline)
            .await?;
        self.reference_state
            .add_metric_calls(parent_screening.metric_calls_new);
        self.record_event(GepaEventSummary::ParentEvaluated {
            metric_calls_delta: parent_screening.metric_calls_new,
            score: parent_screening.average_score.to_string(),
        });
        Ok(parent_screening)
    }

    async fn process_proposal<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_index: GepaCandidateIndex,
        parent_screening: &GepaAssessment,
        evaluation_set: EvaluationSet,
    ) -> Result<(), OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        P::ProposalAnnotations: Default,
        S: EditSurface<P::Artifact> + Sync,
        S::PartId: std::fmt::Debug,
        Pop: GepaPopulation + Sync,
        Reflect: GepaReflector<P, S> + Sync,
        CandidateSel: Sync,
        PartSel: PartSelector<P::Artifact, S> + Sync,
        GatePol: Gate + Sync,
        Batch: Sync,
        Validate: ValidationPolicy + Sync,
        Dataset: ReflectiveDatasetBuilder<P, S> + Sync,
    {
        let attempt_index = self.proposal_attempts.len() + 1;
        let outcome = self
            .propose_candidate(ctx, parent, parent_screening, attempt_index)
            .await?;
        let Some(candidate) = outcome.candidate else {
            self.proposal_attempts.push(GepaProposalAttempt {
                attempt_index,
                iteration: self.completed_iterations + 1,
                parent_index,
                parent,
                parent_assessments: parent_screening.assessments.clone(),
                parent_cases: parent_screening.cases(),
                parent_score: parent_screening.average_score,
                part_label: outcome.part_label,
                reflective_example_count: outcome.reflective_example_count,
                child: None,
                child_assessments: Vec::new(),
                child_cases: Vec::new(),
                child_score: None,
                accepted: None,
                admitted_index: None,
                skip_reason: outcome.skip_reason,
            });
            return Ok(());
        };

        let screened = self
            .evaluate_casewise(ctx, candidate, evaluation_set, EvaluationPurpose::Search)
            .await?;
        self.reference_state
            .add_metric_calls(screened.metric_calls_new);
        self.record_event(GepaEventSummary::ChildEvaluated {
            metric_calls_delta: screened.metric_calls_new,
            score: screened.average_score.to_string(),
        });
        let accepted = self
            .gate
            .decide(parent_screening.average_score, screened.average_score)
            .is_accept();
        self.proposal_attempts.push(GepaProposalAttempt {
            attempt_index,
            iteration: self.completed_iterations + 1,
            parent_index,
            parent,
            parent_assessments: parent_screening.assessments.clone(),
            parent_cases: parent_screening.cases(),
            parent_score: parent_screening.average_score,
            part_label: outcome.part_label,
            reflective_example_count: outcome.reflective_example_count,
            child: Some(candidate),
            child_assessments: screened.assessments.clone(),
            child_cases: screened.cases(),
            child_score: Some(screened.average_score),
            accepted: Some(accepted),
            admitted_index: None,
            skip_reason: None,
        });
        if accepted {
            self.accept_child(ctx, candidate, parent_index, screened)
                .await?;
        } else {
            self.record_event(GepaEventSummary::ProposalRejected);
        }
        Ok(())
    }

    async fn accept_child<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        parent_index: GepaCandidateIndex,
        screened: GepaAssessment,
    ) -> Result<(), OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        S: Sync,
        Pop: GepaPopulation + Sync,
        Reflect: Sync,
        CandidateSel: Sync,
        PartSel: Sync,
        GatePol: Sync,
        Batch: Sync,
        Validate: ValidationPolicy + Sync,
        Dataset: Sync,
    {
        self.record_event(GepaEventSummary::ProposalAccepted { child: candidate });
        self.candidate_history
            .push(screened.history_entry(candidate));
        self.observe_train_candidate(ctx, candidate, &screened);
        self.best = self.population.best();
        let admitted_index = self
            .validate_candidate(ctx, candidate, vec![parent_index], false)
            .await?;
        // Only full validation admits a child into GEPA reference state; a
        // train-accepted child can still remain train-population-only.
        if let Some(admitted_index) = admitted_index {
            if let Some(attempt) = self.proposal_attempts.last_mut() {
                if attempt.child == Some(candidate) {
                    attempt.admitted_index = Some(admitted_index);
                }
            }
        }
        Ok(())
    }

    fn observe_train_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        assessment: &GepaAssessment,
    ) where
        P: OptimizationProblem,
        Pop: GepaPopulation,
    {
        let events = self.population.observe_gepa(
            Some(&self.train_partition),
            candidate,
            &assessment.assessments,
            &assessment.scalar_evidence,
        );
        ctx.emit(leaven_engine::RunEvent::PopulationUpdated {
            population_id: self.population.id(),
            events,
        });
    }
}
