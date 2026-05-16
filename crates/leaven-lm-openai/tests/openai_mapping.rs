use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::num::NonZeroUsize;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;

use leaven_kernel::{Cost, FiniteF64};
use leaven_lm::{
    JsonSchemaOutput, Lm, LmContinuation, LmError, LmRequest, Messages, ModelName, OutputMode,
    ProviderHints, ProviderName, ReasoningEffort, Role, SamplingOptions,
};
use leaven_lm_openai::{OpenAiConfig, OpenAiLm, OpenAiRetryPolicy, OpenAiThrottlePolicy};

type FixtureHeaders = &'static [(&'static str, &'static str)];
type FixtureResponse = (&'static str, FixtureHeaders, &'static str);

#[test]
fn openai_request_uses_previous_response_id_for_uncovered_suffix() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let request = LmRequest::new(
        ModelName::new("gpt-4.1-mini"),
        Messages::new()
            .with_user("first")
            .with_assistant("first answer")
            .with_user("second"),
    )
    .with_continuation(LmContinuation {
        provider: ProviderName::new("openai"),
        response_id: "resp_123".to_owned(),
        covered_messages: 2,
    });

    let wire = lm.to_wire_request(&request).unwrap();

    assert_eq!(wire["previous_response_id"], "resp_123");
    assert_eq!(wire["input"].as_array().unwrap().len(), 1);
    assert_eq!(wire["input"][0]["role"], "user");
    assert_eq!(wire["input"][0]["content"], "second");
}

#[test]
fn openai_request_keeps_system_message_in_uncovered_continuation_suffix() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let request = LmRequest::new(
        ModelName::new("gpt-4.1-mini"),
        Messages::new()
            .with_user("covered")
            .with_system("new instruction")
            .with_user("new question"),
    )
    .with_continuation(LmContinuation {
        provider: ProviderName::new("openai"),
        response_id: "resp_123".to_owned(),
        covered_messages: 1,
    });

    let wire = lm.to_wire_request(&request).unwrap();

    assert_eq!(wire["previous_response_id"], "resp_123");
    assert_eq!(wire["instructions"], "new instruction");
    assert_eq!(wire["input"][0]["role"], "user");
    assert_eq!(wire["input"][0]["content"], "new question");
}

#[test]
fn openai_identity_and_fingerprint_are_stable() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));

    assert_eq!(lm.id().as_str(), "openai");
    assert_eq!(
        lm.fingerprint(),
        OpenAiLm::new(OpenAiConfig::new("other-key")).fingerprint()
    );
    assert_ne!(
        lm.fingerprint(),
        OpenAiLm::new(OpenAiConfig::new("test-key").with_base_url("http://localhost/custom"))
            .fingerprint()
    );
    assert_ne!(
        lm.fingerprint(),
        OpenAiLm::new(
            OpenAiConfig::new("test-key").with_retry_policy(OpenAiRetryPolicy::new(
                1,
                Duration::ZERO,
                Duration::ZERO
            ))
        )
        .fingerprint()
    );
    assert_ne!(
        lm.fingerprint(),
        OpenAiLm::new(OpenAiConfig::new("test-key").with_request_timeout(Duration::from_secs(5)))
            .fingerprint()
    );
}

#[test]
fn openai_from_env_reads_api_key_in_child_process() {
    if std::env::var_os("LEAVEN_OPENAI_FROM_ENV_CHILD").is_some() {
        let lm = OpenAiLm::from_env().unwrap();
        assert_eq!(lm.id().as_str(), "openai");
        return;
    }

    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("openai_from_env_reads_api_key_in_child_process")
        .arg("--nocapture")
        .env("LEAVEN_OPENAI_FROM_ENV_CHILD", "1")
        .env("OPENAI_API_KEY", "test-key")
        .status()
        .unwrap();

    assert!(status.success());
}

#[test]
fn openai_request_ignores_other_provider_continuation_and_rejects_impossible_suffix() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let base = LmRequest::new(
        ModelName::new("gpt-4.1-mini"),
        Messages::new().with_user("first").with_user("second"),
    );
    let other_provider = base.clone().with_continuation(LmContinuation {
        provider: ProviderName::new("anthropic"),
        response_id: "msg_123".to_owned(),
        covered_messages: 1,
    });

    let wire = lm.to_wire_request(&other_provider).unwrap();
    assert!(wire.get("previous_response_id").is_none());
    assert_eq!(wire["input"].as_array().unwrap().len(), 2);

    let invalid = base.with_continuation(LmContinuation {
        provider: ProviderName::new("openai"),
        response_id: "resp_123".to_owned(),
        covered_messages: 99,
    });
    let error = lm.to_wire_request(&invalid).unwrap_err();
    assert!(matches!(error, LmError::InvalidRequest { .. }));
}

