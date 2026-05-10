use std::io;

use leaven_kernel::{Cost, FiniteF64};
use leaven_lm::{
    JsonSchemaOutput, LmError, LmId, LmResponse, Message, Messages, ModelName, OutputMode,
    ProviderHints, ProviderName, ReasoningEffort, Role, SamplingOptions, TokenUsage,
};

#[test]
fn messages_preserve_multi_turn_order() {
    let mut messages = Messages::new()
        .with_system("answer briefly")
        .with_user("one")
        .with_assistant("two")
        .with_user("three");
    messages.push(Message::user("four"));

    let roles = messages.iter().map(Message::role).collect::<Vec<_>>();

    assert_eq!(
        roles,
        vec![
            Role::System,
            Role::User,
            Role::Assistant,
            Role::User,
            Role::User
        ]
    );
    assert_eq!(messages.len(), 5);
    assert_eq!(messages.as_slice()[0].content(), "answer briefly");
    assert_eq!(messages.suffix_from(3).len(), 2);
    assert!(messages.suffix_from(99).is_empty());
    assert!(Messages::new().is_empty());
}

#[test]
fn response_requires_assistant_message() {
    let usage = TokenUsage {
        input_tokens: 10,
        cached_input_tokens: 4,
        output_tokens: 5,
        reasoning_tokens: 2,
    };

    let response = LmResponse::new(Message::assistant("done"), usage.clone()).unwrap();

    assert_eq!(response.assistant.content(), "done");
    assert_eq!(response.usage, usage);
    assert!(LmResponse::new(Message::user("not an assistant"), usage).is_err());
}

#[test]
fn token_usage_maps_to_lm_call_cost() {
    let usage = TokenUsage {
        input_tokens: 12,
        cached_input_tokens: 3,
        output_tokens: 7,
        reasoning_tokens: 5,
    };

    assert_eq!(
        usage.to_cost(),
        Cost {
            llm_calls: 1,
            prompt_tokens: 12,
            completion_tokens: 7,
            ..Cost::zero()
        }
    );
}

#[test]
fn request_defaults_are_text_completion_friendly() {
    let request =
        leaven_lm::LmRequest::new(ModelName::new("gpt-4.1-mini"), Messages::from_user("solve"));

    assert_eq!(request.output, OutputMode::Text);
    assert_eq!(request.sampling, SamplingOptions::default());
    assert_eq!(request.provider_hints, ProviderHints::default());
    assert!(request.continuation.is_none());
}

#[test]
fn request_builders_preserve_sampling_output_and_provider_hints() {
    let sampling = SamplingOptions {
        temperature: Some(FiniteF64::new(0.2).unwrap()),
        top_p: Some(FiniteF64::new(0.9).unwrap()),
        ..SamplingOptions::default()
    }
    .with_max_output_tokens(128)
    .with_reasoning_effort(ReasoningEffort::High);
    let mut hints = ProviderHints::default();
    hints.metadata.insert("suite".to_owned(), "aime".to_owned());

    let request = leaven_lm::LmRequest::new("gpt-4.1-mini", Messages::from_user("solve"))
        .with_sampling(sampling.clone())
        .with_output(OutputMode::JsonSchema(JsonSchemaOutput {
            name: "answer".to_owned(),
            schema: serde_json::json!({"type": "object"}),
            strict: true,
        }))
        .with_provider_hints(hints.clone());

    assert_eq!(request.model.as_str(), "gpt-4.1-mini");
    assert_eq!(request.model.to_string(), "gpt-4.1-mini");
    assert_eq!(request.sampling, sampling);
    assert_eq!(request.provider_hints, hints);
    assert!(matches!(request.output, OutputMode::JsonSchema(_)));
}

#[test]
fn string_identifiers_round_trip_through_common_conversions() {
    let lm_id = LmId::from("mock");
    let model = ModelName::from(String::from("gpt-4.1-mini"));
    let provider = ProviderName::new("openai");

    assert_eq!(lm_id.as_str(), "mock");
    assert_eq!(model.to_string(), "gpt-4.1-mini");
    assert_eq!(provider.to_string(), "openai");
}

#[test]
fn lm_error_constructors_preserve_public_failure_shape() {
    let invalid_request = LmError::invalid_request("missing messages");
    assert_eq!(
        invalid_request.to_string(),
        "invalid lm request: missing messages"
    );

    let invalid_response = LmError::invalid_response("openai", "missing text");
    assert_eq!(
        invalid_response.to_string(),
        "invalid openai lm response: missing text"
    );

    let provider = LmError::provider("openai", Some(429), "rate limited");
    assert_eq!(
        provider.to_string(),
        "openai lm provider failed with status 429: rate limited"
    );

    let provider_without_status = LmError::provider("mock", None, "script exhausted");
    assert_eq!(
        provider_without_status.to_string(),
        "mock lm provider failed: script exhausted"
    );

    let transport = LmError::transport("openai", io::Error::other("socket closed"));
    assert_eq!(transport.to_string(), "openai lm transport failed");

    let cache = LmError::cache("disk refused write");
    assert_eq!(
        cache.to_string(),
        "lm response cache failed: disk refused write"
    );
}
