use leaven_engine::RunEvent;
use leaven_kernel::Cost;
use serde_json::{Map, Value, json};
use thiserror::Error;

/// Public-seam call receipt kind for runtime failures projected from engine cost events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicFailedCallKind {
    /// A failed `lm_complete` call.
    LmComplete,
    /// A failed `agent_run` call.
    AgentRun,
    /// A failed `sandbox_exec` call.
    SandboxExec,
}

impl PublicFailedCallKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::LmComplete => "lm_complete",
            Self::AgentRun => "agent_run",
            Self::SandboxExec => "sandbox_exec",
        }
    }

    fn receipt_prefix(self) -> &'static str {
        match self {
            Self::LmComplete => "lmrec",
            Self::AgentRun => "agentrec",
            Self::SandboxExec => "execrec",
        }
    }

    fn error_code(self) -> &'static str {
        match self {
            Self::LmComplete | Self::AgentRun => "provider_error",
            Self::SandboxExec => "stage_runtime_error",
        }
    }
}

/// Public-seam fields supplied while lowering a failed paid runtime call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicFailedCallReceiptContext {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    capability_fingerprint: String,
    policy_fingerprint: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    charged_at: Option<String>,
}

impl PublicFailedCallReceiptContext {
    /// Creates a public-seam failed-call receipt context.
    #[must_use]
    pub fn new(
        plan_id: impl Into<String>,
        base_revision: impl Into<String>,
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
    ) -> Self {
        let base_revision = base_revision.into();
        Self {
            plan_id: plan_id.into(),
            final_revision: base_revision.clone(),
            base_revision,
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            started_at: None,
            completed_at: None,
            charged_at: None,
        }
    }

    /// Overrides the final graph revision when a failed call changed durable state.
    #[must_use]
    pub fn with_final_revision(mut self, final_revision: impl Into<String>) -> Self {
        self.final_revision = final_revision.into();
        self
    }

    /// Adds audit timing for the failed call and matching charge receipt.
    #[must_use]
    pub fn with_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
        charged_at: impl Into<String>,
    ) -> Self {
        self.started_at = Some(started_at.into());
        self.completed_at = Some(completed_at.into());
        self.charged_at = Some(charged_at.into());
        self
    }

    /// Projects an engine `BudgetCharged` event into a failed call plus charge receipt.
    ///
    /// This is a lowering helper only: engine budget mutation must already have
    /// happened through `RunContext`, and `leaven-public-seam` remains the owner
    /// that validates the returned wire document.
    pub fn failed_paid_call_plan_result(
        &self,
        charge_event: &RunEvent,
        failure_event: &RunEvent,
        kind: PublicFailedCallKind,
        op_var: impl AsRef<str>,
        request: &Value,
        runtime_fingerprint: impl AsRef<str>,
    ) -> Result<Value, PublicFailedCallReceiptProjectionError> {
        let RunEvent::BudgetCharged { stage, cost, .. } = charge_event else {
            return Err(PublicFailedCallReceiptProjectionError::NotBudgetChargeEvent);
        };
        let RunEvent::Error {
            stage: Some(error_stage),
            error: engine_error,
            ..
        } = failure_event
        else {
            return Err(PublicFailedCallReceiptProjectionError::NotFailureEvent);
        };
        if error_stage != stage {
            return Err(PublicFailedCallReceiptProjectionError::StageMismatch);
        }
        let started_at = self
            .started_at
            .as_deref()
            .ok_or(PublicFailedCallReceiptProjectionError::MissingTiming)?;
        let completed_at = self
            .completed_at
            .as_deref()
            .ok_or(PublicFailedCallReceiptProjectionError::MissingTiming)?;
        let charged_at = self
            .charged_at
            .as_deref()
            .ok_or(PublicFailedCallReceiptProjectionError::MissingTiming)?;
        let cost = public_cost(cost)?;
        let op_var = validated_receipt_suffix(op_var.as_ref())?;
        let runtime_fingerprint = runtime_fingerprint.as_ref();
        if runtime_fingerprint.trim().is_empty() {
            return Err(PublicFailedCallReceiptProjectionError::InvalidRuntimeFingerprint);
        }
        let receipt_id = format!("{}_{}", kind.receipt_prefix(), op_var);
        let charge_id = format!("chargerec_{op_var}");
        let error = json!({
            "code": kind.error_code(),
            "message": engine_error.message,
            "op": op_var,
            "receipt": receipt_id,
            "retryable": true,
            "details": {
                "summary": source_chain_summary(&engine_error.source_chain),
                "reason": format!("{:?}", engine_error.kind)
            }
        });
        let charge_receipts = vec![charge_id.clone()];
        let request_hash = prefixed_jcs_hash_for_failed_call("fp_request_sha256_", request)?;
        let result_hash = prefixed_jcs_hash_for_failed_call(
            "fp_result_sha256_",
            &json!({
                "schema_version": "leaven.plan_call_result.v1",
                "name": op_var,
                "error": error,
                "cost": cost,
                "charge_receipts": charge_receipts
            }),
        )?;
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": self.plan_id,
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "final_revision": self.final_revision,
            "replayability_summary": "has_declared_external_effects",
            "values": {},
            "receipts": [
                {
                    "kind": "call",
                    "receipt": receipt_id,
                    "op_var": op_var,
                    "started_at": started_at,
                    "completed_at": completed_at,
                    "call_kind": kind.as_str(),
                    "request_hash": request_hash,
                    "result_hash": result_hash,
                    "runtime_fingerprint": runtime_fingerprint,
                    "status": "failed",
                    "error": error,
                    "cost": cost,
                    "charge_receipts": charge_receipts
                }
            ],
            "redactions": [],
            "charges": [
                {
                    "receipt": charge_id,
                    "source_receipt": receipt_id,
                    "cost": cost,
                    "ledger_scope": format!("engine:{stage}"),
                    "charged_at": charged_at
                }
            ],
            "errors": [error]
        }))
    }
}