#[test]
fn openai_request_lowers_system_to_instructions_and_prompt_cache_key() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let request = LmRequest::new(
        ModelName::new("gpt-4.1-mini"),
        Messages::new().with_system("be terse").with_user("hello"),
    )
    .with_provider_hints(ProviderHints {
        prompt_cache_key: Some("stable-suite".to_owned()),
        store: Some(false),
        metadata: Default::default(),
    });

    let wire = lm.to_wire_request(&request).unwrap();

    assert_eq!(wire["instructions"], "be terse");
    assert_eq!(wire["prompt_cache_key"], "stable-suite");
    assert_eq!(wire["store"], false);
    assert_eq!(wire["input"][0]["role"], "user");
}

#[test]
fn openai_request_lowers_sampling_metadata_and_output_modes() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let mut metadata = std::collections::BTreeMap::new();
    metadata.insert("split".to_owned(), "train".to_owned());
    let request = LmRequest::new(
        "gpt-4.1-mini",
        Messages::new()
            .with_system("be terse")
            .with_system("answer as json")
            .with_user("hello"),
    )
    .with_sampling(SamplingOptions {
        temperature: Some(FiniteF64::new(0.2).unwrap()),
        top_p: Some(FiniteF64::new(0.95).unwrap()),
        max_output_tokens: Some(64),
        seed: Some(7),
        reasoning_effort: Some(ReasoningEffort::Medium),
    })
    .with_output(OutputMode::JsonObject)
    .with_provider_hints(ProviderHints {
        prompt_cache_key: None,
        store: None,
        metadata,
    });

    let wire = lm.to_wire_request(&request).unwrap();

    assert_eq!(wire["instructions"], "be terse\n\nanswer as json");
    assert_eq!(wire["temperature"], 0.2);
    assert_eq!(wire["top_p"], 0.95);
    assert_eq!(wire["max_output_tokens"], 64);
    assert_eq!(wire["seed"], 7);
    assert_eq!(wire["reasoning"]["effort"], "medium");
    assert_eq!(wire["text"]["format"]["type"], "json_object");
    assert_eq!(wire["metadata"]["split"], "train");
}

#[test]
fn openai_request_lowers_assistant_history_when_no_provider_suffix_is_used() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let request = LmRequest::new(
        "gpt-4.1-mini",
        Messages::new()
            .with_user("first")
            .with_assistant("first answer")
            .with_user("second"),
    );

    let wire = lm.to_wire_request(&request).unwrap();

    assert_eq!(wire["input"].as_array().unwrap().len(), 3);
    assert_eq!(wire["input"][1]["role"], "assistant");
    assert_eq!(wire["input"][1]["content"], "first answer");
}

#[tokio::test]
async fn openai_complete_refuses_invalid_continuation_before_transport() {
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key").with_base_url("http://127.0.0.1:9/v1/responses"),
    );
    let request = LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")).with_continuation(
        LmContinuation {
            provider: ProviderName::new("openai"),
            response_id: "resp_123".to_owned(),
            covered_messages: 99,
        },
    );

    let error = lm.complete(request).await.unwrap_err();

    assert!(matches!(error, LmError::InvalidRequest { .. }));
}

#[tokio::test]
async fn openai_complete_maps_transport_failure() {
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url("http://127.0.0.1:9/v1/responses")
            .with_retry_policy(OpenAiRetryPolicy::none()),
    );

    let error = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap_err();

    assert!(matches!(error, LmError::Transport { .. }));
}

#[tokio::test]
async fn openai_complete_retries_transport_failures_until_exhausted() {
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url("http://127.0.0.1:9/v1/responses")
            .with_retry_policy(OpenAiRetryPolicy::new(1, Duration::ZERO, Duration::ZERO)),
    );

    let error = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap_err();

    assert!(matches!(error, LmError::Transport { .. }));
}

