# Leaven LM Runtime And Response Cache

Status: implementation spec.
Date: 2026-05-10.

This spec defines the first real LM surface for Leaven optimizers. It is
subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/gepa_optimizer_surface.md`
- `docs/testing/README.md`

## 1. Product Goal

Leaven needs an LM boundary that optimizer code can use without knowing concrete
providers:

```rust
let lm = OpenAiLm::from_env()?;
let lm = CachedLm::read_write(lm, InMemoryLmCache::default());

let response = lm.complete(
    LmRequest::new("gpt-4.1-mini", Messages::from_user("solve this"))
        .with_sampling(SamplingOptions::default().with_max_output_tokens(512)),
).await?;
```

This must support GEPA reflection and future LM-program runners without pulling
OpenAI, Anthropic, local runtimes, or cache backends into optimizer crates.

## 2. Crate Graph

| Crate | Owns | Must not know |
| --- | --- | --- |
| `leaven-lm` | provider-neutral messages, requests, responses, usage, sampling, output mode, continuation, provider hints, `Lm` trait, `LmError` | response-cache stores, cache policies, HTTP clients, provider SDKs, GEPA, engine graph |
| `leaven-lm-cache` | Leaven response-cache policy, cache keys, cache entries, cache-store trait, in-memory cache backend, `CachedLm<M, C>` wrapper | concrete providers, engine evaluation cache, GEPA rhythm, provider SDKs |
| `leaven-lm-openai` | OpenAI Responses API lowering for the neutral `Lm` trait | GEPA, engine graph, cache backends |
| `leaven-lm-mock` | deterministic scripted/test LM implementation | concrete providers, cache backends |
| `leaven-gepa` | GEPA reflection components that consume `impl Lm` | concrete providers and cache stores |
| `leaven` | feature-gated import facade for `lm-cache`, `lm-openai`, `lm-anthropic` | implementation logic |

Allowed dependency edges:

```text
leaven-lm          -> leaven-kernel
leaven-lm-cache    -> leaven-kernel, leaven-lm
leaven-lm-openai   -> leaven-kernel, leaven-lm
leaven-lm-mock     -> leaven-kernel, leaven-lm
leaven-gepa        -> leaven-lm, not leaven-lm-cache or provider crates by default
```

The response cache is deliberately not part of `leaven-engine::EvaluationCache`.
The engine cache deduplicates scored candidate assessments. The LM response
cache memoizes provider-neutral LM completions before they become optimizer
proposals, feedback, or scores.

## 3. Public LM Contract

`leaven-lm` exposes one cold provider-neutral trait:

```rust
pub trait Lm: Send + Sync {
    fn id(&self) -> LmId;
    fn fingerprint(&self) -> Fingerprint;

    fn complete<'a>(
        &'a self,
        request: LmRequest,
    ) -> impl Future<Output = Result<Metered<LmResponse>, LmError>> + Send + 'a;
}
```

Trait laws:

1. `fingerprint()` must change when provider behavior that can affect output
   changes: base URL, model default, provider family, prompt lowering, output
   mode lowering, retry-relevant transport policy if it can alter results, or
   provider-specific default parameters.
2. `complete()` returns exactly one assistant message and the cost incurred to
   produce it.
3. Raw providers do not read or write the Leaven response cache. Caching is a
   wrapper concern owned by `leaven-lm-cache`.
4. `Metered<LmResponse>::cost` is the cost paid during this call. A cache hit
   returns zero cost even though `LmResponse::usage` still preserves the usage
   reported by the original provider call.
5. Provider crates own concrete network policy. Behavior-affecting timeout,
   retry, and proactive throttle policy must be reflected in the provider
   fingerprint; neutral `leaven-lm` request types do not learn
   provider-specific transport knobs.

## 4. Request And Response Types

`LmRequest` is the canonical semantic request:

```rust
pub struct LmRequest {
    pub model: ModelName,
    pub messages: Messages,
    pub sampling: SamplingOptions,
    pub output: OutputMode,
    pub continuation: Option<LmContinuation>,
    pub provider_hints: ProviderHints,
}
```

Required invariants:

1. `messages` is the canonical multi-turn conversation state. A caller must be
   able to reconstruct intended LM context from messages alone for text-only
   calls.
2. `continuation` is provider transport state, not canonical conversation truth.
   Providers may use it when safe and may ignore it when it would duplicate or
   lose context.
3. Cache identity ignores provider continuation tokens. Two requests with the
   same model, messages, sampling, output mode, provider hints, and provider
   fingerprint must share a response-cache key even if one carries an OpenAI
   `previous_response_id` and another does not.
4. `provider_hints` may carry transport hints such as OpenAI
   `prompt_cache_key`. Since hints can affect provider behavior or routing, the
   response-cache key includes them unless a later typed hint explicitly declares
   itself non-semantic.

`LmResponse` is the canonical response:

```rust
pub struct LmResponse {
    pub assistant: Message,
    pub continuation: Option<LmContinuation>,
    pub usage: TokenUsage,
    pub provider_response_id: Option<String>,
}
```

Required invariants:

1. `assistant.role` must be `Role::Assistant`.
2. `provider_response_id` is for diagnostics and follow-up continuation. It is
   never a cache key by itself.
3. `usage` preserves provider-reported token accounting. Cache hits preserve the
   stored usage but return zero metered cost.

`LmContinuation` stores provider state:

```rust
pub struct LmContinuation {
    pub provider: ProviderName,
    pub response_id: String,
    pub covered_messages: usize,
}
```

`covered_messages` is the number of canonical messages covered by the provider
state. A provider may send only `messages[covered_messages..]` with the
continuation when the provider protocol supports that safely.

## 5. Response Cache Contract

`leaven-lm-cache` exposes:

```rust
pub enum LmCachePolicy {
    Never,
    ReadWrite,
    ReadOnly,
    Refresh,
}