/// Errors raised while projecting engine cost events into V1 failed-call receipts.
#[derive(Debug, Error)]
pub enum PublicFailedCallReceiptProjectionError {
    /// The context did not include receipt timing.
    #[error("failed paid call projection requires receipt timing")]
    MissingTiming,
    /// The supplied event was not an engine budget charge.
    #[error("failed paid call projection requires a RunEvent::BudgetCharged event")]
    NotBudgetChargeEvent,
    /// The supplied event was not an engine failure event.
    #[error("failed paid call projection requires a RunEvent::Error event")]
    NotFailureEvent,
    /// The budget charge and failure event came from different stages.
    #[error("failed paid call budget charge and failure stage must match")]
    StageMismatch,
    /// The engine charge carried no public-seam-representable cost.
    #[error("failed paid call projection requires non-zero public-seam cost")]
    EmptyCost,
    /// The engine charge used a cost axis that the locked V1 cost schema cannot represent.
    #[error("engine cost axis `{axis}` is not representable in public-seam V1 cost")]
    UnsupportedCostAxis {
        /// Unsupported cost axis.
        axis: String,
    },
    /// The operation variable cannot be used as a receipt suffix.
    #[error("failed paid call operation name is not a valid receipt suffix")]
    InvalidReceiptSuffix,
    /// The runtime fingerprint was empty.
    #[error("failed paid call runtime fingerprint must be non-empty")]
    InvalidRuntimeFingerprint,
    /// JCS/SHA-256 fingerprint computation failed.
    #[error("failed paid call fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable fingerprinting error.
        message: String,
    },
}

fn prefixed_jcs_hash_for_failed_call(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicFailedCallReceiptProjectionError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        PublicFailedCallReceiptProjectionError::Fingerprint {
            message: error.to_string(),
        }
    })?;
    Ok(format!("{prefix}{digest}"))
}

fn source_chain_summary(source_chain: &[String]) -> String {
    if source_chain.is_empty() {
        return "engine reported no public source chain".to_owned();
    }
    source_chain.join(": ")
}

fn public_cost(cost: &Cost) -> Result<Value, PublicFailedCallReceiptProjectionError> {
    if !cost.seconds.is_zero() {
        return Err(
            PublicFailedCallReceiptProjectionError::UnsupportedCostAxis {
                axis: "seconds".to_owned(),
            },
        );
    }
    if let Some(axis) = cost
        .other
        .iter()
        .find_map(|(axis, amount)| (!amount.is_zero()).then_some(axis.clone()))
    {
        return Err(PublicFailedCallReceiptProjectionError::UnsupportedCostAxis { axis });
    }
    let mut value = Map::new();
    insert_u64_cost(&mut value, "metric_calls", cost.metric_calls);
    insert_u64_cost(&mut value, "lm_calls", cost.llm_calls);
    insert_u64_cost(&mut value, "input_tokens", cost.prompt_tokens);
    insert_u64_cost(&mut value, "output_tokens", cost.completion_tokens);
    if value.is_empty() {
        return Err(PublicFailedCallReceiptProjectionError::EmptyCost);
    }
    Ok(Value::Object(value))
}

fn insert_u64_cost(value: &mut Map<String, Value>, key: &'static str, amount: u64) {
    if amount > 0 {
        value.insert(key.to_owned(), json!(amount));
    }
}

fn validated_receipt_suffix(value: &str) -> Result<&str, PublicFailedCallReceiptProjectionError> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        Ok(value)
    } else {
        Err(PublicFailedCallReceiptProjectionError::InvalidReceiptSuffix)
    }
}
