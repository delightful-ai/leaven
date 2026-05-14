//! Proposer context.

use leaven_core::OptimizationProblem;
use std::sync::{Arc, Mutex};

use leaven_kernel::{
    BudgetSnapshot, StageAttemptOutcome, StageAttemptReceiptRef, StageCallId, StageId, StageRole,
};

use crate::{BudgetHandle, MaterializeContext, ReadScope, RenderContext, RunGraphView};

pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,
    read_scope: ReadScope,
    stage_call_id: StageCallId,
    stage_attempt_sink: StageAttemptEventSink,
}

impl<'a, P: OptimizationProblem> ProposalContext<'a, P> {
    pub(crate) fn new(
        graph: RunGraphView<'a, P>,
        budget: BudgetHandle<'a>,
        read_scope: ReadScope,
        stage_call_id: StageCallId,
        stage_attempt_sink: StageAttemptEventSink,
    ) -> Self {
        Self {
            graph,
            budget,
            read_scope,
            stage_call_id,
            stage_attempt_sink,
        }
    }

    #[must_use]
    pub fn graph(&self) -> &RunGraphView<'a, P> {
        &self.graph
    }

    #[must_use]
    pub fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub fn budget(&self) -> BudgetSnapshot {
        self.budget.snapshot()
    }

    pub fn budget_handle(&mut self) -> &mut BudgetHandle<'a> {
        &mut self.budget
    }

    #[must_use]
    pub const fn stage_call_id(&self) -> StageCallId {
        self.stage_call_id
    }

    pub fn record_stage_attempt(
        &self,
        role: StageRole,
        receipt: StageAttemptReceiptRef,
        outcome: StageAttemptOutcome,
    ) {
        self.stage_attempt_sink.push(PendingStageAttemptEvent {
            stage_call_id: self.stage_call_id,
            role,
            receipt,
            outcome,
        });
    }

    pub(crate) fn stage_attempt_sink(&self) -> StageAttemptEventSink {
        self.stage_attempt_sink.clone()
    }

    #[must_use]
    pub fn render_context(&mut self) -> RenderContext<'_, P> {
        RenderContext::new(
            self.graph.clone(),
            self.budget.sub_stage(StageId::custom("render")),
            self.read_scope.clone(),
        )
    }

    #[must_use]
    pub fn materialize_context(&self) -> MaterializeContext<'a, P> {
        MaterializeContext::new(
            self.graph.clone(),
            self.budget.snapshot(),
            self.read_scope.clone(),
        )
    }
}

#[derive(Clone, Default)]
pub struct StageAttemptEventSink {
    inner: Arc<Mutex<Vec<PendingStageAttemptEvent>>>,
}

impl StageAttemptEventSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, event: PendingStageAttemptEvent) {
        self.inner
            .lock()
            .expect("stage attempt sink poisoned")
            .push(event);
    }

    pub(crate) fn drain(&self) -> Vec<PendingStageAttemptEvent> {
        std::mem::take(&mut *self.inner.lock().expect("stage attempt sink poisoned"))
    }
}

#[derive(Clone, Debug)]
pub struct PendingStageAttemptEvent {
    pub stage_call_id: StageCallId,
    pub role: StageRole,
    pub receipt: StageAttemptReceiptRef,
    pub outcome: StageAttemptOutcome,
}
