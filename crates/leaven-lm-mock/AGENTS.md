## Boundary
This crate owns deterministic scripted `Lm` implementations for tests and
offline examples.

It is not a live provider template. Its behavior is intentionally local,
predictable, FIFO, and script-exhaustion driven so higher layers can test LM
consumers without network, credentials, or provider drift.

## Map
- `MockLmScript::then_text(...)` appends one assistant response plus token usage.
- `MockLm::complete(...)` ignores the request content and pops the next scripted
  step. It is good for consumer control flow, not prompt-sensitive branching.
- Cloned `MockLm` values share the same `Arc<Mutex<VecDeque<_>>>` script state.
  Clone it when the consumer should share a provider; create a new mock when the
  test needs independent scripts.
- `MockLm::fingerprint()` is computed from the initial script. Consuming calls
  does not mutate the fingerprint.
- `MockLm::default()` has an empty script and therefore errors on the first
  completion.

## Route Away
- Provider-neutral request, response, and error vocabulary belongs in
  `leaven-lm`.
- Leaven response caching belongs in `leaven-lm-cache`; do not teach the mock
  to mimic cache hits.
- OpenAI, Anthropic, local-server, or CLI/provider lowering belongs in the
  corresponding `leaven-lm-*` or `leaven-agent-*` provider leaf.
- GEPA reflection policy and agentic parsing belong in optimizer or agentic
  crates; the mock only returns scripted model responses.

## Proof Anchors
- `cargo nextest run -p leaven-lm-mock` proves scripted response order,
  exhaustion errors, stable identity, and fingerprint behavior.
- Consumer crates should still run their own tests with this mock wired in; a
  mock pass is not live-provider proof.
- If a consumer uses `MockLm` to prove caching, also run the consumer's cache
  test. This crate does not know whether a cache hit occurred; it only exposes
  deterministic call behavior and metered usage.

## Local Bait
- Do not copy `MockLm` into a new provider as the structural model for retries,
  auth, transport errors, or streaming. Real provider leaves must prove their
  own wire mapping and failure semantics.
- A deterministic script is useful for cache and optimizer tests, but it cannot
  prove prompt quality, provider continuation behavior, or live benchmark
  performance.
- Because requests are ignored, `MockLm` cannot prove model routing, prompt
  cache hints, continuation suffix safety, provider metadata, or output-schema
  lowering. Use provider mapping tests for those.
- Do not add cache-hit simulation here. Cache behavior belongs in
  `leaven-lm-cache`; this mock should continue to behave like a raw provider
  whose calls are visible through script consumption and token cost.

## Decision Cards
- when: using `MockLm` in another crate's test
  do: make the test name the consumer behavior being proved: role dispatch,
    parser behavior, cache-policy wiring, reflector input flow, or error
    handling
  preserve: mock success is lower-level proof, not live provider proof
  avoid: claiming OpenAI/Anthropic/local transport behavior from a FIFO script
  verify: run `cargo nextest run -p leaven-lm-mock` plus the consumer crate's
    focused test

- when: extending mock capabilities
  do: keep the fake capability explicit in the public name or script step
  preserve: deterministic fingerprints include every behavior-affecting scripted
    step
  avoid: growing a quasi-provider with retries, auth, network, or provider
    continuation semantics
  verify: run `cargo nextest run -p leaven-lm-mock` and any consumer tests that
    rely on the new fake behavior
