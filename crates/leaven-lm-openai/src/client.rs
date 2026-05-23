use std::{sync::Arc, time::Duration};

use leaven_kernel::{Fingerprint, FingerprintBuilder, Metered};
use leaven_lm::{
    Lm, LmContinuation, LmError, LmId, LmRequest, LmResponse, LmTool, Message, MessageContentPart,
    OutputMode, ProviderName, ReasoningEffort, Role, SamplingOptions, TokenUsage,
};
use reqwest::{StatusCode, header::RETRY_AFTER};
use serde_json::{Map, Value, json};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::OpenAiConfig;

/// `OpenAI` Responses API implementation of the neutral [`Lm`] trait.
#[derive(Clone)]
pub struct OpenAiLm {
    config: OpenAiConfig,
    client: reqwest::Client,
    throttle: Arc<Semaphore>,
}

impl OpenAiLm {
    /// Creates an `OpenAI` LM provider from config.
    #[must_use]
    pub fn new(config: OpenAiConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout())
            .build()
            .expect("OpenAI reqwest client config is valid");
        let throttle = Arc::new(Semaphore::new(
            config.throttle_policy().max_concurrent_requests().get(),
        ));
        Self {
            config,
            client,
            throttle,
        }
    }

    /// Creates an `OpenAI` LM provider from `OPENAI_API_KEY`.
    ///
    /// Requests carry their explicit model; this provider has no implicit
    /// model default.
    ///
    /// # Errors
    ///
    /// Returns [`LmError::InvalidRequest`] when `OPENAI_API_KEY` is missing.
    pub fn from_env() -> Result<Self, LmError> {
        Ok(Self::new(OpenAiConfig::from_env()?))
    }

    /// Lowers a neutral request into an `OpenAI` Responses API JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`LmError::InvalidRequest`] when continuation state is invalid.
    pub fn to_wire_request(&self, request: &LmRequest) -> Result<Value, LmError> {
        let mut object = Map::new();
        object.insert("model".to_owned(), json!(request.model.as_str()));

        if let Some(instructions) = instructions(request.messages.as_slice()) {
            object.insert("instructions".to_owned(), Value::String(instructions));
        }

        let start = openai_suffix_start(request)?;
        let input = request
            .messages
            .suffix_from(start)
            .iter()
            .filter(|message| !matches!(message.role(), Role::System | Role::Developer))
            .map(openai_message)
            .collect::<Vec<_>>();
        object.insert("input".to_owned(), Value::Array(input));

        if let Some(continuation) = continuation_for_openai(request) {
            object.insert(
                "previous_response_id".to_owned(),
                Value::String(continuation.response_id.clone()),
            );
        }

        lower_sampling(&mut object, &request.sampling);
        lower_output(&mut object, &request.output);

        if let Some(prompt_cache_key) = &request.provider_hints.prompt_cache_key {
            object.insert(
                "prompt_cache_key".to_owned(),
                Value::String(prompt_cache_key.clone()),
            );
        }
        if let Some(store) = request.provider_hints.store {
            object.insert("store".to_owned(), Value::Bool(store));
        }
        if !request.provider_hints.metadata.is_empty() {
            object.insert(
                "metadata".to_owned(),
                json!(request.provider_hints.metadata),
            );
        }
        if !request.tools.is_empty() {
            object.insert(
                "tools".to_owned(),
                Value::Array(request.tools.iter().map(openai_tool).collect()),
            );
        }

        Ok(Value::Object(object))
    }

    /// Lowers an `OpenAI` Responses API JSON body into a neutral response.
    ///
    /// # Errors
    ///
    /// Returns [`LmError::InvalidResponse`] when assistant text or usage cannot
    /// be extracted.
    pub fn parse_response(
        raw: &Value,
        request_message_count: usize,
    ) -> Result<LmResponse, LmError> {
        let response_id = raw
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| LmError::invalid_response("openai", "missing response id"))?
            .to_owned();
        let text = assistant_text(raw)
            .ok_or_else(|| LmError::invalid_response("openai", "missing assistant output text"))?;
        let usage = token_usage(raw.get("usage"));
        let continuation = LmContinuation {
            provider: ProviderName::new("openai"),
            response_id: response_id.clone(),
            covered_messages: request_message_count + 1,
        };
        LmResponse::new(Message::assistant(text), usage)
            .map_err(|error| LmError::invalid_response("openai", error.to_string()))
            .map(|response| {
                response
                    .with_provider_response_id(response_id)
                    .with_continuation(continuation)
            })
    }
}

