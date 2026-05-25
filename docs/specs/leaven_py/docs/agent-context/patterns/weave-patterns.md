# Weave @op() Decorator Patterns

**Status**: Pattern observation from vendored weave (wandb/weave@main, sparse-clone).  
**Target**: Inform Leaven's `@lv.scorer` / `@lv.runner` decorator design.

---

## 1. What to Read First

All paths relative to `repos/weave/weave/`:

- **`trace/op.py`** (1234 lines): The `@op()` decorator and call machinery. Defines the decorator (lines 1234–1403), call dispatch (lines 454–586), and tracing logic.
- **`flow/scorer.py`** (lines 30–55): Base `Scorer` class showing composition pattern.
- **`scorers/similarity_scorer.py`**: Concrete subclass using `@weave.op` on `score()` method.
- **`trace/op_protocol.py`**: The `Op` protocol and handler types (`OnInputHandlerType`, `OnOutputHandlerType`, `FinishCallbackType`).

---

## 2. THE @weave.op() Decorator (LOAD-BEARING)

### Decoration Time (`op.py:1234–1403`)

The decorator **does not wrap a class or method**; instead, it **replaces the function with a wrapper that holds metadata**:

```python
def op(
    func: Callable[P, R] | None = None,
    *,
    name: str | None = None,
    call_display_name: str | CallDisplayNameFunc | None = None,
    postprocess_inputs: PostprocessInputsFunc | None = None,
    postprocess_output: PostprocessOutputFunc | None = None,
    tracing_sample_rate: float = 1.0,
    enable_code_capture: bool = True,
    accumulator: Callable[[Any | None, Any], Any] | None = None,
    kind: OpKind | None = None,
    color: OpColor | None = None,
    eager_call_start: bool = False,
) -> Op[P, R]:
```

Key decoration actions (lines 1269–1396):
1. **Introspect** function type: async, sync, generator, async generator (line 1272–1274).
2. **Create a wrapper** (lines 1277–1312) that matches the function type, internally calling `_call_sync_func()`, `_call_async_func()`, or generator variants.
3. **Attach attributes** to the wrapper:
   - `wrapper.resolve_fn = func` (line 1315) — original function.
   - `wrapper.name` (line 1324) — op name.
   - `wrapper.postprocess_inputs`, `wrapper.postprocess_output` (lines 1327–1328).
   - `wrapper.call = partial(call, wrapper)` (line 1330) — `.call()` method for explicit invocation.
   - `wrapper.calls = partial(calls, wrapper)` (line 1331) — iterator over all calls.
   - Handler setters: `_set_on_input_handler`, `_set_on_output_handler`, `_set_on_finish_handler` (lines 1336–1343).
   - `wrapper._tracing_enabled`, `tracing_sample_rate`, `_accumulator` (lines 1346–1349).
   - `wrapper.__signature__`, cached annotations (lines 1373–1390).
4. **Return the wrapper** (line 1399) as the decorated op.

The wrapper **is not a class**; it's a function with protocol attributes.

### Call Time (`op.py:454–586` for sync; async at 588–712)

When the decorated function is **called normally** (not via `.call()`):

1. **Wrapper invokes** `_call_sync_func(wrapper, *args, **kwargs)` (line 1309–1311).
2. **Tracing disabled check** (line 469): if `is_tracing_setting_disabled()` or sampling is triggered, run bare and return `(res, placeholder_call)`.
3. **Weave dict setup** (line 480): collect `kind`, `color`, attributes into `__weave` dict via `setup_dunder_weave_dict()` (line 230–251).
4. **Create Call object** (line 488): `_create_call(op, *args, __weave=__weave, **kwargs)` (lines 345–402).
   - Binds args to signature via `_default_on_input_handler()` (lines 288–342).
   - Handles annotations: `@text` or `@image` annotations trigger `Content._from_guess()` (lines 314–317).
   - Applies default values (lines 192–206).
   - Redacts `api_key` (line 375–376).
   - Pushes call onto context stack (via `client.create_call()`, line 393).
