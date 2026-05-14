use leaven_kernel::{BudgetSnapshot, MetadataBag, StageCallId, StageRole};
use serde::{Deserialize, Serialize};

use crate::{StageOutputContract, StageQueryPolicy};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageDirective {
    pub title: String,
    pub instructions: String,
    pub success_criteria: Vec<String>,
    pub cautions: Vec<String>,
}

impl StageDirective {
    #[must_use]
    pub fn new(title: impl Into<String>, instructions: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            instructions: instructions.into(),
            success_criteria: Vec::new(),
            cautions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStagePlan<Req> {
    pub role: StageRole,
    pub request: Req,
    pub directive: StageDirective,
    pub query: StageQueryPolicy,
    pub output: StageOutputContract,
    pub metadata: MetadataBag,
}

impl<Req> AgentStagePlan<Req> {
    #[must_use]
    pub fn new(
        role: StageRole,
        request: Req,
        directive: StageDirective,
        output: StageOutputContract,
    ) -> Self {
        Self {
            role,
            request,
            directive,
            query: StageQueryPolicy::minimal(),
            output,
            metadata: MetadataBag::new(),
        }
    }

    #[must_use]
    pub fn with_query_policy(mut self, query: StageQueryPolicy) -> Self {
        self.query = query;
        self
    }
}

#[derive(Clone, Debug)]
pub struct AgentStageCallContext {
    stage_call_id: StageCallId,
    read_scope: leaven_engine::ReadScope,
    budget: BudgetSnapshot,
}

impl AgentStageCallContext {
    #[must_use]
    pub fn new(
        stage_call_id: StageCallId,
        read_scope: leaven_engine::ReadScope,
        budget: BudgetSnapshot,
    ) -> Self {
        Self {
            stage_call_id,
            read_scope,
            budget,
        }
    }

    #[must_use]
    pub const fn stage_call_id(&self) -> StageCallId {
        self.stage_call_id
    }

    #[must_use]
    pub const fn read_scope(&self) -> &leaven_engine::ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub fn budget_snapshot(&self) -> BudgetSnapshot {
        self.budget.clone()
    }
}
