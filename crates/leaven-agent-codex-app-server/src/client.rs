//! Typed JSON-RPC client for Codex app-server.

#![cfg(feature = "app-server")]

use std::collections::VecDeque;

use codex_app_server_protocol::{
    ClientInfo, ClientRequest, CommandExecutionApprovalDecision,
    CommandExecutionRequestApprovalResponse, FileChangeApprovalDecision,
    FileChangeRequestApprovalResponse, InitializeCapabilities, InitializeParams,
    InitializeResponse, JSONRPCError, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest,
    JSONRPCResponse, RequestId, ServerRequest, ThreadReadParams, ThreadReadResponse,
    ThreadStartParams, ThreadStartResponse, TurnStartParams, TurnStartResponse,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{CodexAppServerError, Result};
use crate::transport::CodexAppServerTransport;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ApprovalMode {
    #[default]
    Error,
    Accept,
    Decline,
    Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializeOptions {
    pub(crate) client_name: String,
    pub(crate) client_title: Option<String>,
    pub(crate) experimental_api: bool,
    pub(crate) opt_out_notification_methods: Option<Vec<String>>,
}

pub struct CodexAppServerClient<T> {
    transport: T,
    pending_notifications: VecDeque<JSONRPCNotification>,
    next_request_id: i64,
    approval_mode: ApprovalMode,
}

impl<T> CodexAppServerClient<T>
where
    T: CodexAppServerTransport,
{
    pub(crate) fn new(transport: T) -> Self {
        Self {
            transport,
            pending_notifications: VecDeque::new(),
            next_request_id: 1,
            approval_mode: ApprovalMode::default(),
        }
    }

    pub(crate) fn with_approval_mode(mut self, approval_mode: ApprovalMode) -> Self {
        self.approval_mode = approval_mode;
        self
    }

    #[cfg(test)]
    pub(crate) fn transport(&self) -> &T {
        &self.transport
    }

    pub(crate) async fn initialize(
        &mut self,
        options: InitializeOptions,
    ) -> Result<InitializeResponse> {
        let request_id = self.request_id();
        let response = self
            .send_request(
                ClientRequest::Initialize {
                    request_id: request_id.clone(),
                    params: InitializeParams {
                        client_info: ClientInfo {
                            name: options.client_name,
                            title: options.client_title,
                            version: env!("CARGO_PKG_VERSION").to_owned(),
                        },
                        capabilities: Some(InitializeCapabilities {
                            experimental_api: options.experimental_api,
                            request_attestation: false,
                            opt_out_notification_methods: options.opt_out_notification_methods,
                        }),
                    },
                },
                request_id,
                "initialize",
            )
            .await?;

        self.write_jsonrpc_message(JSONRPCMessage::Notification(JSONRPCNotification {
            method: "initialized".to_owned(),
            params: None,
        }))
        .await?;

        Ok(response)
    }

    pub(crate) async fn thread_start(
        &mut self,
        params: ThreadStartParams,
    ) -> Result<ThreadStartResponse> {
        let request_id = self.request_id();
        self.send_request(
            ClientRequest::ThreadStart {
                request_id: request_id.clone(),
                params,
            },
            request_id,
            "thread/start",
        )
        .await
    }

    pub(crate) async fn thread_read(
        &mut self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse> {
        let request_id = self.request_id();
        self.send_request(
            ClientRequest::ThreadRead {
                request_id: request_id.clone(),
                params,
            },
            request_id,
            "thread/read",
        )
        .await
    }

    pub(crate) async fn turn_start(
        &mut self,
        params: TurnStartParams,
    ) -> Result<TurnStartResponse> {
        let request_id = self.request_id();
        self.send_request(
            ClientRequest::TurnStart {
                request_id: request_id.clone(),
                params,
            },
            request_id,
            "turn/start",
        )
        .await
    }

    pub(crate) async fn next_raw_notification(&mut self) -> Result<JSONRPCNotification> {
        self.next_notification().await
    }

    pub(crate) async fn shutdown(&mut self) -> Result<()> {
        self.transport.shutdown().await
    }

    async fn send_request<R>(
        &mut self,
        request: ClientRequest,
        request_id: RequestId,
        method: &str,
    ) -> Result<R>
    where
        R: DeserializeOwned,
    {
        self.write_request(&request).await?;
        self.wait_for_response(request_id, method).await
    }

    async fn write_request(&mut self, request: &ClientRequest) -> Result<()> {
        let payload = encode_client_request(request)?;
        self.transport.write_payload(&payload).await
    }

    async fn write_jsonrpc_message(&mut self, message: JSONRPCMessage) -> Result<()> {
        self.transport
            .write_payload(&serde_json::to_string(&message)?)
            .await
    }

    async fn wait_for_response<R>(&mut self, request_id: RequestId, method: &str) -> Result<R>
    where
        R: DeserializeOwned,
    {
        loop {
            match self.read_jsonrpc_message().await? {
                JSONRPCMessage::Response(JSONRPCResponse { id, result }) if id == request_id => {
                    let payload = result.to_string();
                    return serde_json::from_value(result).map_err(|source| {
                        CodexAppServerError::ResponseDecode {
                            method: method.to_owned(),
                            payload,
                            source,
                        }
                    });
                }
                JSONRPCMessage::Error(JSONRPCError { id, error }) if id == request_id => {
                    return Err(CodexAppServerError::JsonRpc {
                        id: format!("{id:?}"),
                        code: error.code,
                        message: format!("{method}: {}", error.message),
                        data: error.data.map(|value| value.to_string()),
                    });
                }
                JSONRPCMessage::Notification(notification) => {
                    self.pending_notifications.push_back(notification);
                }
                JSONRPCMessage::Request(request) => {
                    self.handle_server_request(request).await?;
                }
                JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => {}
            }
        }
    }

    async fn next_notification(&mut self) -> Result<JSONRPCNotification> {
        if let Some(notification) = self.pending_notifications.pop_front() {
            return Ok(notification);
        }

        loop {
            match self.read_jsonrpc_message().await? {
                JSONRPCMessage::Notification(notification) => return Ok(notification),
                JSONRPCMessage::Request(request) => self.handle_server_request(request).await?,
                JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => {}
            }
        }
    }

    async fn read_jsonrpc_message(&mut self) -> Result<JSONRPCMessage> {
        loop {
            let payload = self.transport.read_payload().await?;
            if payload.trim().is_empty() {
                continue;
            }
            return serde_json::from_str(payload.trim()).map_err(Into::into);
        }
    }

    async fn handle_server_request(&mut self, request: JSONRPCRequest) -> Result<()> {
        let server_request = ServerRequest::try_from(request)
            .map_err(|error| CodexAppServerError::Protocol(error.to_string()))?;

        match server_request {
            ServerRequest::CommandExecutionRequestApproval { request_id, .. } => {
                let response = CommandExecutionRequestApprovalResponse {
                    decision: match self.approval_mode {
                        ApprovalMode::Error => return Err(CodexAppServerError::ApprovalRequested),
                        ApprovalMode::Accept => CommandExecutionApprovalDecision::Accept,
                        ApprovalMode::Decline => CommandExecutionApprovalDecision::Decline,
                        ApprovalMode::Cancel => CommandExecutionApprovalDecision::Cancel,
                    },
                };
                self.send_server_request_response(request_id, &response)
                    .await
            }
            ServerRequest::FileChangeRequestApproval { request_id, .. } => {
                let response = FileChangeRequestApprovalResponse {
                    decision: match self.approval_mode {
                        ApprovalMode::Error => return Err(CodexAppServerError::ApprovalRequested),
                        ApprovalMode::Accept => FileChangeApprovalDecision::Accept,
                        ApprovalMode::Decline => FileChangeApprovalDecision::Decline,
                        ApprovalMode::Cancel => FileChangeApprovalDecision::Cancel,
                    },
                };
                self.send_server_request_response(request_id, &response)
                    .await
            }
            other => Err(CodexAppServerError::UnsupportedServerRequest {
                method: format!("{other:?}"),
            }),
        }
    }

    async fn send_server_request_response<R>(
        &mut self,
        request_id: RequestId,
        response: &R,
    ) -> Result<()>
    where
        R: Serialize + Sync,
    {
        self.write_jsonrpc_message(JSONRPCMessage::Response(JSONRPCResponse {
            id: request_id,
            result: serde_json::to_value(response)?,
        }))
        .await
    }

    fn request_id(&mut self) -> RequestId {
        let id = self.next_request_id;
        self.next_request_id += 1;
        RequestId::String(format!("leaven-codex-{id}"))
    }
}

pub fn encode_client_request(request: &ClientRequest) -> Result<String> {
    let request_value = serde_json::to_value(request)?;
    let request: JSONRPCRequest = serde_json::from_value(request_value)?;
    Ok(serde_json::to_string(&request)?)
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::{JSONRPCMessage, JSONRPCResponse, TurnStartParams, UserInput};

    use super::*;
    use crate::transport::tests::MockTransport;

    #[tokio::test]
    async fn turn_start_serializes_app_server_method() {
        let response = JSONRPCMessage::Response(JSONRPCResponse {
            id: RequestId::String("leaven-codex-1".to_owned()),
            result: serde_json::json!({
                "turn": {
                    "id": "turn-1",
                    "items": [],
                    "status": "inProgress",
                    "error": null,
                    "startedAt": null,
                    "completedAt": null,
                    "durationMs": null
                }
            }),
        });
        let mut client = CodexAppServerClient::new(MockTransport::new(vec![
            serde_json::to_string(&response).unwrap(),
        ]));

        let output = client
            .turn_start(TurnStartParams {
                thread_id: "thread-1".to_owned(),
                input: vec![UserInput::Text {
                    text: "mutate the skill".to_owned(),
                    text_elements: Vec::new(),
                }],
                ..TurnStartParams::default()
            })
            .await
            .unwrap();

        assert_eq!(output.turn.id, "turn-1");
        let written: serde_json::Value =
            serde_json::from_str(&client.transport().written[0]).unwrap();
        assert_eq!(written["method"], "turn/start");
        assert_eq!(written["params"]["threadId"], "thread-1");
        assert_eq!(written["params"]["input"][0]["text"], "mutate the skill");
    }
}
