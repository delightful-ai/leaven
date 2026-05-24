use std::collections::VecDeque;

use serde_json::{Value, json};

use super::{AcpBackpressure, AcpProfileDocument, invalid_acp, required_string};
use crate::{
    PublicSeamError,
    plan_error::{is_closed_plan_error_code, receipt_ref_id},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpSessionLifecycle {
    max_inflight_updates: usize,
    backpressure: AcpBackpressure,
    next_sequence: u64,
    updates: VecDeque<AcpSessionUpdate>,
    cancellation: Option<AcpSessionCancellation>,
    state: AcpSessionState,
}

impl AcpSessionLifecycle {
    /// Builds a bounded ACP update lifecycle from the validated profile.
    pub fn from_profile(profile: &AcpProfileDocument) -> Result<Self, PublicSeamError> {
        let max_inflight_updates = usize::try_from(profile.default_max_inflight_updates())
            .map_err(|_| invalid_acp("ACP max inflight updates does not fit this platform"))?;
        Self::bounded(max_inflight_updates, profile.backpressure())
    }

    fn bounded(
        max_inflight_updates: usize,
        backpressure: AcpBackpressure,
    ) -> Result<Self, PublicSeamError> {
        if max_inflight_updates == 0 {
            return Err(invalid_acp("ACP update queue bound must be non-zero"));
        }
        Ok(Self {
            max_inflight_updates,
            backpressure,
            next_sequence: 0,
            updates: VecDeque::new(),
            cancellation: None,
            state: AcpSessionState::Running,
        })
    }

    /// Maximum in-flight progress updates allowed before backpressure is applied.
    pub const fn max_inflight_updates(&self) -> usize {
        self.max_inflight_updates
    }

    /// Backpressure strategy governing the bounded update queue.
    pub const fn backpressure(&self) -> AcpBackpressure {
        self.backpressure
    }

    /// Current number of queued progress updates.
    pub fn inflight_updates(&self) -> usize {
        self.updates.len()
    }

    /// Whether ACP session cancellation has been requested.
    pub const fn is_cancelled(&self) -> bool {
        matches!(self.state, AcpSessionState::Cancelled)
    }

    /// Current session lifecycle state.
    pub const fn state(&self) -> AcpSessionState {
        self.state
    }

    /// Cancellation facts, when the session has been cancelled.
    pub const fn cancellation(&self) -> Option<&AcpSessionCancellation> {
        self.cancellation.as_ref()
    }

    /// Enqueues one ACP progress update or returns bounded-queue backpressure.
    pub fn enqueue_progress(
        &mut self,
        message: impl Into<String>,
    ) -> Result<&AcpSessionUpdate, PublicSeamError> {
        match self.offer_progress(message, AcpProgressPriority::Critical)? {
            AcpProgressDisposition::Enqueued(_) => Ok(self
                .updates
                .back()
                .expect("enqueued critical update must be observable")),
            AcpProgressDisposition::DroppedNoncritical => Err(invalid_acp(
                "ACP critical progress update cannot be dropped as noncritical",
            )),
            AcpProgressDisposition::Disconnected(reason) => Err(invalid_acp(reason)),
        }
    }

    /// Offers one progress update with explicit priority.
    pub fn offer_progress(
        &mut self,
        message: impl Into<String>,
        priority: AcpProgressPriority,
    ) -> Result<AcpProgressDisposition, PublicSeamError> {
        if self.is_cancelled() {
            return Err(invalid_acp(
                "ACP session updates are refused after session cancellation",
            ));
        }
        if self.updates.len() >= self.max_inflight_updates {
            return match self.backpressure {
                AcpBackpressure::PauseWorker => Err(invalid_acp(
                    "ACP session update queue is full; worker must pause",
                )),
                AcpBackpressure::DropNoncriticalUpdates
                    if priority == AcpProgressPriority::Noncritical =>
                {
                    Ok(AcpProgressDisposition::DroppedNoncritical)
                }
                AcpBackpressure::DropNoncriticalUpdates => Err(invalid_acp(
                    "ACP session update queue is full; worker must pause critical updates",
                )),
                AcpBackpressure::Disconnect => {
                    let reason = "ACP session disconnected after update overflow";
                    let receipt = format!("valrec_acp_disconnect_{}", self.next_sequence);
                    let error = cancellation_plan_error(&receipt, "cancelled", reason);
                    let cancellation = self.cancel_with_error(reason, receipt, error)?;
                    Ok(AcpProgressDisposition::Disconnected(
                        cancellation.reason().to_owned(),
                    ))
                }
            };
        }
        let update = AcpSessionUpdate {
            sequence: self.next_sequence,
            message: message.into(),
        };
        self.next_sequence += 1;
        self.updates.push_back(update);
        Ok(AcpProgressDisposition::Enqueued(
            self.updates
                .back()
                .expect("pushed update must be observable")
                .clone(),
        ))
    }

    /// Acknowledges the oldest in-flight progress update.
    pub fn acknowledge_oldest_update(&mut self) -> Option<AcpSessionUpdate> {
        self.updates.pop_front()
    }

    /// Cancels the ACP session with an auditable receipt and closed `PlanError`.
    pub fn cancel_with_error(
        &mut self,
        reason: impl Into<String>,
        receipt: impl Into<String>,
        error: Value,
    ) -> Result<&AcpSessionCancellation, PublicSeamError> {
        if self.cancellation.is_none() {
            let receipt = receipt.into();
            validate_cancellation_error(&receipt, &error)?;
            self.cancellation = Some(AcpSessionCancellation {
                reason: reason.into(),
                receipt,
                error,
            });
            self.state = AcpSessionState::Cancelled;
        }
        Ok(self
            .cancellation
            .as_ref()
            .expect("cancellation set before return"))
    }
}

/// Priority of one ACP progress update under backpressure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcpProgressPriority {
    /// Critical updates must be delivered or force producer backpressure.
    Critical,
    /// Noncritical updates may be dropped when the profile allows it.
    Noncritical,
}

