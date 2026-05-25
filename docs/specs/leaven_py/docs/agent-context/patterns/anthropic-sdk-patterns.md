# Anthropic SDK Patterns: Per-Repo Observation

**Reference:** vendored at `docs/specs/leaven_py/repos/anthropic-sdk-python/` (main branch)

**Relevance:** Direct reference for `lv.lm.anthropic` shape, request/response idioms, and typed retry/error contract.

---

## 1. What to Read First

- `src/anthropic/_client.py` (lines 145–290): Client construction, credential chain, sync/async split.
- `src/anthropic/_base_client.py` (lines 1009–1160): Request dispatch, retry loop, idempotency key injection.
- `src/anthropic/_exceptions.py`: Typed error hierarchy keyed to HTTP status codes.
- `src/anthropic/_response.py` (lines 49–100): `BaseAPIResponse` generic wrapper, response parsing.
- `src/anthropic/_streaming.py` (lines 45–80): `Stream[_T]` iteration contract over SSE events.
- `src/anthropic/lib/streaming/_messages.py` (lines 33–80): `MessageStream` stateful accumulation over text/content blocks.
- `src/anthropic/_models.py`: Pydantic v1/v2 dual support, custom validation hook machinery.
- `src/anthropic/_constants.py`: Retry defaults (2 max, 0.5s initial, 8s cap), timeout (10m total, 5s connect).

---

## 2. Client Construction Pattern

**Credential Chain (lines 174–276 of `_client.py`):**
Resolved in strict order: explicit args → env vars (ANTHROPIC_API_KEY/ANTHROPIC_AUTH_TOKEN) → profile lookup → workload-identity federation → active profile on disk. Static credentials shadow auto-discovery with one-shot logged warning. No credentials are passed in code; all via env or `credentials=` provider interface.

**Sync/Async Split:**
`Anthropic` extends `SyncAPIClient`; `AsyncAnthropic` extends `AsyncAPIClient`. Both inherit from `BaseClient` (lines 1–200 of `_base_client.py`), which isolates HTTP transport from call logic. Resource classes (`messages`, `models`, etc.) are late-bound via `@cached_property`.

**Config at Construction:**
```python
def __init__(
    self,
    *,
    api_key: str | None = None,
    timeout: float | Timeout | None | NotGiven = not_given,
    max_retries: int = DEFAULT_MAX_RETRIES,  # 2
    default_headers: Mapping[str, str] | None = None,
    http_client: httpx.Client | None = None,
    _strict_response_validation: bool = False,
    ...
) -> None:
```

**Retry Defaults (_constants.py, lines 9–14):**
- `DEFAULT_MAX_RETRIES = 2`
- `INITIAL_RETRY_DELAY = 0.5` (seconds)
- `MAX_RETRY_DELAY = 8.0`
- `DEFAULT_TIMEOUT = Timeout(timeout=600, connect=5.0)` (10 min total, 5 sec connect)

**Per-Request Override:**
Idempotency key is auto-generated (`stainless-python-retry-{uuid}`) and reused across retries (line 1051 of `_base_client.py`).

---

## 3. Typed Response Patterns

**Generic `APIResponse[R]` / `BaseAPIResponse[R]` (lines 49–80 of `_response.py`):**
Wraps httpx.Response. Tracks `_cast_to` type, `_parsed_by_type` cache, stream status, retries taken.
```python
class BaseAPIResponse(Generic[R]):
    _cast_to: type[R]
    _client: BaseClient[Any, Any]
    _parsed_by_type: dict[type[Any], Any]
    http_response: httpx.Response
    retries_taken: int
```

**Sync/Async Response Types:**
- `APIResponse[R]` (sync) → `.parse_as(type_)` lazily parses and caches per type.
- `AsyncAPIResponse[R]` (async) → async `.parse_as()`, respects early parse/late parse trades.

**Stream Response (lines 45–80 of `_streaming.py`):**
`Stream[_T]` wraps SSE decoder + response, yields items via `__stream__()` iterator. No buffering until consumed. Decoder is client-owned (`SSEBytesDecoder` or `SSEDecoder`), shared across concurrent streams (not thread-safe).

**MessageStream Decoration (lines 33–80 of `lib/streaming/_messages.py`):**
Higher-level wrapper over `Stream[RawMessageStreamEvent]`. Exposes `text_stream` iterator (text deltas only) and `__stream__()` for full `ParsedMessageStreamEvent`. Accumulates final `ParsedMessage[ResponseFormatT]` snapshot. Not itself a Pydantic model—pure iteration abstraction.

**Message Type (types/message.py, lines 17–60):**
Pydantic BaseModel. `content: List[ContentBlock]` (union of text, tool_use, image blocks). `id`, `model`, `stop_reason`, `usage` (explicit token counts).

