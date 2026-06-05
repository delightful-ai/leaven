use serde_json::{Map, Value};

/// Parsed closed public-seam `PlanError`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanErrorDocument {
    code: PlanErrorCode,
    message: String,
    op: Option<String>,
    path: Option<String>,
    receipt: String,
    retryable: Option<bool>,
    details: Option<PlanErrorDetails>,
}

impl PlanErrorDocument {
    pub(crate) fn from_object(error: &Map<String, Value>) -> Result<Self, String> {
        for key in error.keys() {
            if !matches!(
                key.as_str(),
                "code" | "message" | "op" | "path" | "receipt" | "retryable" | "details"
            ) {
                return Err(format!("PlanError carries unknown field `{key}`"));
            }
        }
        let code = required_string(error.get("code"), "PlanError.code")?;
        let code = PlanErrorCode::parse(code)
            .ok_or_else(|| "PlanError code must be a closed public-seam error code".to_owned())?;
        let message = required_string(error.get("message"), "PlanError.message")?;
        if message.trim().is_empty() {
            return Err("PlanError message must be non-empty".to_owned());
        }
        let receipt = plan_error_receipt_id(error)?.to_owned();
        let op = optional_string(error.get("op"), "PlanError.op")?.map(str::to_owned);
        let path = optional_string(error.get("path"), "PlanError.path")?.map(str::to_owned);
        let retryable = optional_bool(error.get("retryable"), "PlanError.retryable")?;
        let details = PlanErrorDetails::parse(error.get("details"))?;
        Ok(Self {
            code,
            message: message.to_owned(),
            op,
            path,
            receipt,
            retryable,
            details,
        })
    }

    /// Closed public-seam error code.
    pub const fn code(&self) -> PlanErrorCode {
        self.code
    }

    /// Human-readable public error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Optional Plan IR operation name associated with the error.
    pub fn op(&self) -> Option<&str> {
        self.op.as_deref()
    }

    /// Optional JSON Pointer associated with the error.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Receipt id bound to this error.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Whether the failed operation may be retried.
    pub const fn retryable(&self) -> Option<bool> {
        self.retryable
    }

    /// Closed typed detail payload, when supplied.
    pub const fn details(&self) -> Option<&PlanErrorDetails> {
        self.details.as_ref()
    }
}

/// Closed public-seam `PlanError` codes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlanErrorCode {
    TokenInvalid,
    TokenExpired,
    TokenRevoked,
    CapabilityDenied,
    BudgetExceeded,
    QuotaExceeded,
    HiddenPartitionViolation,
    DataClassViolation,
    SchemaValidationFailed,
    StageRuntimeError,
    PreconditionFailed,
    RevisionStale,
    RateLimited,
    Cancelled,
    Timeout,
    ProviderPolicyDenied,
    ProviderError,
    WorkspacePolicyDenied,
    PathDenied,
    SandboxDenied,
    WatchClosed,
    InternalError,
    ExtensionError,
}

impl PlanErrorCode {
    pub(crate) fn parse(code: &str) -> Option<Self> {
        Some(match code {
            "token_invalid" => Self::TokenInvalid,
            "token_expired" => Self::TokenExpired,
            "token_revoked" => Self::TokenRevoked,
            "capability_denied" => Self::CapabilityDenied,
            "budget_exceeded" => Self::BudgetExceeded,
            "quota_exceeded" => Self::QuotaExceeded,
            "hidden_partition_violation" => Self::HiddenPartitionViolation,
            "data_class_violation" => Self::DataClassViolation,
            "schema_validation_failed" => Self::SchemaValidationFailed,
            "stage_runtime_error" => Self::StageRuntimeError,
            "precondition_failed" => Self::PreconditionFailed,
            "revision_stale" => Self::RevisionStale,
            "rate_limited" => Self::RateLimited,
            "cancelled" => Self::Cancelled,
            "timeout" => Self::Timeout,
            "provider_policy_denied" => Self::ProviderPolicyDenied,
            "provider_error" => Self::ProviderError,
            "workspace_policy_denied" => Self::WorkspacePolicyDenied,
            "path_denied" => Self::PathDenied,
            "sandbox_denied" => Self::SandboxDenied,
            "watch_closed" => Self::WatchClosed,
            "internal_error" => Self::InternalError,
            "extension_error" => Self::ExtensionError,
            _ => return None,
        })
    }

    /// Wire spelling for this closed error code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TokenInvalid => "token_invalid",
            Self::TokenExpired => "token_expired",
            Self::TokenRevoked => "token_revoked",
            Self::CapabilityDenied => "capability_denied",
            Self::BudgetExceeded => "budget_exceeded",
            Self::QuotaExceeded => "quota_exceeded",
            Self::HiddenPartitionViolation => "hidden_partition_violation",
            Self::DataClassViolation => "data_class_violation",
            Self::SchemaValidationFailed => "schema_validation_failed",
            Self::StageRuntimeError => "stage_runtime_error",
            Self::PreconditionFailed => "precondition_failed",
            Self::RevisionStale => "revision_stale",
            Self::RateLimited => "rate_limited",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::ProviderPolicyDenied => "provider_policy_denied",
            Self::ProviderError => "provider_error",
            Self::WorkspacePolicyDenied => "workspace_policy_denied",
            Self::PathDenied => "path_denied",
            Self::SandboxDenied => "sandbox_denied",
            Self::WatchClosed => "watch_closed",
            Self::InternalError => "internal_error",
            Self::ExtensionError => "extension_error",
        }
    }
}