/// Result of offering one ACP progress update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcpProgressDisposition {
    /// The update entered the bounded queue.
    Enqueued(AcpSessionUpdate),
    /// The profile dropped a noncritical update at the queue boundary.
    DroppedNoncritical,
    /// The profile disconnected the session at the queue boundary.
    Disconnected(String),
}

/// Profile-level ACP session lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcpSessionState {
    /// The worker session accepts progress updates.
    Running,
    /// ACP cancellation has been requested for the session.
    Cancelled,
}

/// One ACP session progress update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpSessionUpdate {
    sequence: u64,
    message: String,
}

impl AcpSessionUpdate {
    /// Monotone sequence number within one session.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Human-readable progress update.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// ACP session cancellation facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpSessionCancellation {
    reason: String,
    receipt: String,
    error: Value,
}

impl AcpSessionCancellation {
    /// Cancellation reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Receipt that audits the ACP session cancellation.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Closed `PlanError` associated with the ACP session cancellation.
    pub const fn error(&self) -> &Value {
        &self.error
    }
}

fn cancellation_plan_error(receipt: &str, code: &str, message: &str) -> Value {
    json!({
        "code": code,
        "message": message,
        "receipt": receipt
    })
}

fn validate_cancellation_error(receipt: &str, error: &Value) -> Result<(), PublicSeamError> {
    if receipt.trim().is_empty() {
        return Err(invalid_acp("ACP cancellation receipt must be non-empty"));
    }
    let object = error
        .as_object()
        .ok_or_else(|| invalid_acp("ACP cancellation error must be a PlanError object"))?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "code" | "message" | "op" | "path" | "receipt" | "retryable" | "details"
        ) {
            return Err(invalid_acp(format!(
                "ACP cancellation error carries unknown PlanError field `{key}`"
            )));
        }
    }
    let code = required_string(object.get("code"), "cancellation error code")?;
    if !is_closed_plan_error_code(code) {
        return Err(invalid_acp(
            "ACP cancellation error code must be a closed PlanError code",
        ));
    }
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_acp("ACP cancellation error must carry message"))?;
    if message.trim().is_empty() {
        return Err(invalid_acp(
            "ACP cancellation error must carry non-empty message",
        ));
    }
    let error_receipt = object
        .get("receipt")
        .ok_or_else(|| invalid_acp("ACP cancellation error receipt must be present"))
        .and_then(|receipt| {
            receipt_ref_id(receipt, "ACP cancellation error receipt").map_err(invalid_acp)
        })?;
    if error_receipt != receipt {
        return Err(invalid_acp(
            "ACP cancellation error receipt must match cancellation receipt",
        ));
    }
    Ok(())
}