impl Lm for OpenAiLm {
    fn id(&self) -> LmId {
        LmId::new("openai")
    }

    fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven-lm-openai-responses-v1");
        builder.update(self.config.base_url().as_bytes());
        builder.update(self.config.request_timeout().as_millis().to_string());
        builder.update(self.config.retry_policy().max_retries().to_string());
        builder.update(
            self.config
                .retry_policy()
                .initial_backoff()
                .as_millis()
                .to_string(),
        );
        builder.update(
            self.config
                .retry_policy()
                .max_backoff()
                .as_millis()
                .to_string(),
        );
        builder.update(
            self.config
                .throttle_policy()
                .max_concurrent_requests()
                .get()
                .to_string(),
        );
        builder.update(
            self.config
                .throttle_policy()
                .acquire_timeout()
                .as_millis()
                .to_string(),
        );
        builder.finish()
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        let body = self.to_wire_request(&request)?;
        let response = self.send_with_retries(&body).await?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| LmError::transport("openai", error))?;
        if !status.is_success() {
            return Err(LmError::provider("openai", Some(status.as_u16()), text));
        }
        let raw = serde_json::from_str::<Value>(&text)
            .map_err(|error| LmError::invalid_response("openai", error.to_string()))?;
        let parsed = Self::parse_response(&raw, request.messages.len())?;
        let cost = parsed.usage.to_cost();
        Ok(Metered::new(parsed, cost))
    }
}

impl OpenAiLm {
    async fn send_with_retries(&self, body: &Value) -> Result<reqwest::Response, LmError> {
        let policy = self.config.retry_policy();
        let mut attempt = 0;
        loop {
            let permit = self.acquire_throttle_permit().await?;
            let result = self
                .client
                .post(self.config.base_url())
                .bearer_auth(self.config.api_key())
                .json(body)
                .send()
                .await;
            drop(permit);

            match result {
                Ok(response) => {
                    let status = response.status();
                    if !is_retryable_status(status) || attempt >= policy.max_retries() {
                        return Ok(response);
                    }
                    let delay = retry_after_delay(response.headers())
                        .map(|delay| delay.min(policy.max_backoff()))
                        .unwrap_or_else(|| retry_delay(policy, attempt));
                    tokio::time::sleep(delay).await;
                }
                Err(error) => {
                    if attempt >= policy.max_retries() {
                        return Err(LmError::transport("openai", error));
                    }
                    tokio::time::sleep(retry_delay(policy, attempt)).await;
                }
            }

            attempt += 1;
        }
    }

    async fn acquire_throttle_permit(&self) -> Result<OwnedSemaphorePermit, LmError> {
        let acquire = self.throttle.clone().acquire_owned();
        let timeout = self.config.throttle_policy().acquire_timeout();
        if timeout.is_zero() {
            return acquire.await.map_err(|_| {
                LmError::invalid_request("OpenAI throttle closed before request could start")
            });
        }
        tokio::time::timeout(timeout, acquire)
            .await
            .map_err(|_| {
                LmError::invalid_request("OpenAI throttle timed out before request could start")
            })?
            .map_err(|_| {
                LmError::invalid_request("OpenAI throttle closed before request could start")
            })
    }
}

fn continuation_for_openai(request: &LmRequest) -> Option<&LmContinuation> {
    request
        .continuation
        .as_ref()
        .filter(|continuation| continuation.provider.as_str() == "openai")
}

fn openai_suffix_start(request: &LmRequest) -> Result<usize, LmError> {
    let Some(continuation) = continuation_for_openai(request) else {
        return Ok(0);
    };
    if continuation.covered_messages > request.messages.len() {
        return Err(LmError::invalid_request(
            "OpenAI continuation covers more messages than the request contains",
        ));
    }
    Ok(continuation.covered_messages)
}

fn instructions(messages: &[Message]) -> Option<String> {
    let values = messages
        .iter()
        .filter(|message| matches!(message.role(), Role::System | Role::Developer))
        .map(Message::content)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.join("\n\n"))
    }
}

fn openai_message(message: &Message) -> Value {
    let role = match message.role() {
        Role::System => "system",
        Role::Developer => "developer",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => {
            return json!({
                "type": "function_call_output",
                "call_id": message.tool_call_id().unwrap_or_default(),
                "output": message.content(),
            });
        }
    };
    json!({
        "role": role,
        "content": openai_message_content(message),
    })
}