#[test]
fn openai_request_lowers_json_schema_and_all_reasoning_efforts() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key"));
    let efforts = [
        (ReasoningEffort::None, "none"),
        (ReasoningEffort::Low, "low"),
        (ReasoningEffort::Medium, "medium"),
        (ReasoningEffort::High, "high"),
        (ReasoningEffort::XHigh, "xhigh"),
    ];

    for (effort, expected) in efforts {
        let request = LmRequest::new("gpt-4.1-mini", Messages::from_user("hello"))
            .with_sampling(SamplingOptions::default().with_reasoning_effort(effort))
            .with_output(OutputMode::JsonSchema(JsonSchemaOutput {
                name: "answer".to_owned(),
                schema: serde_json::json!({"type": "object"}),
                strict: true,
            }));

        let wire = lm.to_wire_request(&request).unwrap();

        assert_eq!(wire["reasoning"]["effort"], expected);
        assert_eq!(wire["text"]["format"]["type"], "json_schema");
        assert_eq!(wire["text"]["format"]["name"], "answer");
        assert_eq!(wire["text"]["format"]["schema"]["type"], "object");
        assert_eq!(wire["text"]["format"]["strict"], true);
    }
}

#[test]
fn openai_response_extracts_assistant_text_usage_and_continuation() {
    let raw = serde_json::json!({
        "id": "resp_abc",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "hello"}]
        }],
        "usage": {
            "input_tokens": 10,
            "input_tokens_details": {"cached_tokens": 4},
            "output_tokens": 6,
            "output_tokens_details": {"reasoning_tokens": 2}
        }
    });

    let parsed = OpenAiLm::parse_response(&raw, 3).unwrap();

    assert_eq!(parsed.assistant.role(), Role::Assistant);
    assert_eq!(parsed.assistant.content(), "hello");
    assert_eq!(parsed.provider_response_id.as_deref(), Some("resp_abc"));
    assert_eq!(parsed.usage.cached_input_tokens, 4);
    assert_eq!(parsed.usage.reasoning_tokens, 2);
    assert_eq!(parsed.continuation.unwrap().covered_messages, 4);
}

#[test]
fn openai_response_defaults_usage_when_provider_omits_it() {
    let raw = serde_json::json!({
        "id": "resp_no_usage",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "hello"}]
        }]
    });

    let parsed = OpenAiLm::parse_response(&raw, 1).unwrap();

    assert_eq!(parsed.usage.input_tokens, 0);
    assert_eq!(parsed.usage.output_tokens, 0);
}

#[test]
fn openai_response_rejects_missing_id_or_missing_assistant_text() {
    let missing_id = serde_json::json!({
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "hello"}]
        }]
    });
    let error = OpenAiLm::parse_response(&missing_id, 1).unwrap_err();
    assert!(matches!(error, LmError::InvalidResponse { .. }));

    let missing_text = serde_json::json!({
        "id": "resp_no_text",
        "output": [
            {"type": "tool_call", "content": []},
            {"type": "message", "role": "assistant", "content": [{"type": "refusal", "text": "no"}]}
        ]
    });
    let error = OpenAiLm::parse_response(&missing_text, 1).unwrap_err();
    assert!(matches!(error, LmError::InvalidResponse { .. }));

    let missing_output = serde_json::json!({"id": "resp_no_output"});
    let error = OpenAiLm::parse_response(&missing_output, 1).unwrap_err();
    assert!(matches!(error, LmError::InvalidResponse { .. }));
}

#[tokio::test]
async fn openai_complete_posts_response_and_meters_usage() {
    let body = r#"{
        "id": "resp_http",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "output_text", "text": "hello "},
                {"type": "output_text", "text": "there"}
            ]
        }],
        "usage": {"input_tokens": 2, "output_tokens": 3}
    }"#;
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key").with_base_url(serve_once("200 OK", body)));

    let metered = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap();

    assert_eq!(metered.value.assistant.content(), "hello there");
    assert_eq!(
        metered.cost,
        Cost {
            llm_calls: 1,
            prompt_tokens: 2,
            completion_tokens: 3,
            ..Cost::zero()
        }
    );
    assert_eq!(metered.value.continuation.unwrap().covered_messages, 2);
}

