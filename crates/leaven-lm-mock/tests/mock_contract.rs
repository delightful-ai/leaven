use leaven_lm::{Lm, LmRequest, Messages, ModelName};
use leaven_lm_mock::{MockLm, MockLmScript};

fn request(text: &str) -> LmRequest {
    LmRequest::new(ModelName::new("mock"), Messages::from_user(text))
}

#[tokio::test]
async fn scripted_mock_consumes_responses_in_order() {
    let lm = MockLm::new(
        MockLmScript::new()
            .then_text("first", 2, 3)
            .then_text("second", 4, 5),
    );

    let first = lm.complete(request("a")).await.unwrap();
    let second = lm.complete(request("b")).await.unwrap();

    assert_eq!(first.value.assistant.content(), "first");
    assert_eq!(second.value.assistant.content(), "second");
    assert_eq!(first.cost.prompt_tokens, 2);
    assert_eq!(second.cost.completion_tokens, 5);
}

#[tokio::test]
async fn scripted_mock_errors_when_exhausted() {
    let lm = MockLm::new(MockLmScript::new().then_text("only", 1, 1));

    lm.complete(request("a")).await.unwrap();
    let err = lm.complete(request("b")).await.unwrap_err();

    assert!(err.to_string().contains("mock script exhausted"));
}

#[tokio::test]
async fn mock_identity_fingerprint_and_default_are_stable() {
    let scripted = MockLm::new(MockLmScript::new().then_text("only", 1, 1));
    let defaulted = MockLm::default();

    assert_eq!(scripted.id().as_str(), "mock");
    assert_ne!(scripted.fingerprint(), defaulted.fingerprint());
    assert!(defaulted.complete(request("anything")).await.is_err());
}