fn openai_message_content(message: &Message) -> Value {
    let parts = message.content_parts();
    if let [MessageContentPart::Text { text }] = parts {
        return Value::String(text.clone());
    }
    Value::Array(
        parts
            .iter()
            .map(|part| match part {
                MessageContentPart::Text { text } => {
                    json!({ "type": "input_text", "text": text })
                }
                MessageContentPart::ToolResult {
                    tool_call_id,
                    content,
                } => {
                    json!({
                        "type": "function_call_output",
                        "call_id": tool_call_id,
                        "output": content,
                    })
                }
            })
            .collect(),
    )
}

fn openai_tool(tool: &LmTool) -> Value {
    let mut object = Map::new();
    object.insert("type".to_owned(), Value::String("function".to_owned()));
    object.insert("name".to_owned(), Value::String(tool.name.clone()));
    if let Some(description) = &tool.description {
        object.insert("description".to_owned(), Value::String(description.clone()));
    }
    object.insert("parameters".to_owned(), tool.input_schema.clone());
    Value::Object(object)
}

fn lower_sampling(object: &mut Map<String, Value>, sampling: &SamplingOptions) {
    if let Some(temperature) = sampling.temperature {
        object.insert("temperature".to_owned(), json!(temperature.as_f64()));
    }
    if let Some(top_p) = sampling.top_p {
        object.insert("top_p".to_owned(), json!(top_p.as_f64()));
    }
    if let Some(max_output_tokens) = sampling.max_output_tokens {
        object.insert("max_output_tokens".to_owned(), json!(max_output_tokens));
    }
    if let Some(seed) = sampling.seed {
        object.insert("seed".to_owned(), json!(seed));
    }
    if !sampling.stop.is_empty() {
        object.insert("stop".to_owned(), json!(sampling.stop));
    }
    if let Some(effort) = sampling.reasoning_effort {
        object.insert(
            "reasoning".to_owned(),
            json!({ "effort": reasoning_effort(effort) }),
        );
    }
}

fn lower_output(object: &mut Map<String, Value>, output: &OutputMode) {
    match output {
        OutputMode::Text => {}
        OutputMode::FinalMessage { .. } => {
            object.insert("text".to_owned(), json!({ "format": { "type": "text" } }));
        }
        OutputMode::JsonObject => {
            object.insert(
                "text".to_owned(),
                json!({ "format": { "type": "json_object" } }),
            );
        }
        OutputMode::JsonSchema(schema) => {
            object.insert(
                "text".to_owned(),
                json!({
                    "format": {
                        "type": "json_schema",
                        "name": schema.name,
                        "schema": schema.schema,
                        "strict": schema.strict,
                    }
                }),
            );
        }
    }
}

fn reasoning_effort(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::None => "none",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
    }
}

fn assistant_text(raw: &Value) -> Option<String> {
    let output = raw.get("output")?.as_array()?;
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message")
            || item.get("role").and_then(Value::as_str) != Some("assistant")
        {
            continue;
        }
        let content = item.get("content")?.as_array()?;
        let text = content
            .iter()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<String>();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn token_usage(value: Option<&Value>) -> TokenUsage {
    let Some(usage) = value else {
        return TokenUsage::default();
    };
    TokenUsage {
        input_tokens: field(usage, "input_tokens"),
        cached_input_tokens: usage
            .get("input_tokens_details")
            .map_or(0, |details| field(details, "cached_tokens")),
        output_tokens: field(usage, "output_tokens"),
        reasoning_tokens: usage
            .get("output_tokens_details")
            .map_or(0, |details| field(details, "reasoning_tokens")),
    }
}

fn field(value: &Value, name: &str) -> u64 {
    value.get(name).and_then(Value::as_u64).unwrap_or(0)
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 409 | 425 | 429 | 500 | 502 | 503 | 504
    )
}

fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?;
    let seconds = value.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds))
}

fn retry_delay(policy: &crate::OpenAiRetryPolicy, attempt: u32) -> Duration {
    let multiplier = 1_u32.checked_shl(attempt.min(31)).unwrap_or(u32::MAX);
    policy
        .initial_backoff()
        .saturating_mul(multiplier)
        .min(policy.max_backoff())
}