/// Typed public detail payload for a closed `PlanError`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanErrorDetails {
    summary: Option<String>,
    reason: Option<String>,
    retry_after_ms: Option<u64>,
}

impl PlanErrorDetails {
    fn parse(value: Option<&Value>) -> Result<Option<Self>, String> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(summary)) => Ok(Some(Self::from_summary(summary)?)),
            Some(Value::Object(object)) => Self::from_object(object).map(Some),
            Some(_) => Err(
                "PlanError.details must be null, a string summary, or a typed details object"
                    .to_owned(),
            ),
        }
    }

    fn from_summary(summary: &str) -> Result<Self, String> {
        let summary = required_non_empty_detail(summary, "PlanError.details summary")?;
        Ok(Self {
            summary: Some(summary.to_owned()),
            reason: None,
            retry_after_ms: None,
        })
    }

    fn from_object(object: &Map<String, Value>) -> Result<Self, String> {
        for key in object.keys() {
            if !matches!(key.as_str(), "summary" | "reason" | "retry_after_ms") {
                return Err(format!("PlanError.details carries unknown field `{key}`"));
            }
        }
        let summary = optional_string(object.get("summary"), "PlanError.details.summary")?
            .map(|summary| required_non_empty_detail(summary, "PlanError.details.summary"))
            .transpose()?
            .map(str::to_owned);
        let reason = optional_string(object.get("reason"), "PlanError.details.reason")?
            .map(|reason| required_non_empty_detail(reason, "PlanError.details.reason"))
            .transpose()?
            .map(str::to_owned);
        let retry_after_ms = optional_u64(
            object.get("retry_after_ms"),
            "PlanError.details.retry_after_ms",
        )?;
        if summary.is_none() && reason.is_none() && retry_after_ms.is_none() {
            return Err("PlanError.details object must carry a typed detail field".to_owned());
        }
        Ok(Self {
            summary,
            reason,
            retry_after_ms,
        })
    }

    /// Public summary for the failure.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Closed textual reason code or provider reason, when available.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Retry delay in milliseconds, when supplied by the producer.
    pub const fn retry_after_ms(&self) -> Option<u64> {
        self.retry_after_ms
    }
}

pub fn validate_closed_plan_error(error: &Map<String, Value>) -> Result<(), String> {
    PlanErrorDocument::from_object(error)?;
    Ok(())
}

pub fn closed_plan_error(error: &Value, field: &str) -> Result<PlanErrorDocument, String> {
    let error = error
        .as_object()
        .ok_or_else(|| format!("{field} must be a PlanError object"))?;
    PlanErrorDocument::from_object(error)
}

pub fn closed_plan_errors(errors: &[Value], field: &str) -> Result<Vec<PlanErrorDocument>, String> {
    errors
        .iter()
        .map(|error| closed_plan_error(error, field))
        .collect()
}

pub fn plan_error_receipt_id(error: &Map<String, Value>) -> Result<&str, String> {
    let receipt = error
        .get("receipt")
        .ok_or_else(|| "PlanError receipt must be present".to_owned())?;
    receipt_ref_id(receipt, "PlanError receipt")
}

pub fn receipt_ref_id<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    if let Some(receipt) = value.as_str() {
        require_receipt_id(receipt, field)?;
        return Ok(receipt);
    }
    let object = value
        .as_object()
        .ok_or_else(|| format!("{field} must be a ReceiptRef"))?;
    if object.get("kind").and_then(Value::as_str) != Some("receipt") {
        return Err(format!("{field} object must have kind `receipt`"));
    }
    let receipt = required_string(object.get("id"), &format!("{field} id"))?;
    require_receipt_id(receipt, field)?;
    Ok(receipt)
}

fn optional_string<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a str>, String> {
    value
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{field} must be a string"))
        })
        .transpose()
}

fn optional_bool(value: Option<&Value>, field: &str) -> Result<Option<bool>, String> {
    value
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| format!("{field} must be a boolean"))
        })
        .transpose()
}

fn optional_u64(value: Option<&Value>, field: &str) -> Result<Option<u64>, String> {
    value
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("{field} must be an unsigned integer"))
        })
        .transpose()
}

fn required_non_empty_detail<'a>(value: &'a str, field: &str) -> Result<&'a str, String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    Ok(value)
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, String> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a string"))
}

fn require_receipt_id(receipt: &str, field: &str) -> Result<(), String> {
    if is_locked_receipt_id(receipt) {
        return Ok(());
    }
    Err(format!(
        "{field} must match the locked public-seam ReceiptId grammar"
    ))
}

fn is_locked_receipt_id(receipt: &str) -> bool {
    let Some((prefix, suffix)) = receipt.split_once('_') else {
        return false;
    };
    matches!(
        prefix,
        "qrec"
            | "caseread"
            | "wsread"
            | "lmrec"
            | "agentrec"
            | "execrec"
            | "humanrec"
            | "wrec"
            | "chargerec"
            | "valrec"
            | "watchrec"
    ) && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}
