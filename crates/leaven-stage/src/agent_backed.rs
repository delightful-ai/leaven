use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use leaven_workspace::WorkspaceConfig;

pub struct AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    pub workspace_factory: Arc<dyn Send + Sync>,
    pub runtime: Runtime,
    pub bootstrap: Bootstrap,
    pub parser: Parser,
    pub policy: AgentBackedPolicy,
    _marker: PhantomData<Slot>,
}

impl<Slot, Runtime, Bootstrap, Parser> AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    #[must_use]
    pub fn new(
        workspace_factory: Arc<dyn Send + Sync>,
        runtime: Runtime,
        bootstrap: Bootstrap,
        parser: Parser,
        policy: AgentBackedPolicy,
    ) -> Self {
        Self {
            workspace_factory,
            runtime,
            bootstrap,
            parser,
            policy,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentBackedPolicy {
    pub workspace: WorkspaceConfig,
    pub runtime_timeout: Option<Duration>,
    pub on_parse_failure: ParseFailurePolicy,
    pub receipt_sink: ReceiptSinkPolicy,
}

impl Default for AgentBackedPolicy {
    fn default() -> Self {
        Self {
            workspace: WorkspaceConfig::default(),
            runtime_timeout: None,
            on_parse_failure: ParseFailurePolicy::Strict,
            receipt_sink: ReceiptSinkPolicy::Inline,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailurePolicy {
    Strict,
    RecordAttempt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiptSinkPolicy {
    Inline,
    External { sink: String },
}
