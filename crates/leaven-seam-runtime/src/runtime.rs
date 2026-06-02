use std::path::Path;

use leaven_public_seam::{
    AcpExtensionMethod, AcpJsonRpcRequestDocument, AcpProfileDocument, AcpStageRunRequestDocument,
    PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

const STAGE_RUN_METHOD: &str = "leaven/stage.run";

/// JSON-RPC error codes emitted by the public-seam runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonRpcErrorCode {
    /// Parse error.
    ParseError,
    /// Invalid request envelope or invalid public-seam payload.
    InvalidRequest,
    /// Method is not in the locked Leaven worker profile.
    MethodNotFound,
    /// The request is valid but no runtime owner is wired for the method.
    MethodUnavailable,
    /// The runtime owner returned a malformed public-seam result.
    InvalidResult,
}

impl JsonRpcErrorCode {
    const fn code(self) -> i64 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::MethodUnavailable => -32004,
            Self::InvalidResult => -32005,
        }
    }
}

/// A JSON-RPC response produced by the public-seam runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonRpcResponse {
    value: Value,
}

impl JsonRpcResponse {
    /// Builds a JSON-RPC error response.
    pub fn error(id: &Value, code: JsonRpcErrorCode, message: impl Into<String>) -> Self {
        error_response(id, code, message)
    }

    /// Raw response value ready for transport serialization.
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Consumes this response into the raw JSON value.
    pub fn into_value(self) -> Value {
        self.value
    }

    /// Returns true when the response is a JSON-RPC error.
    pub fn is_error(&self) -> bool {
        self.value.get("error").is_some()
    }
}

/// Transport-neutral runtime for one loaded Leaven public seam profile.
pub struct SeamRuntime<S> {
    package: PublicSeamPackage,
    profile: AcpProfileDocument,
    service: S,
}

impl<S> SeamRuntime<S> {
    /// Loads the locked public-seam package from a repository root.
    pub fn from_repo(root: impl AsRef<Path>, service: S) -> Result<Self, SeamRuntimeError> {
        let package = PublicSeamPackage::active_from_repo(root)?;
        Self::from_package(package, service)
    }

    /// Builds a runtime from an already loaded public-seam package.
    pub fn from_package(package: PublicSeamPackage, service: S) -> Result<Self, SeamRuntimeError> {
        let profile = package.locked_acp_profile_document()?;
        Ok(Self {
            package,
            profile,
            service,
        })
    }

    /// Locked worker-profile methods this runtime exposes.
    pub fn methods(&self) -> impl Iterator<Item = &str> {
        self.profile
            .extension_methods()
            .iter()
            .map(AcpExtensionMethod::method)
    }
}

impl<S: SeamService> SeamRuntime<S> {
    /// Handles one parsed JSON-RPC value and returns one response value.
    pub fn handle_value(&self, value: &Value) -> JsonRpcResponse {
        let id = response_id(value);
        match self.validate_request(value) {
            Ok(ValidatedRequest::Plan(request)) => self.handle_plan_request(&id, value, request),
            Ok(ValidatedRequest::StageRun(request)) => {
                self.handle_stage_run_request(&id, value, request)
            }
            Err(error) => error_response(&id, error.code, error.to_string()),
        }
    }

    fn validate_request(&self, value: &Value) -> Result<ValidatedRequest, RequestError> {
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| RequestError::invalid("JSON-RPC request must carry method"))?;
        if self.profile.method(method).is_none() {
            return Err(RequestError {
                code: JsonRpcErrorCode::MethodNotFound,
                message: format!("method `{method}` is not in the locked Leaven worker profile"),
            });
        }
        if method == STAGE_RUN_METHOD {
            let request = self
                .package
                .validate_acp_stage_run_request_document(&self.profile, value)
                .map_err(|error| RequestError::from_public_seam(&error))?;
            Ok(ValidatedRequest::StageRun(request))
        } else {
            let request = self
                .package
                .validate_acp_jsonrpc_request_document(&self.profile, value)
                .map_err(|error| RequestError::from_public_seam(&error))?;
            Ok(ValidatedRequest::Plan(request))
        }
    }

    fn handle_plan_request(
        &self,
        id: &Value,
        value: &Value,
        request: AcpJsonRpcRequestDocument,
    ) -> JsonRpcResponse {
        let Some(params) = value.get("params") else {
            return error_response(
                id,
                JsonRpcErrorCode::InvalidRequest,
                "request missing params",
            );
        };
        let validated_request = request.clone();
        let service_request = SeamPlanRequest {
            document: request,
            params,
        };
        match self.service.handle_plan(service_request) {
            Ok(result) => {
                let response = json!({"jsonrpc": "2.0", "id": id, "result": result});
                match self
                    .package
                    .validate_acp_jsonrpc_response_document(&validated_request, &response)
                {
                    Ok(_) => JsonRpcResponse { value: response },
                    Err(error) => error_response(
                        &response_id(value),
                        JsonRpcErrorCode::InvalidResult,
                        error.to_string(),
                    ),
                }
            }
            Err(error) => {
                error_response(id, JsonRpcErrorCode::MethodUnavailable, error.to_string())
            }
        }
    }

    fn handle_stage_run_request(
        &self,
        id: &Value,
        value: &Value,
        request: AcpStageRunRequestDocument,
    ) -> JsonRpcResponse {
        let Some(params) = value.get("params") else {
            return error_response(
                id,
                JsonRpcErrorCode::InvalidRequest,
                "request missing params",
            );
        };
        let validated_request = request.clone();
        let service_request = SeamStageRunRequest {
            document: request,
            params,
        };
        match self.service.handle_stage_run(service_request) {
            Ok(result) => {
                let response = json!({"jsonrpc": "2.0", "id": id, "result": result});
                match self
                    .package
                    .validate_acp_stage_run_response_document(&validated_request, &response)
                {
                    Ok(_) => JsonRpcResponse { value: response },
                    Err(error) => error_response(
                        &response_id(value),
                        JsonRpcErrorCode::InvalidResult,
                        error.to_string(),
                    ),
                }
            }
            Err(error) => {
                error_response(id, JsonRpcErrorCode::MethodUnavailable, error.to_string())
            }
        }
    }
}

