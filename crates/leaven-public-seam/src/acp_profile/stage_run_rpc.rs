use serde_json::Value;

use super::{
    AcpProfileDocument, invalid_acp, jsonrpc_id, require_jsonrpc_2, require_jsonrpc_members,
    required_string,
};
use crate::{PublicSeamError, StageRunRequestDocument, StageRunResultDocument};

/// The one host->worker stage-dispatch method in the locked ACP profile.
const STAGE_RUN_METHOD: &str = "leaven/stage.run";

/// ACP JSON-RPC request envelope for the `leaven/stage.run` dispatch method.
///
/// Unlike the 25 worker->host effect callbacks, stage dispatch carries a
/// stage-run request as its params (not Plan IR). The envelope still gates the
/// method through the locked profile, so a non-`leaven/stage.run` method cannot
/// ride the stage-run params past the profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpStageRunRequestDocument {
    id: String,
    request: StageRunRequestDocument,
}

impl AcpStageRunRequestDocument {
    pub(crate) fn from_validated_params(
        profile: &AcpProfileDocument,
        value: &Value,
        request: StageRunRequestDocument,
    ) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP stage-run request must be an object"))?;
        require_jsonrpc_2(object)?;
        if object.contains_key("result") || object.contains_key("error") {
            return Err(invalid_acp(
                "ACP stage-run request must not carry response result or error fields",
            ));
        }
        require_jsonrpc_members(
            object,
            &["jsonrpc", "id", "method", "params"],
            "ACP stage-run request",
        )?;
        let id = jsonrpc_id(object.get("id"))?;
        let method = required_string(object.get("method"), "method")?;
        if method != STAGE_RUN_METHOD {
            return Err(invalid_acp(format!(
                "ACP stage-run request method must be `{STAGE_RUN_METHOD}`, not `{method}`"
            )));
        }
        if profile.method(method).is_none() {
            return Err(invalid_acp(
                "ACP stage-run method is not in the locked Leaven profile",
            ));
        }
        Ok(Self { id, request })
    }

    /// JSON-RPC request id, normalized to a string for request/response binding.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Validated stage-run request carried by this dispatch.
    pub const fn request(&self) -> &StageRunRequestDocument {
        &self.request
    }
}

/// ACP JSON-RPC response envelope for the `leaven/stage.run` dispatch method.
///
/// The result is a stage-run result (a typed stage output), not a Plan Result
/// extension envelope. The response id must bind the dispatched request id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpStageRunResponseDocument {
    id: String,
    result: StageRunResultDocument,
}

impl AcpStageRunResponseDocument {
    pub(crate) fn from_validated_result(
        request: &AcpStageRunRequestDocument,
        value: &Value,
        result: StageRunResultDocument,
    ) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP stage-run response must be an object"))?;
        require_jsonrpc_2(object)?;
        if object.contains_key("method") || object.contains_key("params") {
            return Err(invalid_acp(
                "ACP stage-run response must not carry request method or params fields",
            ));
        }
        require_jsonrpc_members(
            object,
            &["jsonrpc", "id", "result"],
            "ACP stage-run response",
        )?;
        let id = jsonrpc_id(object.get("id"))?;
        if id != request.id() {
            return Err(invalid_acp(
                "ACP stage-run response id must match the dispatched request id",
            ));
        }
        if object.contains_key("error") {
            return Err(invalid_acp(
                "ACP stage-run success response must carry result, not error",
            ));
        }
        Ok(Self { id, result })
    }

    /// JSON-RPC response id, normalized to a string for request/response binding.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Validated stage-run result returned by the worker.
    pub const fn result(&self) -> &StageRunResultDocument {
        &self.result
    }
}
