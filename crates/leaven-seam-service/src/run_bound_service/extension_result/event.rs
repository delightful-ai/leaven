use leaven_engine::ExternalEventPayload;
use serde::Serialize;
use serde_json::Value;

use super::common::{EmptyObject, prefixed_jcs_hash};
use crate::run_bound_service::RunBoundGraphEffectError;

pub(crate) struct EventEmitExtensionContext<'a> {
    pub(crate) plan_id: &'a str,
    pub(crate) name: &'a str,
    pub(crate) event_kind: &'a str,
    pub(crate) payload_schema: &'a str,
    pub(crate) payload: &'a ExternalEventPayload,
    pub(crate) visibility: &'a str,
    pub(crate) event_id: &'a str,
    pub(crate) base_revision: &'a str,
    pub(crate) final_revision: &'a str,
    pub(crate) capability_fingerprint: &'a str,
    pub(crate) policy_fingerprint: &'a str,
    pub(crate) started_at: &'a str,
    pub(crate) completed_at: &'a str,
    pub(crate) return_values: Option<&'a Value>,
}

pub(crate) fn event_emit_extension_result(
    context: EventEmitExtensionContext<'_>,
) -> Result<Value, RunBoundGraphEffectError> {
    let receipt_id = format!("wrec_{}", context.name);
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &EventEmitRequestPreimage {
            schema_version: "leaven.plan_write_request.v1",
            name: context.name,
            kind: "emit_run_event",
            write: EventEmitWriteProjection {
                kind: "emit_run_event",
                event_kind: context.event_kind,
                payload_schema: context.payload_schema,
                payload: context.payload,
                visibility: context.visibility,
            },
            deps: EmptyObject {},
            dependency_data_classes: &[],
            base_revision: context.base_revision,
        },
    )?;
    let primary = EventEmitPrimary {
        kind: "emit_run_event",
        event_id: context.event_id,
        receipt: &receipt_id,
        data_classes: &["public"],
        replayability: "fully_managed",
    };
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &EventEmitResultPreimage {
            schema_version: "leaven.plan_write_result.v1",
            name: context.name,
            value: &primary,
        },
    )?;
    let result = EventEmitExtensionResult {
        method: "leaven/event.emit",
        primary,
        receipts: vec![EventEmitReceipt {
            kind: "write",
            receipt: &receipt_id,
            op_var: context.name,
            started_at: context.started_at,
            completed_at: context.completed_at,
            write_kind: "emit_run_event",
            request_hash: &request_hash,
            result_hash: &result_hash,
            base_revision: context.base_revision,
            committed_revision: context.final_revision,
            status: "succeeded",
            event_id: context.event_id,
        }],
        redactions: &[],
        capability_fingerprint: context.capability_fingerprint,
        policy_fingerprint: context.policy_fingerprint,
        data_classes: &["public"],
        plan_id: context.plan_id,
        return_values: EventEmitReturnValues::from(context.return_values),
    };
    serde_json::to_value(result).map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))
}

#[derive(Serialize)]
struct EventEmitWriteProjection<'a> {
    kind: &'static str,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: &'a ExternalEventPayload,
    visibility: &'a str,
}

#[derive(Serialize)]
struct EventEmitRequestPreimage<'a> {
    schema_version: &'static str,
    name: &'a str,
    kind: &'static str,
    write: EventEmitWriteProjection<'a>,
    deps: EmptyObject,
    dependency_data_classes: &'static [&'static str],
    base_revision: &'a str,
}

#[derive(Serialize)]
struct EventEmitPrimary<'a> {
    kind: &'static str,
    event_id: &'a str,
    receipt: &'a str,
    data_classes: &'static [&'static str],
    replayability: &'static str,
}

#[derive(Serialize)]
struct EventEmitResultPreimage<'a> {
    schema_version: &'static str,
    name: &'a str,
    value: &'a EventEmitPrimary<'a>,
}

#[derive(Serialize)]
struct EventEmitReceipt<'a> {
    kind: &'static str,
    receipt: &'a str,
    op_var: &'a str,
    started_at: &'a str,
    completed_at: &'a str,
    write_kind: &'static str,
    request_hash: &'a str,
    result_hash: &'a str,
    base_revision: &'a str,
    committed_revision: &'a str,
    status: &'static str,
    event_id: &'a str,
}

#[derive(Serialize)]
struct EventEmitExtensionResult<'a> {
    method: &'static str,
    primary: EventEmitPrimary<'a>,
    receipts: Vec<EventEmitReceipt<'a>>,
    redactions: &'static [&'static str],
    capability_fingerprint: &'a str,
    policy_fingerprint: &'a str,
    data_classes: &'static [&'static str],
    plan_id: &'a str,
    #[serde(rename = "return")]
    return_values: EventEmitReturnValues<'a>,
}

enum EventEmitReturnValues<'a> {
    Empty,
    Values(&'a Value),
}

impl<'a> EventEmitReturnValues<'a> {
    fn from(values: Option<&'a Value>) -> Self {
        values.map_or(Self::Empty, Self::Values)
    }
}

impl Serialize for EventEmitReturnValues<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Empty => <[&str; 0]>::default().serialize(serializer),
            Self::Values(values) => values.serialize(serializer),
        }
    }
}