enum ValidatedRequest {
    Plan(AcpJsonRpcRequestDocument),
    StageRun(AcpStageRunRequestDocument),
}

/// A validated Plan IR method request delivered to a [`SeamService`].
#[derive(Clone, Debug)]
pub struct SeamPlanRequest<'a> {
    document: AcpJsonRpcRequestDocument,
    params: &'a Value,
}

impl<'a> SeamPlanRequest<'a> {
    /// Validated JSON-RPC request document.
    pub const fn document(&self) -> &AcpJsonRpcRequestDocument {
        &self.document
    }

    /// Leaven method name.
    pub fn method(&self) -> &str {
        self.document.method()
    }

    /// Validated Plan IR params.
    pub const fn params(&self) -> &'a Value {
        self.params
    }
}

/// A validated `leaven/stage.run` request delivered to a [`SeamService`].
#[derive(Clone, Debug)]
pub struct SeamStageRunRequest<'a> {
    document: AcpStageRunRequestDocument,
    params: &'a Value,
}

impl<'a> SeamStageRunRequest<'a> {
    /// Validated stage-run request document.
    pub const fn document(&self) -> &AcpStageRunRequestDocument {
        &self.document
    }

    /// Validated stage-run params.
    pub const fn params(&self) -> &'a Value {
        self.params
    }
}

/// Method family delivered to a [`SeamService`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeamRequestKind {
    /// Plan IR effect/query/write method.
    Plan,
    /// Host-to-worker stage dispatch method.
    StageRun,
}

/// Runtime owner for validated Leaven public-seam methods.
pub trait SeamService {
    /// Handles a validated Plan IR method request and returns an extension result
    /// payload. The runtime validates the returned payload before transport
    /// serialization.
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError>;

    /// Handles a validated `leaven/stage.run` dispatch and returns a stage-run
    /// result payload. The runtime validates the returned payload before
    /// transport serialization.
    fn handle_stage_run(&self, request: SeamStageRunRequest<'_>)
    -> Result<Value, SeamServiceError>;
}

/// A service that exposes the whole seam but implements no method bodies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RejectingSeamService;

impl SeamService for RejectingSeamService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable(request.method()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable(STAGE_RUN_METHOD))
    }
}

/// Error raised by a [`SeamService`].
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct SeamServiceError {
    message: String,
}

impl SeamServiceError {
    /// Creates a method-unavailable service error.
    pub fn unavailable(method: impl Into<String>) -> Self {
        Self {
            message: format!(
                "Leaven seam method `{}` is not implemented by this service",
                method.into()
            ),
        }
    }
}

/// Runtime construction error.
#[derive(Debug, thiserror::Error)]
pub enum SeamRuntimeError {
    /// Public-seam package/profile loading failed.
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct RequestError {
    code: JsonRpcErrorCode,
    message: String,
}

impl RequestError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: JsonRpcErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    fn from_public_seam(error: &PublicSeamError) -> Self {
        Self {
            code: JsonRpcErrorCode::InvalidRequest,
            message: error.to_string(),
        }
    }
}

fn error_response(
    id: &Value,
    code: JsonRpcErrorCode,
    message: impl Into<String>,
) -> JsonRpcResponse {
    JsonRpcResponse {
        value: json!({
            "jsonrpc": "2.0",
            "id": id.clone(),
            "error": {
                "code": code.code(),
                "message": message.into()
            }
        }),
    }
}

fn response_id(value: &Value) -> Value {
    match value.get("id") {
        Some(id @ (Value::String(_) | Value::Number(_))) => id.clone(),
        _ => Value::Null,
    }
}
