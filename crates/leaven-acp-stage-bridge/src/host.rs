//! Host effect handling for worker-initiated `leaven/lm.complete` callbacks.
//!
//! When the worker runs a rollout it calls `leaven/lm.complete` back into the
//! host. The transport validates the inbound Plan IR params and dispatches them
//! to [`StageRunEffectHost`], which extracts the prompt, asks the host-side
//! [`HostLm`] for a deterministic completion, and returns a locked `lm_response`
//! extension result. The transport then stamps the launched capability
//! fingerprint and writes the reply back to the worker.

use leaven_acp::{AcpEffectHost, AcpTransportError, AcpTransportResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One LM completion request lowered from a worker `leaven/lm.complete` callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LmCompletionRequest {
    prompt: String,
}

impl LmCompletionRequest {
    /// The prompt the worker asked the host to complete.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
}

/// A deterministic host-side language model.
///
/// V1 example 03 uses a deterministic mock (no spend, no network). A live
/// provider would implement this same trait behind an explicit opt-in; the
/// bidirectional seam, stage dispatch, and accept loop are unchanged.
pub trait HostLm {
    /// Produces a completion for one prompt.
    fn complete(&self, request: &LmCompletionRequest) -> String;
}

/// A deterministic mock LM that evaluates the arithmetic question in the prompt.
///
/// It scans the rendered prompt for the last `a <op> b`-style expression and
/// returns the evaluated integer as text. A prompt that does not surface the
/// arithmetic question (for example a seed prompt that drops `{question}`)
/// yields an empty completion, so a better prompt is genuinely required to score.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MockArithmeticLm;

impl HostLm for MockArithmeticLm {
    fn complete(&self, request: &LmCompletionRequest) -> String {
        evaluate_arithmetic(request.prompt()).map_or_else(String::new, |value| value.to_string())
    }
}

/// Host effect handler wiring `leaven/lm.complete` to a [`HostLm`].
///
/// Only `lm_complete` is wired for the prompt/LM/exact-match slice; every other
/// locked method rejects through the default `AcpEffectHost::service` dispatch.
/// This host owns no graph mutation, transport framing, or JSON-RPC ids.
pub struct StageRunEffectHost<'lm, L: HostLm> {
    lm: &'lm L,
}

impl<'lm, L: HostLm> StageRunEffectHost<'lm, L> {
    /// Binds a host LM to the effect handler.
    pub fn new(lm: &'lm L) -> Self {
        Self { lm }
    }
}

impl<L: HostLm> AcpEffectHost for StageRunEffectHost<'_, L> {
    fn lm_complete(&self, params: &Value) -> AcpTransportResult<Value> {
        let request = lower_lm_request(params)?;
        let completion = self.lm.complete(&request);
        lm_response_result(&completion)
    }
}

#[derive(Debug, Deserialize)]
struct LmCompleteParams {
    ops: Vec<LmCompleteOp>,
}

#[derive(Debug, Deserialize)]
struct LmCompleteOp {
    name: String,
    expr: LmCompleteExpr,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LmCompleteExpr {
    Literal { value: String },
}

impl LmCompleteParams {
    fn prompt_binding(&self) -> Option<&str> {
        self.ops.iter().find_map(|op| {
            if op.name == "prompt" {
                let LmCompleteExpr::Literal { value } = &op.expr;
                Some(value.as_str())
            } else {
                None
            }
        })
    }
}

/// Lowers a worker `leaven/lm.complete` Plan IR document into a prompt request.
///
/// The worker binds the rendered prompt as a `literal` op named `prompt`; the
/// host reads that binding rather than guessing op order.
fn lower_lm_request(params: &Value) -> AcpTransportResult<LmCompletionRequest> {
    let params: LmCompleteParams = serde_json::from_value(params.clone())
        .map_err(|_| protocol("leaven/lm.complete params must be typed Plan IR with ops"))?;
    let prompt = params
        .prompt_binding()
        .ok_or_else(|| protocol("leaven/lm.complete must bind a `prompt` literal op"))?;
    Ok(LmCompletionRequest {
        prompt: prompt.to_owned(),
    })
}

#[derive(Debug, Serialize)]
struct LmResponseResult<'a> {
    method: &'static str,
    redactions: Vec<&'static str>,
    data_classes: Vec<&'static str>,
    primary: LmResponsePrimary<'a>,
    receipts: Vec<LmResponseReceipt>,
}

#[derive(Debug, Serialize)]
struct LmResponsePrimary<'a> {
    kind: &'static str,
    message: LmResponseMessage<'a>,
    graph_revision: &'static str,
    cost: LmResponseCost,
    data_classes: Vec<&'static str>,
    replayability: &'static str,
    receipt: &'static str,
}