5. **Execute function** (line 559): `res = func(*args, **kwargs)`.
6. **Output handling** (lines 574–583):
   - Call `on_output(res)` (line 574), which may invoke `_on_output_handler` or iterator accumulation.
   - If no handler and no accumulator, call `finish(output)` (line 555).
   - Finish pops the call from the stack (lines 583, 535–535).

**Trace for a simple op call**:
```
@op def add(a: int, b: int) -> int:
    return a + b

add(1, 2)
├─ wrapper(1, 2) invoked
├─ _call_sync_func(wrapper, 1, 2)
│  ├─ tracing check (enabled)
│  ├─ _create_call(wrapper, 1, 2)
│  │  ├─ sig.bind(1, 2) → {a: 1, b: 2}
│  │  ├─ client.create_call(...) → Call(id=X, inputs={...})
│  │  └─ call_context.push_call(call)
│  ├─ func(1, 2) → 3
│  ├─ on_output(3) → finish(3)
│  ├─ client.finish_call(call, output=3, exception=None)
│  ├─ call_context.pop_call(call.id)
│  └─ return (3, call)
└─ return 3
```

---

## 3. Scorer Composition Patterns

### Weave's `Scorer` Base (`flow/scorer.py:30–55`)

```python
class Scorer(Object):
    column_map: dict[str, str] | None = Field(...)

    def model_post_init(self, context: Any, /) -> None:
        super().model_post_init(context)
        _validate_scorer_signature(self)
        score_fn = getattr(self.score, "__func__", self.score)
        if is_op(score_fn) and score_fn.kind is None:
            score_fn.kind = "scorer"

    @op
    def score(self, *, output: Any, **kwargs: Any) -> Any:
        raise NotImplementedError

    @op
    def summarize(self, score_rows: list) -> dict | None:
        return auto_summarize(score_rows)
```

