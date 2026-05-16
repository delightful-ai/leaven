use super::{
    BatchSampler, CandidateId, CandidateSelector, EditSurface, EvaluationPurpose, EvaluationSet,
    Gate, Gepa, GepaAssessment, GepaCandidateIndex, GepaCaseEvidence, GepaEventSummary,
    GepaPopulation, GepaReflector, OptimizationProblem, OptimizerError, PartSelector,
    ReflectiveDatasetBuilder, RunContext, RunGraphView, ValidationPolicy,
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
        self.events
            .push(GepaEventSummary::OptimizationEnded { best });
        Some(leaven_engine::StepStatus::Done)
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
        self.events.push(GepaEventSummary::IterationStarted {
            iteration: self.completed_iterations + 1,
        });
        let evaluation_set = self.batch_sampler.sample_train(&self.train_partition);
        self.events.push(GepaEventSummary::TrainMinibatchSampled);
        let (parent_index, parent) = self.select_reference_parent(ctx.graph(), seed)?;
        let parent_screening = self
            .screen_parent(ctx, parent, evaluation_set.clone())
            .await?;

        if self.observed.insert(parent) {
            self.candidate_history
                .push(parent_screening.history_entry(parent));
            self.observe_train_candidate(ctx, parent, &parent_screening);
        }
        for _ in 0..self.proposal_count {
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
            .select_by_validation_frontier_frequency()
            .or_else(|| {
                let parent = self.select_candidate(graph).unwrap_or(seed);
                self.reference_state
                    .index_of(parent)
                    .map(|index| (index, parent))
            })
            .ok_or_else(|| {
                OptimizerError::Message("GEPA selected a parent outside reference state".to_owned())
            })?;
        self.events.push(GepaEventSummary::ParentSelected {
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
        self.events.push(GepaEventSummary::ParentEvaluated {
            metric_calls_delta: parent_screening.metric_calls_new,
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
        let Some(candidate) = self
            .propose_candidate(ctx, parent, &parent_screening.assessments)
            .await?
        else {
            return Ok(());
        };

        let screened = self
            .evaluate_casewise(ctx, candidate, evaluation_set, EvaluationPurpose::Search)
            .await?;
        self.reference_state
            .add_metric_calls(screened.metric_calls_new);
        self.events.push(GepaEventSummary::ChildEvaluated {
            metric_calls_delta: screened.metric_calls_new,
        });
        if self
            .gate
            .decide(parent_screening.average_score, screened.average_score)
            .is_accept()
        {
            self.accept_child(ctx, candidate, parent_index, screened)
                .await?;
        } else {
            self.events.push(GepaEventSummary::ProposalRejected);
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
        self.events
            .push(GepaEventSummary::ProposalAccepted { child: candidate });
        self.candidate_history
            .push(screened.history_entry(candidate));
        self.observe_train_candidate(ctx, candidate, &screened);
        self.best = self.population.best();
        self.validate_candidate(ctx, candidate, vec![parent_index], false)
            .await?;
        if self.reference_state.index_of(candidate).is_none() {
            self.reference_state
                .add_unvalidated_candidate(candidate, vec![parent_index]);
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