#[derive(Debug, Serialize)]
struct LmResponseMessage<'a> {
    role: &'static str,
    content: Vec<LmResponseTextPart<'a>>,
}

#[derive(Debug, Serialize)]
struct LmResponseTextPart<'a> {
    kind: &'static str,
    text: &'a str,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct LmResponseCost {
    usd_micro: u64,
    lm_calls: u64,
}

#[derive(Debug, Serialize)]
struct LmResponseReceipt {
    kind: &'static str,
    receipt: &'static str,
    op_var: &'static str,
    started_at: &'static str,
    completed_at: &'static str,
    call_kind: &'static str,
    request_hash: &'static str,
    result_hash: String,
    runtime_fingerprint: &'static str,
    status: &'static str,
    cost: LmResponseCost,
}

#[derive(Debug, Serialize)]
struct LmResponseResultHashPreimage<'a> {
    schema_version: &'static str,
    name: &'static str,
    value: &'a LmResponsePrimary<'a>,
}

/// Builds a locked `lm_response` extension result for one completion.
///
/// The capability fingerprint is intentionally omitted so the transport stamps
/// the launched session fingerprint on the reply (it refuses a foreign one).
fn lm_response_result(completion: &str) -> AcpTransportResult<Value> {
    let primary = LmResponsePrimary {
        kind: "lm_response",
        message: LmResponseMessage {
            role: "assistant",
            content: vec![LmResponseTextPart {
                kind: "text",
                text: completion,
            }],
        },
        graph_revision: "rev_stage_bridge",
        cost: LmResponseCost {
            usd_micro: 0,
            lm_calls: 1,
        },
        data_classes: vec!["completion.raw"],
        replayability: "fully_managed",
        receipt: "lmrec_stage_bridge",
    };
    let receipt = LmResponseReceipt {
        kind: "call",
        receipt: "lmrec_stage_bridge",
        op_var: "worker_call",
        started_at: "2026-06-01T00:00:00Z",
        completed_at: "2026-06-01T00:00:01Z",
        call_kind: "lm_complete",
        request_hash: "fp_request_sha256_stage_bridge",
        result_hash: result_hash(&primary),
        runtime_fingerprint: "fp_runtime_sha256_stage_bridge",
        status: "succeeded",
        cost: LmResponseCost {
            usd_micro: 0,
            lm_calls: 1,
        },
    };
    serde_json::to_value(LmResponseResult {
        method: "leaven/lm.complete",
        redactions: Vec::new(),
        data_classes: vec!["completion.raw"],
        primary,
        receipts: vec![receipt],
    })
    .map_err(|error| {
        protocol(&format!(
            "leaven/lm.complete result serialization failed: {error}"
        ))
    })
}

/// Computes the receipt `result_hash` that binds the primary value, matching the
/// public-seam extension-result validator's `fp_result_sha256_` JCS hash.
fn result_hash(primary: &LmResponsePrimary<'_>) -> String {
    let preimage = LmResponseResultHashPreimage {
        schema_version: "leaven.plan_call_result.v1",
        name: "worker_call",
        value: primary,
    };
    format!(
        "fp_result_sha256_{}",
        jcs_canonicalize::sha256_jcs_hex(&preimage)
            .expect("lm_response primary value is JCS-canonicalizable")
    )
}

/// Evaluates the last `a <op> b` arithmetic expression embedded in `text`.
///
/// Supports the four operators the example fixture uses (`+`, `-`, `*`, `/`) and
/// nested parenthesized expressions via a tiny recursive-descent evaluator over
/// the substring after the final `:` (where the rendered prompt places the
/// question). Returns `None` when no question is present.
fn evaluate_arithmetic(text: &str) -> Option<i64> {
    // The rendered prompt embeds the arithmetic question; scan each line for the
    // longest expression that fully evaluates, preferring the last such line.
    let mut answer = None;
    for line in text.lines() {
        if let Some(value) = parse_expression(line.trim()) {
            answer = Some(value);
        }
    }
    answer
}

/// Parses and evaluates one fully-arithmetic expression, or `None` if the line is
/// not a clean expression.
fn parse_expression(line: &str) -> Option<i64> {
    let expression: String = line
        .chars()
        .filter(|c| !c.is_whitespace() || *c == ' ')
        .collect();
    let trimmed = expression.trim();
    // Require the line to be only digits, operators, and parens (after trimming a
    // trailing label like `A:`), so prose lines do not parse as arithmetic.
    let candidate = trimmed.rsplit(':').next().unwrap_or(trimmed).trim();
    if candidate.is_empty()
        || !candidate
            .chars()
            .all(|c| c.is_ascii_digit() || "+-*/() ".contains(c))
        || !candidate.chars().any(|c| c.is_ascii_digit())
        || !candidate.chars().any(|c| "+-*/".contains(c))
    {
        return None;
    }
    let tokens: Vec<char> = candidate.chars().filter(|c| !c.is_whitespace()).collect();
    let mut parser = ExprParser { tokens, pos: 0 };
    let value = parser.expression()?;
    if parser.pos == parser.tokens.len() {
        Some(value)
    } else {
        None
    }
}