**Key pattern**:
- `Scorer` is a Pydantic `Object` (weave's serializable base).
- The `score()` method is **decorated with `@op`** (line 49), making it traceable.
- Subclasses override `score()` and optionally `summarize()`.
- All subclass instances inherit tracing automatically.

### Example: `EmbeddingSimilarityScorer` (`scorers/similarity_scorer.py:14–69`)

```python
class EmbeddingSimilarityScorer(LLMScorer):
    model_id: str = OPENAI_DEFAULT_EMBEDDING_MODEL
    threshold: float = Field(...)

    @weave.op
    async def score(self, *, output: str, target: str, **kwargs: Any) -> Any:
        embeddings = await self._aembedding(self.model_id, [output, target])
        return self._cosine_similarity(embeddings.data[0]["embedding"], 
                                       embeddings.data[1]["embedding"])

    async def _compute_embeddings(self, output: str, target: str) -> tuple[...]:
        # Helper, NOT decorated
        ...
```

**Pattern**:
- Only the public `score()` is `@op`-decorated (line 34).
- Helpers remain undecorated.
- Async is transparent: decorator handles it (lines 1272, 1278–1285).

---

## 4. Patterns Worth Stealing for Leaven

### 4a. Lazy Signature Introspection with Deferred Parsing

**Location**: `op.py:89–93`, 304–334, 1373–1394.

**Pattern**: Cache the signature at decoration time, but defer annotation parsing if introspection fails:
```python
PARSE_DEFERRED: Any = object()

try:
    cached_sig = inspect.signature(func)
    wrapper_any._weave_cached_parsed_input_annotations = parse_from_signature(cached_sig)
except (TypeError, ValueError):
    wrapper_any._weave_cached_parsed_input_annotations = PARSE_DEFERRED
```

At call time, retry parsing if needed (lines 305–306). **Steal**: Avoids brittle decoration-time failures; graceful fallback.

### 4b. Stacked Handler Chain: Input → Output → Finish

**Location**: `op_protocol.py:132–135`.

**Pattern**: Define three orthogonal handlers:
```python
OnInputHandlerType = Callable[["Op", tuple, dict], ProcessedInputs | None]
OnOutputHandlerType = Callable[[Any, FinishCallbackType, dict], Any]
OnFinishHandlerType = Callable[["Call", Any, BaseException | None], None]
```

Each can be **independently set** (lines 254–269) without blocking others. Used by integrations (OpenAI streaming, etc.) to intercept at specific points. **Steal**: Composable hook points; cleaner than monolithic postprocessing.

### 4c. Accumulator for Streaming Ops

**Location**: `op.py:224`, 1243, 546–551, 862–868.

**Pattern**: Optional accumulator function for generators:
```python
@op(accumulator=lambda acc, val: (acc or []) + [val])
def stream_op():
    for i in range(10):
        yield i
```

The accumulator is called on each yielded value and can signal early termination via `StopIteration`. **Steal**: Declarative folding of streaming outputs; the op records the final accumulated state, not raw generator.

### 4d. Call Display Name Customization

**Location**: `op.py:1238, 1355–1361, 397–398`.

**Pattern**: Allow display name to be static or computed:
```python
@op(call_display_name=lambda call: f"rank_{call.inputs['rank_method']}")
def rank_candidates(rank_method: str):
    ...
```

At call time (line 398), use runtime call-time name if provided, else fall back to function name. **Steal**: Better UI tracing without boilerplate; computed names encode call context.

### 4e. Sampling and Tracing Gating

**Location**: `op.py:420–427, 469–478`.

**Pattern**: Root calls can be sampled; child calls inherit parent's trace:
```python
def _should_sample_traces(op: Op) -> bool:
    if call_context.get_current_call():
        return False  # Don't sample traces for child calls
    if random.random() > op.tracing_sample_rate:
        return True  # Sample traces for this call
    return False
```

Child calls are **always traced if parent is**, avoiding broken chains. **Steal**: Probabilistic tracing without breaking observability; efficient for high-volume ops.

---

## 5. Patterns We Should NOT Copy

### 5a. W&B-Hosted Trace Server Coupling

**Location**: `op.py:365, 393, 503–525; trace_server/*.py`.

**Problem**: The call `weave_client_context.require_weave_client()` (line 365) assumes a W&B-connected client. The `client.create_call()` (line 393) and `client.finish_call()` (line 525) push to a remote server immediately. **What happens locally**:
- No client → tracing silently disabled (line 408–409).
- Client required at call time, not decoration time (line 365).
- Calls are serialized and sent over the wire to W&B's trace server.

**For Leaven**: Keep tracing **fully local** at decoration time or make the backend pluggable. Don't couple the decorator to a specific trace service.

### 5b. Code Capture via `art.path_contents`

**Location**: `op.py:1406–1420; get_captured_code()`.

**Problem**: The decorator captures function source into a W&B artifact (`art.path_contents["obj.py"]`) at publish time. **This is W&B-specific**. **For Leaven**: Don't auto-capture code unless explicitly requested; code is metadata, not a tracing contract.

### 5c. Imperative `.call()` Method Overloading Call-Time Params

**Location**: `op.py:1164–1207`.

**Pattern**: The `.call()` method allows passing `__weave` dict to override display names and attributes per invocation. **Problem**: This is a private override mechanism; users should not regularly inspect `__weave`. **For Leaven**: If you need call-time overrides, use a public `RunContext` dict, not hidden `__weave` kwargs.

---

## 6. What Would NOT Survive Adversarial Review

### 6a. Sentinel Cache for SDK Default Markers

**Location**: `op.py:127–189`.

**Concern**: The code maintains a cache of sentinel values from OpenAI, Cohere, Anthropic SDKs to detect "not provided" markers and skip them. This is **fragile**:
- Hardcoded import paths (line 128–135).
- Assumes packages remain in `sys.modules` (line 158).
- Sentinel identity is cached by Python object identity, not value equality.

**Risk**: If an SDK changes its sentinel, the cache silently misses it. If a package is imported after decoration, the cache cannot be invalidated. **For Leaven**: Don't cache import-time sentinels; introspect at call time if needed, or require explicit `Optional[T] = None` signatures.

### 6b. Call Stack Context as Global State

**Location**: `op.py:44, 109–110, 384–385, 486, 535`.

**Concern**: Calls are pushed/popped on a **thread-local stack** via `call_context._call_stack`. The decorator relies on this for parent-child nesting. **Fragile if**:
- Code is run in a thread pool without proper context propagation.
- Async code mixes with sync code without careful context management.
- The stack is manually corrupted (mitigated by `_restore_call_stack()`, line 102–110, but still a risk).

**For Leaven**: Document context propagation as a requirement, or use async-native context (`contextvars`).

### 6c. Tight Coupling Between Decorator and Protocol

**Location**: `op.py:1269–1396; op_protocol.py`.

**Concern**: The decorator hardcodes attributes into the wrapper to satisfy the `Op` protocol. If the protocol changes, the decorator must change. There's no abstraction boundary. **For Leaven**: Define the interface as a clear contract first; allow multiple decorator implementations.

---

## 7. Surprises + Open Questions

### 7a. Generator Wrapping Complexity

**Location**: `op.py:715–933` (sync), `936–1161` (async).

**Surprise**: Generator functions are **not called at decoration time**; instead, the wrapper returns a **custom wrapper generator** that manages the call context per iteration. The code manually pushes/pops the call stack around each yield (lines 812, 856, 882, 895) to ensure nested generators see the correct parent. This is **extremely detailed** and suggests generators are a hard case for tracing.

**Q**: Does Leaven need to support streaming scorer outputs? If so, plan for similar complexity.

### 7b. On-Finish Handler Doesn't Receive the Call

**Location**: `op.py:266–269; op_protocol.py:135`.

**Concern**: `OnFinishHandlerType` is `Callable[["Call", Any, BaseException | None], None]` but the handler is **not called by the core decorator**. Instead, integrations manually define subclasses and set `_on_finish_handler`. The `finish()` function (line 506–535) calls `client.finish_call()`, not the handler. **Why?** Unclear. **For Leaven**: Make it explicit when handlers run and what they receive.

### 7c. Tracing Disabled Silently, No Error

**Location**: `op.py:407–413`.

**Pattern**: If no client is available, tracing is **silently disabled** with a one-time log warning. This means ops run correctly but are never recorded. **Risk**: A user might think tracing is working when it isn't. **For Leaven**: Either fail fast (require explicit init) or provide a way to verify tracing is active.

---

## Summary: Top 3 Patterns + 1 Anti-pattern + 1 Review Concern

| Pattern | Value | Location |
|---------|-------|----------|
| **Lazy Signature Introspection** | Defer annotation parsing; graceful fallback avoids decoration-time failures. | `op.py:1373–1394, 305–306` |
| **Stacked Handler Chain** | Three orthogonal hooks (input, output, finish) instead of one postprocessor. Composable by integrations. | `op_protocol.py:132–135, op.py:254–269` |
| **Accumulator for Streaming** | Declarative fold of generator outputs; op records final state, not raw iterator. | `op.py:224, 1243, 546–551` |
| **Anti-pattern: Server Coupling** | Tracing is coupled to W&B client. For Leaven, keep backend pluggable or local. | `op.py:365, 393, 503–525` |
| **Review Concern: Generator Complexity** | Generator wrapping manually manages call context per iteration (250+ lines). Hard case; document risk. | `op.py:715–933` |

**File**: `/Users/darin/src/personal/leaven/docs/specs/leaven_py/docs/agent-context/patterns/weave-patterns.md`