#[tokio::test]
async fn openai_complete_retries_retryable_statuses_then_succeeds() {
    let success_body = r#"{
        "id": "resp_after_retry",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "after retry"}]
        }],
        "usage": {"input_tokens": 4, "output_tokens": 5}
    }"#;
    let (url, attempts) = serve_sequence([
        (
            "429 Too Many Requests",
            [("retry-after", "999")].as_slice(),
            "slow down",
        ),
        ("200 OK", [].as_slice(), success_body),
    ]);
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url(url)
            .with_retry_policy(OpenAiRetryPolicy::new(1, Duration::ZERO, Duration::ZERO)),
    );

    let metered = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap();

    assert_eq!(metered.value.assistant.content(), "after retry");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn openai_complete_uses_backoff_when_retry_after_is_absent() {
    let success_body = r#"{
        "id": "resp_after_backoff",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "after backoff"}]
        }]
    }"#;
    let (url, attempts) = serve_sequence([
        ("503 Service Unavailable", [].as_slice(), "busy"),
        ("200 OK", [].as_slice(), success_body),
    ]);
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url(url)
            .with_retry_policy(OpenAiRetryPolicy::new(1, Duration::ZERO, Duration::ZERO)),
    );

    let metered = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap();

    assert_eq!(metered.value.assistant.content(), "after backoff");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn openai_complete_does_not_retry_non_retryable_statuses() {
    let (url, attempts) = serve_sequence([("400 Bad Request", [].as_slice(), "bad request")]);
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url(url)
            .with_retry_policy(OpenAiRetryPolicy::new(3, Duration::ZERO, Duration::ZERO)),
    );

    let error = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "openai lm provider failed with status 400: bad request"
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn openai_complete_maps_status_failure_to_provider_error() {
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url(serve_once("429 Too Many Requests", "slow down"))
            .with_retry_policy(OpenAiRetryPolicy::none()),
    );

    let error = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "openai lm provider failed with status 429: slow down"
    );
}

#[tokio::test]
async fn openai_complete_respects_configured_concurrency_limit() {
    let body = r#"{
        "id": "resp_throttled",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "ok"}]
        }]
    }"#;
    let (url, max_active) = serve_concurrent(2, body);
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url(url)
            .with_retry_policy(OpenAiRetryPolicy::none())
            .with_throttle_policy(OpenAiThrottlePolicy::new(
                NonZeroUsize::new(1).unwrap(),
                Duration::ZERO,
            )),
    );
    let first = lm.complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("one")));
    let second = lm.complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("two")));

    let (first, second) = tokio::join!(first, second);

    first.unwrap();
    second.unwrap();
    assert_eq!(max_active.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn openai_complete_times_out_waiting_for_throttle_permit() {
    let body = r#"{
        "id": "resp_throttled",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "ok"}]
        }]
    }"#;
    let (url, _max_active) = serve_concurrent(1, body);
    let lm = OpenAiLm::new(
        OpenAiConfig::new("test-key")
            .with_base_url(url)
            .with_retry_policy(OpenAiRetryPolicy::none())
            .with_throttle_policy(OpenAiThrottlePolicy::new(
                NonZeroUsize::new(1).unwrap(),
                Duration::from_millis(1),
            )),
    );
    let first = tokio::spawn({
        let lm = lm.clone();
        async move {
            lm.complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("one")))
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(5)).await;

    let second = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("two")))
        .await
        .unwrap_err();

    assert!(second.to_string().contains("throttle timed out"));
    first.await.unwrap().unwrap();
}

#[tokio::test]
async fn openai_complete_maps_invalid_json_to_invalid_response() {
    let lm = OpenAiLm::new(OpenAiConfig::new("test-key").with_base_url(serve_once("200 OK", "{")));

    let error = lm
        .complete(LmRequest::new("gpt-4.1-mini", Messages::from_user("hi")))
        .await
        .unwrap_err();

    assert!(matches!(error, LmError::InvalidResponse { .. }));
}

fn serve_once(status: &'static str, body: &'static str) -> String {
    serve_sequence([(status, [].as_slice(), body)]).0
}

fn serve_concurrent(requests: usize, body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let url = format!("http://{}/v1/responses", listener.local_addr().unwrap());
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let thread_active = active;
    let thread_max_active = max_active.clone();
    thread::spawn(move || {
        let mut handles = Vec::new();
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().unwrap();
            let active = thread_active.clone();
            let max_active = thread_max_active.clone();
            handles.push(thread::spawn(move || {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(current, Ordering::SeqCst);
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).unwrap();
                thread::sleep(Duration::from_millis(50));
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });
    (url, max_active)
}

fn serve_sequence<const N: usize>(responses: [FixtureResponse; N]) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let url = format!("http://{}/v1/responses", listener.local_addr().unwrap());
    let attempts = Arc::new(AtomicUsize::new(0));
    let thread_attempts = attempts.clone();
    thread::spawn(move || {
        for (status, headers, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            thread_attempts.fetch_add(1, Ordering::SeqCst);
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            let mut headers_text = String::new();
            for (name, value) in headers {
                let _ = write!(headers_text, "{name}: {value}\r\n");
            }
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n{headers_text}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (url, attempts)
}