pub trait LmCacheStore: Send + Sync {
    fn get<'a>(
        &'a self,
        key: &'a LmCacheKey,
    ) -> impl Future<Output = Result<Option<LmCacheEntry>, LmCacheError>> + Send + 'a;

    fn put<'a>(
        &'a self,
        key: LmCacheKey,
        entry: LmCacheEntry,
    ) -> impl Future<Output = Result<(), LmCacheError>> + Send + 'a;
}

pub struct CachedLm<M, C> {
    inner: M,
    cache: C,
    policy: LmCachePolicy,
}
```

Policy behavior:

| Policy | Reads cache | Calls inner on miss | Writes result |
| --- | --- | --- | --- |
| `Never` | no | yes | no |
| `ReadWrite` | yes | yes | yes |
| `ReadOnly` | yes | yes | no |
| `Refresh` | no | yes | yes |

Cache key ingredients:

1. provider fingerprint from `Lm::fingerprint()`
2. model name
3. full canonical messages
4. sampling options
5. output mode/schema
6. provider hints

The key must not include API keys, bearer tokens, response IDs,
`previous_response_id`, wall-clock time, random process IDs, or backend-specific
cache paths.

## 6. OpenAI Provider Contract

`leaven-lm-openai` lowers `LmRequest` to the non-streaming OpenAI Responses API.
The current official OpenAI docs say GPT-5 series models work best with
Responses API, use `previous_response_id` for multi-turn state handling, and
track prompt cache hits via `usage.input_tokens_details.cached_tokens`.

Required implementation behavior:

1. `OpenAiLm::from_env()` reads `OPENAI_API_KEY`; request models live on `LmRequest.model`, not on the provider constructor.
2. The default endpoint is `https://api.openai.com/v1/responses`.
3. `messages` lower to Responses `input` items. A system message lowers to the
   request `instructions` string; user and assistant messages lower to input
   message items.
4. If `LmContinuation.provider == "openai"` and `covered_messages` is not beyond
   the request length, the provider sends `previous_response_id` and only the
   uncovered suffix as input.
5. `provider_hints.prompt_cache_key` lowers to `prompt_cache_key`.
6. The provider extracts the first completed assistant output-text message into
   `LmResponse::assistant`.
7. `TokenUsage` maps `input_tokens`, `cached_input_tokens`, `output_tokens`, and
   `reasoning_tokens`.
8. HTTP failures, non-success status codes, malformed responses, and completed
   responses without assistant text return structured `LmError` variants.
9. The provider uses bounded retries for transport failures and retryable HTTP
   statuses, honors numeric `Retry-After` seconds up to the configured maximum
   backoff, and applies a finite request timeout by default.
10. The provider applies a configured proactive in-process concurrency throttle
    before issuing Responses API calls. This throttle limits simultaneous
    provider calls; it is not a durable distributed quota manager or token-rate
    estimator.

OpenAI prompt caching is not the Leaven response cache. OpenAI prompt caching is
provider-side prefix reuse and still computes a fresh response. Leaven response
caching can skip the provider call entirely.

## 7. Verification

The implementation must include:

1. `leaven-lm` example/law tests for message construction, assistant response
   validation, and cost conversion from token usage.
2. `leaven-lm-cache` contract tests for every cache policy, deterministic key
   construction, and continuation-token exclusion from cache identity.
3. `leaven-lm-mock` tests proving scripted multi-turn responses consume calls in
   order and return metered usage.
4. `leaven-lm-openai` request/response mapping tests that do not require live
   credentials.
5. topology contract updates for the new crate and dependency edges.

Completion gate remains `just check`.
