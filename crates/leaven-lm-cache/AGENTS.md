## Boundary
This crate owns Leaven's provider-neutral LM response cache: cache policy,
cache key, cache entry, cache-store trait, in-memory backend, and `CachedLm`.

It wraps `impl Lm`; it is not a provider crate and it is not the engine
evaluation cache.

## Map
- `LmCachePolicy` decides read/write behavior around an inner LM call.
- `LmCacheKey` is built from provider fingerprint plus canonical `LmRequest`
  content: model, messages, sampling, output mode, and provider hints.
- `LmContinuation` is intentionally not key material. This is only safe because
  `LmRequest.messages` remains the canonical full conversation.
- `LmCacheEntry` stores the provider-neutral response and original usage; a
  cache hit returns zero metered cost while preserving stored usage.
- `LmCacheStore` is the cache backend capability; concrete persistent stores can
  grow from this trait without changing provider crates.
- `CachedLm::id()` and `CachedLm::fingerprint()` delegate to the inner provider.
  Cache policy and backend are wrapper/runtime composition, not provider
  identity. Role-level resume identity belongs above this crate.

## Route Away
- Raw provider clients stay in `leaven-lm-openai`, `leaven-lm-anthropic`,
  `leaven-lm-local`, or future provider crates. Providers do not depend on this
  cache to be valid providers.
- Engine assessment/evaluation caching stays in `leaven-engine`; it deduplicates
  scored candidate evaluations, not raw LM responses.
- Provider-side prompt caching stays in the provider leaf. OpenAI prompt cache
  hints may affect `ProviderHints`, but this crate only decides whether Leaven
  skips the provider call.
- Ordinary solver/reflector/judge cache policy belongs in `leaven-run` or a
  runtime-role composition root. This crate may expose the advanced wrapper and
  store traits, but product examples should not require users to stack wrappers
  manually.

## Proof Anchors
- `crates/leaven-lm-cache/tests/cache_contract.rs` proves each cache policy,
  zero-cost cache hits, backend error lifting, and that continuation response
  IDs are ignored by cache keys.
- `docs/specs/lm_runtime_and_response_cache.md` is the local spec for key
  ingredients and response-cache boundaries.
- Run `cargo nextest run -p leaven-lm-cache` to prove response-cache behavior.
- If `LmRequest`, `ProviderHints`, `SamplingOptions`, or `OutputMode` changes,
  pair this with `cargo nextest run -p leaven-lm`; those types define the key
  material this crate serializes.

## Local Bait
- Do not include API keys, bearer tokens, wall-clock time, backend paths, or
  provider response IDs in `LmCacheKey`; they are transport or environment
  facts, not canonical response identity.
- Do not include `LmContinuation` in `LmCacheKey` as a quick fix for provider
  suffix bugs. Fix the provider lowering or canonical messages instead.
- `ProviderHints` currently participates in the key. That means OpenAI
  `prompt_cache_key`, `store`, and metadata are treated as behavior/routing
  inputs for Leaven response reuse.
- `CachedLm` is a reusable advanced wrapper, not the ordinary Layer 1 product
  story. Typical users should eventually configure cache policy through
  solver/reflector/judge runtime roles in `leaven-run` or the runtime
  composition root; keep the wrapper available without teaching wrapper
  stacking as the default user path.
- A cache hit returns zero `Metered` cost but leaves `LmResponse.usage` intact.
  Do not "normalize" usage to zero; usage is the original provider accounting,
  while cost is what this call spent.

## Decision Cards
- when: changing cache-key material
  do: update the key law in `docs/specs/lm_runtime_and_response_cache.md` and
    add/adjust a `cache_contract.rs` assertion that distinguishes hit from miss
  preserve: provider fingerprint plus canonical request content is enough to
    decide response reuse
  avoid: using cache backend paths, response IDs, continuation tokens, or clock
    time as identity
  verify: run `cargo nextest run -p leaven-lm-cache -p leaven-lm`

- when: changing cache policy behavior
  do: keep all four policy laws explicit: `Never`, `ReadWrite`, `ReadOnly`, and
    `Refresh`
  preserve: read misses call the inner LM where the policy says they should, and
    cache hits do not charge new cost
  avoid: silently making `ReadOnly` write, or making `Refresh` read before the
    provider call
  verify: run `cargo nextest run -p leaven-lm-cache`
