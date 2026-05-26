## Boundary
This crate owns OpenAI Responses API lowering for the provider-neutral `Lm`
trait.

It translates Leaven `LmRequest` values into OpenAI wire requests, maps OpenAI
responses and failures back into `LmResponse`/`LmError`, and computes an OpenAI
runtime fingerprint that excludes secrets.

## Map
- `OpenAiConfig` owns endpoint, API-key sourcing, request timeout, and retry
  policy. `from_env()` reads `OPENAI_API_KEY` and may read
  `LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS` for long-running operator calls; the
  effective timeout is fingerprinted. It does not currently own a model
  default; `LmRequest.model` is what lowers to the wire `model`.
- `OpenAiLm::to_wire_request` is the local seam for request lowering tests.
- OpenAI `previous_response_id` is used only when `LmContinuation` says the
  covered canonical messages line up safely.
- OpenAI prompt caching is provider-side prefix reuse. It may lower from
  `ProviderHints`, but it is not Leaven response caching.
- `OpenAiLm::fingerprint()` includes the adapter version marker and base URL,
  request timeout, and retry policy, and excludes API keys. It also currently
  excludes request model because model lives on `LmRequest`.
- Retry policy is local provider transport policy: bounded retries apply to
  transport errors and retryable OpenAI HTTP statuses, and numeric
  `Retry-After` seconds are honored up to the configured maximum backoff.
- Throttle policy is local provider concurrency policy: `OpenAiThrottlePolicy`
  bounds simultaneous provider calls with a semaphore and an optional acquire
  timeout. It is not a token bucket and does not estimate provider request or
  token quotas; retry/`Retry-After` still handles backpressure after a request
  reaches OpenAI.

## Route Away
- Provider-neutral request, response, sampling, continuation, usage, and error
  vocabulary belong in `leaven-lm`.
- Leaven response caching belongs in `leaven-lm-cache`; do not make this raw
  provider read or write cache stores.
- GEPA, engine graph, proposal parsing, and evaluation semantics belong in
  optimizer/engine/agentic crates. This adapter only returns an LM response.
- Anthropic, local, and mock behavior belong in their provider leaves; do not
  generalize OpenAI wire facts into `leaven-lm`.

## Proof Anchors
- `crates/leaven-lm-openai/tests/openai_mapping.rs` proves request lowering,
  continuation handling, sampling stop sequence lowering, prompt cache hint lowering, fingerprint behavior,
  env-var loading, timeout/retry policy identity, transport errors, retryable
  statuses, and response parsing against local fixtures.
- `docs/specs/lm_runtime_and_response_cache.md` section "OpenAI Provider
  Contract" owns the adapter contract.
- Run `cargo test -p leaven-lm-openai` to prove OpenAI mapping behavior,
  retry behavior, and provider-side concurrency throttling without live provider
  calls.
- The env test uses `OPENAI_API_KEY=test-key` in a child process; it proves
  environment loading only, not live credential validity or provider reachability.
- Mapping tests use `to_wire_request`, `parse_response`, and a local one-shot
  TCP server. Keep this non-network proof loop as the default gate.

## Local Bait
- Do not copy the OpenAI `previous_response_id` model into neutral cache keys.
  It is transport continuation, not canonical conversation identity.
- Requests own the model explicitly through `LmRequest.model`; `OpenAiLm::from_env()`
  only reads credentials and must not grow a model-looking argument unless the
  provider also stores and fingerprints a real default model.
- Do not add live OpenAI tests as the default proof for mapping changes; keep
  deterministic local wire/fixture tests as the cheap contract.
- Do not infer that OpenAI prompt caching equals Leaven response caching.
  `prompt_cache_key` can reduce provider-side prefix cost, but OpenAI still
  computes a fresh response; `leaven-lm-cache` is the only crate that may skip
  the provider call.
- System messages lower to `instructions`; user and assistant messages lower to
  `input`. If continuation slicing changes, preserve that canonical messages
  still contain the full conversation and that the OpenAI suffix starts at
  `covered_messages`.
- Non-OpenAI continuations are ignored, not translated. If another provider's
  continuation begins to work here, the neutral `ProviderName` boundary has been
  broken.

## Decision Cards
- when: changing `from_env` or model-default semantics
  do: keep `from_env()` credential-only, or store a real default with explicit
    fallback rules
  preserve: cache/replay identity changes when default model can affect output
  avoid: adding a model-looking argument for import ergonomics while requests
    still own the model
  verify: run `cargo test -p leaven-lm-openai`; add assertions that
    distinguish request model, default model, and fingerprint behavior

- when: changing OpenAI request lowering
  do: update `openai_mapping.rs` at the seam before adding higher-level product
    tests
  preserve: invalid OpenAI continuation is rejected before transport, other
    provider continuations are ignored, and provider hints lower only through
    neutral `ProviderHints`
  avoid: introducing live OpenAI calls into the default test path
  verify: run `cargo test -p leaven-lm-openai`

- when: changing OpenAI timeout, retry, or throttle behavior
  do: keep the policy in `OpenAiConfig`, include behavior-affecting fields in
    `OpenAiLm::fingerprint()`, and prove retry/non-retry/throttle behavior with
    local one-shot HTTP fixtures
  preserve: raw providers do not read/write Leaven response caches, provider
    errors remain structured `LmError` values after retries are exhausted, and
    proactive throttling limits in-flight provider calls before transport
  avoid: leaking OpenAI retry or throttle policy into `leaven-lm`, GEPA, engine,
    or cache crates
  verify: run `cargo test -p leaven-lm-openai`

- when: changing response parsing
  do: keep assistant text, provider response ID, continuation, and token usage
    mapping together in the same proof
  preserve: malformed/missing assistant output returns `LmError::InvalidResponse`
    and non-success HTTP status returns provider error
  avoid: treating refusals/tool calls as assistant text until the neutral output
    contract explicitly supports them
  verify: run `cargo test -p leaven-lm-openai`
