## Boundary
This crate owns provider-neutral language-model vocabulary: `Lm`, `LmRequest`,
`Messages`, text/tool content parts, tool definitions, model roles, sampling,
output mode, continuation, usage, and `LmError`.

It is the contract optimizer and adapter code depend on when they need "an LM"
without depending on OpenAI, Anthropic, local servers, cache stores, engine
graph state, or GEPA rhythm.

## Map
- Conversation truth is `Messages`; it preserves system/developer/user/
  assistant/tool roles, text parts, tool-result parts, tool-call ids, and
  optional provider-visible names. Provider continuation tokens are optional
  transport hints in `LmContinuation`.
- `LmRequest.model` is explicit request identity today. `LmRequest.model_role`
  preserves public-seam policy/routing intent alongside that concrete model;
  it is not an ambient default model and must not be silently substituted for
  provider wire model identity.
- Provider-shaping but provider-neutral knobs live in `ProviderHints`,
  `SamplingOptions`, `LmTool`, and `OutputMode`.
- `TokenUsage` preserves provider accounting; `Metered<LmResponse>` carries the
  cost charged for the call.
- `Lm::fingerprint()` is cache/replay identity for behavior-affecting runtime
  choices. It must not include secrets, and it is not a substitute for the
  request model; `leaven-lm-cache` combines provider fingerprint with request
  content.

## Route Away
- Response-cache policy, key construction, cache stores, and cache-hit zero-cost
  behavior belong in `leaven-lm-cache`.
- OpenAI, Anthropic, local, and mock request lowering belong in `leaven-lm-*`
  provider crates. Do not add provider SDK, HTTP, or env-var handling here.
- Engine evaluation cache belongs in `leaven-engine`; this crate only describes
  LM calls before they become proposals, feedback, or assessments.
- GEPA reflection policy belongs in `leaven-gepa`; GEPA may consume `impl Lm`,
  but `leaven-lm` must not learn GEPA vocabulary.

## Proof Anchors
- `crates/leaven-lm/tests/lm_contract.rs` proves message ordering, developer/
  tool role and content-part preservation, assistant response validation,
  request defaults, model-role preservation, tool definitions, provider hints,
  sampling stop sequences, identifier conversions, token-cost mapping, and
  public error shapes.
- `docs/specs/lm_runtime_and_response_cache.md` owns the LM/cache/provider
  split; use it before changing request, response, continuation, or fingerprint
  semantics.
- Run `cargo test -p leaven-lm` to prove this neutral vocabulary still
  satisfies its crate-level contract.
- If a change affects `LmRequest` key material, also run
  `cargo test -p leaven-lm-cache`; that is where cache-key ingredients
  and continuation exclusion are proved.
- If a change affects `ProviderHints`, run
  `cargo test -p leaven-lm-openai` too, because OpenAI currently lowers
  prompt-cache keys, storage hints, and metadata through that neutral bag.

## Local Bait
- `docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md` defines
  the legacy-named Leaven worker-profile `lm.complete` extension method that
  external-language workers call. This crate's `Lm` trait and request/response
  vocabulary are the durable source those payloads lower from; the
  worker-facing wire bridge itself is not implemented here.
- `ProviderHints` is not permission to add provider-specific wire structs here.
  Add typed neutral hints only when multiple providers can ignore or translate
  them without importing provider APIs.
- `ProviderHints` is response-cache key material. Adding or reclassifying a hint
  is a cache-identity decision, not just a provider-lowering convenience. If a
  hint should not affect response reuse, add an explicit law and cache-key test
  instead of relying on omission by accident.
- `LmContinuation.response_id` is not a response-cache key. Cache identity must
  be built in `leaven-lm-cache` from canonical request content and provider
  fingerprint, not provider response IDs.
- `LmContinuation.covered_messages` counts a canonical `Messages` prefix. Do
  not delete or reorder canonical messages to satisfy a provider suffix API;
  providers can send less wire input only when the neutral messages still retain
  the whole intended conversation.

## Decision Cards
- when: adding a new request field
  do: decide whether it is canonical semantic input, provider transport hint, or
    private provider state before adding it
  preserve: cache keys can be rebuilt from provider fingerprint plus canonical
    request material without provider response IDs
  avoid: storing OpenAI/Anthropic wire structs or SDK enums in this neutral crate
  verify: run `cargo test -p leaven-lm -p leaven-lm-cache`; add the
    provider mapping test only in the provider crate that lowers the field

- when: changing model/default semantics
  do: keep `LmRequest.model` as the source of truth unless a spec adds an
    explicit neutral optional-model/default-model contract
  preserve: provider fingerprints describe runtime/provider behavior, while
    request model remains cache-key material
  avoid: constructor arguments that look like defaults but are ignored by
    providers
  verify: run `cargo test -p leaven-lm` plus each provider crate whose
    constructor or fingerprint semantics changed