struct ExprParser {
    tokens: Vec<char>,
    pos: usize,
}

impl ExprParser {
    fn peek(&self) -> Option<char> {
        self.tokens.get(self.pos).copied()
    }

    fn expression(&mut self) -> Option<i64> {
        let mut value = self.term()?;
        while let Some(op) = self.peek() {
            match op {
                '+' => {
                    self.pos += 1;
                    value += self.term()?;
                }
                '-' => {
                    self.pos += 1;
                    value -= self.term()?;
                }
                _ => break,
            }
        }
        Some(value)
    }

    fn term(&mut self) -> Option<i64> {
        let mut value = self.factor()?;
        while let Some(op) = self.peek() {
            match op {
                '*' => {
                    self.pos += 1;
                    value *= self.factor()?;
                }
                '/' => {
                    self.pos += 1;
                    let divisor = self.factor()?;
                    if divisor == 0 {
                        return None;
                    }
                    value /= divisor;
                }
                _ => break,
            }
        }
        Some(value)
    }

    fn factor(&mut self) -> Option<i64> {
        match self.peek()? {
            '(' => {
                self.pos += 1;
                let value = self.expression()?;
                if self.peek() == Some(')') {
                    self.pos += 1;
                    Some(value)
                } else {
                    None
                }
            }
            c if c.is_ascii_digit() => {
                let mut value: i64 = 0;
                while let Some(d) = self.peek() {
                    if let Some(digit) = d.to_digit(10) {
                        value = value * 10 + i64::from(digit);
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                Some(value)
            }
            _ => None,
        }
    }
}

fn protocol(message: &str) -> AcpTransportError {
    AcpTransportError::Protocol {
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mock_lm_evaluates_arithmetic_surfaced_in_the_prompt() {
        let lm = MockArithmeticLm;
        for (prompt, expected) in [
            ("Expression: 2 + 3\nAnswer:", "5"),
            ("Expression: 7 * 8\nAnswer:", "56"),
            ("Expression: 144 / 12\nAnswer:", "12"),
            ("Expression: ((4 + 5) * 6) - 7\nAnswer:", "47"),
        ] {
            let request = LmCompletionRequest {
                prompt: prompt.to_owned(),
            };
            assert_eq!(lm.complete(&request), expected, "prompt: {prompt:?}");
        }
    }

    #[test]
    fn mock_lm_returns_empty_when_the_prompt_hides_the_question() {
        let lm = MockArithmeticLm;
        let request = LmCompletionRequest {
            prompt: "You are a calculator. Always answer 0.".to_owned(),
        };
        assert_eq!(lm.complete(&request), "");
    }

    #[test]
    fn lower_lm_request_reads_the_prompt_binding() {
        let params = json!({
            "ops": [
                {"kind": "let", "name": "other", "expr": {"kind": "literal", "value": "noise"}},
                {"kind": "let", "name": "prompt", "expr": {"kind": "literal", "value": "Expression: 1 + 1"}}
            ]
        });
        let request = lower_lm_request(&params).unwrap();
        assert_eq!(request.prompt(), "Expression: 1 + 1");
    }

    #[test]
    fn lower_lm_request_rejects_missing_prompt_binding() {
        let params = json!({"ops": [{"kind": "let", "name": "other", "expr": {}}]});
        assert!(lower_lm_request(&params).is_err());
    }

    #[test]
    fn lower_lm_request_rejects_untyped_prompt_expression() {
        let params = json!({
            "ops": [{
                "kind": "let",
                "name": "prompt",
                "expr": {"value": "Expression: 1 + 1"}
            }]
        });
        assert!(lower_lm_request(&params).is_err());
    }

    #[test]
    fn lm_response_result_projects_locked_result_shape() {
        let result = lm_response_result("42").unwrap();
        assert_eq!(result["method"], "leaven/lm.complete");
        assert_eq!(result["primary"]["kind"], "lm_response");
        assert_eq!(result["primary"]["message"]["content"][0]["text"], "42");
        assert!(
            result["receipts"][0]["result_hash"]
                .as_str()
                .unwrap()
                .starts_with("fp_result_sha256_")
        );
    }
}