---

## 4. Error Model

**Error Hierarchy (_exceptions.py, lines 25–144):**
- `AnthropicError` (base, catchable).
- `APIError` (lines 29–49): Wraps httpx.Request + message + body.
- `APIStatusError` (lines 65–84): 4xx/5xx. Parses `error.type` from body.
  - Status-specific subclasses: `BadRequestError` (400), `AuthenticationError` (401), `PermissionDeniedError` (403), `NotFoundError` (404), `ConflictError` (409), `UnprocessableEntityError` (422), `RateLimitError` (429), `ServiceUnavailableError` (503), `OverloadedError` (529), `DeadlineExceededError` (504), `InternalServerError` (5xx).
- `APIResponseValidationError`: Raised if parsed response fails Pydantic validation (when `_strict_response_validation=True`).
- `APIConnectionError`, `APITimeoutError` (httpx connection/timeout + request context).

**Retry Decision (_base_client.py, lines 804–837):**
- Respect `x-should-retry` header if present.
- Auto-retry on 408 (request timeout), 409 (lock timeout), 429 (rate limit), 5xx.
- Do not retry on 4xx (except 429, 409, 408).
- Do not retry on connection errors after all retries exhausted → raise `APIConnectionError`.
- Do not retry on timeout after all retries exhausted → raise `APITimeoutError`.

---

## 5. Patterns Worth Stealing for `lv.lm.anthropic` + `lv.builders.lm`

### 5.1 Credential Precedence Chain
The explicit `credentials=` interface allows injecting a custom provider (callable or object with `bind_base_url`). This enables:
- AWS IAM passing.
- Federated identity federation.
- Token refresh without client reconstruction.

**Leaven integration:** `AnthropicLm.api_key_env` (line 14 of `anthropic.py`) should be enriched to support `credentials=` provider path for multi-auth scenarios.

### 5.2 Request Options Immutability + Retry Idempotency
Lines 1050–1053 of `_base_client.py`: Deep copy options before retry loop, so mutations during parsing don't poison retries.
```python
input_options = model_copy(options)
if input_options.idempotency_key is None:
    input_options.idempotency_key = self._idempotency_key()
```
Auto-reuse idempotency key across retries, but generate once. Prevents duplicate-detection false positives on provider side.

**Leaven integration:** Any receipt-bearing call should preserve idempotency key in the receipt (`lv.CallReceipt`), enabling idempotent resume across engine restart.

### 5.3 Timeout Configuration as Explicit Constructor Param
Line 155 of `_client.py`: `timeout: float | Timeout | None | NotGiven = not_given`. Allows per-client or per-request override. Default is 10 min total + 5 sec connect.

**Leaven integration:** `AnthropicLm.timeout_s` (line 27 of `anthropic.py`) is already exposed; respect it in engine dispatch.

### 5.4 Pydantic v1/v2 Dual Shim via `_compat`
Lines 68–80 of `_models.py`: Helper functions `model_copy`, `model_dump`, `is_basemodel`, `construct_type` abstract Pydantic version differences. Allows single SDK codebase to support both major versions.

**Leaven integration:** Build equivalent shim for `lv.builders.lm.LmResponse` if dual-version support is required.

### 5.5 Async Iterator Pattern for Streaming
`Stream[_T]` and `AsyncStream[_T]` both implement `__iter__` / `__aiter__` + `__next__` / `__anext__`. Decoder is stateful per stream instance (not shared). Cleanup via `__exit__` / `__aexit__` closes httpx response.

**Leaven integration:** If Leaven needs streaming LM responses, inherit this pattern; don't implement custom buffering.

### 5.6 Retry Delay Jitter
Lines 800–802 of `_base_client.py`:
```python
jitter = 1 - 0.25 * random()
timeout = sleep_seconds * jitter
return timeout if timeout >= 0 else 0
```
Exponential backoff with ±25% jitter. Prevents thundering herd on rate limit.

---

## 6. Patterns We Should NOT Copy

### 6.1 Pydantic Discriminated Union Overloading
`_models.py` lines 200–300+ define custom Pydantic validators to handle discriminated unions with cyclic-reference resolution. Beautiful but SDK-specific. Leaven's simpler typed message shape doesn't require this.

### 6.2 `_BaseClient.request()` as Single Polymorphic Entrypoint
The base client's `request(cast_to, options)` method handles sync/async dispatch via runtime type checking (Generator vs AsyncIterator). Elegant but obscures flow. Leaven should keep sync and async paths explicitly separated in builder code.

### 6.3 Late Resource Binding via `@cached_property`
Lines 291–294 of `_client.py`:
```python
@cached_property
def messages(self) -> Messages:
    from .resources.messages import Messages
    return Messages(self)
```
Defers import of resource classes until first access. Good for SDK boot time, but adds indirection. Leaven should just construct once.

### 6.4 SSE Decoder as HTTP Response Wrapper
`_streaming.py` lines 74–80: The decoder consumes `response.iter_bytes()` in a generator. If the stream is abandoned early, the HTTP connection stays open until GC. Leaven should explicitly close (via context manager or explicit `.close()`).

---

## 7. What Would NOT Survive Adversarial Review

### 7.1 `_strict_response_validation=False` as Default
Lines 163–171 of `_client.py` mark it as "may be removed or changed in the future." Schema mismatch from API drift is silently ignored by default. Leaven should log or circuit-break on validation failure.

### 7.2 MessageStream Retains Raw Stream Response
Lines 56–61 of `lib/streaming/_messages.py`:
```python
@property
def response(self) -> httpx.Response:
    return self._raw_stream.response
```
Exposes low-level httpx.Response, including headers and status. Coupling to transport layer. Leaven should only surface status via receipt (one-way type).

### 7.3 Retry Loop Does Not Backpressure Callers
Lines 1059–1160 of `_base_client.py`: Retries happen inline, blocking the caller. No semaphore or queue. Under sustained rate limit, the thread pool thrashes. Leaven's call surface should offer explicit rate-limit feedback.

### 7.4 Credentials Provider Not Tested Against Closure
Lines 71–82 of `_client.py`:
```python
def _bind_credentials_base_url(credentials: AccessTokenProvider | None, base_url: str) -> None:
    bind = getattr(credentials, "bind_base_url", None)
    if callable(bind):
        bind(base_url)
```
Assumes provider's `bind_base_url` is idempotent and side-effect-free. If provider mutates shared state (e.g., cache invalidation), concurrent calls may conflict.

---

## 8. Surprises + Open Questions

### 8.1 Idempotency Key Format is Not User-Facing
Line 840 of `_base_client.py`: `return f"stainless-python-retry-{uuid.uuid4()}"`. The key is opaque to the application. Leaven should expose idempotency key in `CallReceipt` so caller can trace retries.

### 8.2 No Explicit Circuit Breaker for Rate Limit
The retry loop respects `Retry-After` header implicitly via sleep, but does not track cumulative rate-limit exposure across calls. If 429 repeats, the loop exhausts max_retries and raises. Caller must implement own backoff strategy.

### 8.3 Streaming Response Parsing is Lazy + Non-Atomic
In `lib/streaming/_messages.py`, the final `ParsedMessage` is only available after `.get_final_message()` (not shown here, but inferred from line 52). If parsing fails mid-stream, the stream is half-consumed and caller cannot retry. Leaven's receipt should mark whether a stream was fully consumed.

### 8.4 Pydantic Validation of Nested Types Happens at Parse Time
If the API returns a `Message` with a malformed `ContentBlock`, validation fails at `.parse_as(Message)`, not at stream construction. No streaming validation hook. Leaven should decide: fail fast on first bad block, or accumulate + report at end.

### 8.5 No Built-In Request Logging to Structured Format
Lines 1074–1124 of `_base_client.py` log via Python `logging` module (text). Leaven should emit structured logs (JSON) with request_id, model, prompt_tokens, and outcome for observability.

---

## Summary Table

| Aspect | Pattern | Leaven Implication |
|--------|---------|-------------------|
| **Credentials** | Chain: args → env → profile → federation → disk | Enrich `AnthropicLm` with `credentials=` provider interface |
| **Retry** | 2 default, 0.5s initial, 8s max, jitter ±25%, idempotency key reuse | Expose key + retries in `CallReceipt` |
| **Timeout** | 10m total, 5s connect, per-request override | Honor `timeout_s` in engine dispatch |
| **Response** | Generic `APIResponse[R]`, lazy parse-as-type, stream via SSE decoder | Implement equiv. generic wrapper for builder result |
| **Error** | Status-specific subclasses, auto-retry 408/429/5xx, no retry 4xx | Circuit-break or surface retry info in receipt |
| **Streaming** | `Stream[_T]` + `MessageStream` decorator, text iterator + event iterator | Use if Leaven adds streaming LM support; don't custom-buffer |
| **Anti-Pattern** | Retry loop blocking caller, validation off by default, response exposes httpx | Implement backpressure, strict validation, seal transport layer |

---

**Document Path:**
`/Users/darin/src/personal/leaven/docs/specs/leaven_py/docs/agent-context/patterns/anthropic-sdk-patterns.md`

**Last Updated:** 2026-05-24

**Scope:** Vendored SDK, main branch snapshot. Verify line numbers if SDK version changes.
