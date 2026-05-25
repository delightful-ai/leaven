# DSPy Vendoring Patterns — Leaven Python Scaffold

**Date:** 2026-05-24  
**Scope:** `repos/dspy/` for `lv.x.dspy.LeavenDSPyLM` and `lv.builders.lm` seam design.

---

## 1. What to Read First

| File | Why It Matters |
|------|---|
| `repos/dspy/dspy/clients/base_lm.py` | Defines `BaseLM` interface contract: `forward(prompt, messages, **kwargs)` return shape, exception handling, history tracking, state serialization. **Mandatory.** |
| `repos/dspy/dspy/clients/lm.py:36-206` | Concrete `LM` subclass: kwargs merging, model-family branching (reasoning models require special handling), caching decorator, retry logic, async variant. Shows how to wrap a provider. |
| `repos/dspy/dspy/predict/predict.py:246-271` | `Predict.forward/aforward`: how modules invoke the LM, pre/post-processing, config merging, temperature hacks for multi-generation. Shows consumer-side expectations. |
| `repos/dspy/dspy/dsp/utils/settings.py:165-239` | Configuration model: `dspy.configure(lm=..., adapter=...)` for global defaults, `dspy.context()` for temporary overrides. Leaven's context seam must integrate here. |
| `repos/dspy/dspy/utils/exceptions.py:5-22` | `ContextWindowExceededError`: the only error subclasses are obligated to re-raise. Fallback behavior depends on catching this type. **Do not invent new exception types.** |

---

## 2. Idioms Worth Stealing

### A. Kwargs Merging and Config Precedence

**What DSPy does:**  
`LM.forward()` merges `self.kwargs` (instance defaults) with per-call `kwargs` (call-site overrides), giving call-site values priority. Example: `lm = dspy.LM(model=..., temperature=0.7)` + `predict(..., temperature=1.0)` → final temperature is 1.0.

**Code excerpt** (repos/dspy/dspy/clients/lm.py:180):
```python
kwargs = {**self.kwargs, **kwargs}
```

**Why it matters for Leaven:**  
Our `LeavenDSPyLM` and `lv.lm.LmBuilder` must adopt the same precedence: instance config < call-site config. This is user-facing API stability.

**Applies to:** `src/leaven/x/dspy/lm.py` (constructor stores defaults) and `src/leaven/builders/lm.py` (complete method merges model, temperature, max_tokens).

---

### B. Model-Family-Specific Parameter Branching

**What DSPy does:**  
The `LM` class detects reasoning models (o1, o3, gpt-5 family) via regex and enforces different parameter names (`max_completion_tokens` instead of `max_tokens`, `temperature=1.0` required). This prevents silent failures downstream.

**Code excerpt** (repos/dspy/dspy/clients/lm.py:104-115):
```python
model_pattern = re.match(
    r"^(?:o[1345](?:-(?:mini|nano|pro))?(?:-\d{4}-\d{2}-\d{2})?|gpt-5(?!-chat)(?:-.*)?)$",
    model_family,
)
if model_pattern:
    if (temperature and temperature != 1.0) or (max_tokens and max_tokens < 16000):
        raise ValueError(...)
    initial_kwargs = dict(temperature=temperature, max_completion_tokens=max_tokens, **kwargs)
```

**Why it matters for Leaven:**  
Anthropic and other providers may have similar quirks. Leaven's `lv.builders.lm` should validate and normalize model-specific constraints **at config time**, not at request time.

**Applies to:** `src/leaven/builders/lm.py` (the `complete` method should catch unsupported param combos early).

---

### C. Cache Bypass via Rollout ID + Unique Kwargs

**What DSPy does:**  
To break cache without changing the prompt, pass `rollout_id=<unique int>`. This is stripped from the provider request but affects cache key. Useful for sampling multiple trajectories.

**Code excerpt** (repos/dspy/dspy/clients/lm.py:182-183):
```python
if kwargs.get("rollout_id") is None:
    kwargs.pop("rollout_id", None)
```

**Why it matters for Leaven:**  
If Leaven implements caching (via `default_cache_storage.md`), adopt the same `rollout_id` contract for cache-bypass without prompt modification.

**Applies to:** `src/leaven/builders/lm.py` (reserve the rollout_id kwarg for the seam, never send to provider).

---

### D. History Tracking with Callback Hooks

**What DSPy does:**  
`BaseLM` wraps `forward()` and `aforward()` with `@with_callbacks` decorator, logs to both per-LM history and global history, and includes metadata (timestamp, uuid, model, cost, usage). Modules and users can inspect via `lm.inspect_history(n=10)`.

**Code excerpt** (repos/dspy/dspy/clients/base_lm.py:191-201):
```python
@with_callbacks
def __call__(self, prompt=None, messages=None, **kwargs):
    response = self.forward(prompt=prompt, messages=messages, **kwargs)
    outputs = self._process_lm_response(response, prompt, messages, **kwargs)
    return outputs
```

**Why it matters for Leaven:**  
Logging and observability are baked in at the `BaseLM` level, not bolted on per-consumer. Leaven's seam should emit receipts at the `LmBuilder.complete` level (not lower), and the engine should drive cost/token tracking.

**Applies to:** `src/leaven/builders/lm.py::LmResponse::receipt` is the Leaven analogue.

---

## 3. Anti-Patterns / Failure Modes to Avoid

### A. Accepting Anything in `forward()` Response

**Problem:**  
DSPy documents that `forward()` must return OpenAI-compatible chat/completion/response shape, but the code only validates shape in `_process_completion()` after the fact. A buggy adapter could return arbitrary dicts; the error appears three layers down.

**Lesson for Leaven:**  
Validate the response shape in `forward()` immediately after the provider call, not in a generic post-processor. Fail early with a clear "response did not match contract" error, not "failed to find field X".

**Applies to:** `src/leaven/x/dspy/lm.py::forward` should validate the lifted response is structurally `LmResponse`-compatible before returning.

---

### B. Silently Eating Config Errors

**Problem:**  
DSPy's `Predict._forward_preprocess()` logs warnings for missing/extra fields but does not raise. Typos in signature field names result in incomplete outputs, not early errors. The user sees partial results and blames the LM.

**Lesson for Leaven:**  
Config and request validation errors should raise, not warn. Warnings are for recoverable degradation (e.g., truncation due to max_tokens); config bugs should fail fast.

**Applies to:** `src/leaven/builders/lm.py::complete` should raise if `messages` and `prompt` are both provided, or if unknown kwargs are passed.

---

### C. History Bloat Without Limits

**Problem:**  
`BaseLM` caps history at `MAX_HISTORY_SIZE=10_000` entries per instance, but the global history has the same limit. In a long-running evaluator, global history can OOM if not capped per-module or truncated.

**Lesson for Leaven:**  
History should be optional (users can disable) and ring-buffered with a configurable limit. Do not assume callers want unbounded logs.

**Applies to:** `src/leaven/builders/lm.py` (optional receipt emit, or pushed to the engine/context level).

---

## 4. The `BaseLM.forward` Contract (Precise)

### Signature
```python
def forward(
    self,
    prompt: str | None = None,
    messages: list[dict[str, Any]] | None = None,
    **kwargs
) -> <OpenAI-compatible response object>
```

### Parameters
- **`prompt`**: Optional single string. If provided and `messages` is None, adapter converts it to `[{"role": "user", "content": prompt}]`.
- **`messages`**: Optional list of dicts with keys `role` (str) and `content` (str), optionally `tool_call_id` (str). One or both of `prompt` and `messages` must be provided (not both None).
- **`**kwargs`**: Provider-specific args (temperature, max_tokens, top_p, stop, logprobs, etc.). Merged with `self.kwargs` by caller; subclass just passes through to provider.

### Return Value (One of Three Formats)

**Chat Completion** (most common):
```python
{
    "choices": [
        {
            "message": {"content": "response text", ...},
            "finish_reason": "stop" | "length" | "tool_calls" | ...,
            "logprobs": <obj> or None,
            ...
        },
        ...
    ],
    "usage": {"prompt_tokens": N, "completion_tokens": N, "total_tokens": N},
    "model": "model-identifier",
    ...
}
```

**Text Completion** (deprecated, not recommended):
```python
{
    "choices": [
        {"text": "response text", "finish_reason": "stop", ...},
        ...
    ],
    "usage": {...},
    ...
}
```

**Response API** (new, for function-calling):
```python
{
    "output": [
        {"type": "message", "content": [{"text": "..."}]},
        {"type": "function_call", ...},
        ...
    ],
    ...
}
```

### Exception Contract

**Mandatory:**  
- Catch the provider's "context window exceeded" error and re-raise as `dspy.ContextWindowExceededError(model=self.model)`. Example: `repos/dspy/dspy/clients/lm.py:200-202`.
- Do NOT invent new exception types. Fallback behavior depends on catching only this one.

**Optional (but recommended):**
- Let other provider errors (auth, rate limit, network) propagate as-is or wrap in a generic `RuntimeError`. Callers will retry at the `dspy.Predict` level if configured.

### Optional Methods to Override

1. **`aforward(prompt, messages, **kwargs)`**: Async variant. Return type identical to `forward()`. If not implemented, `Predict.acall()` cannot be used.

2. **`dump_state() -> dict[str, Any]`**: Return a dict of all constructor args (minus API keys) for serialization. Base implementation handles `model`, `model_type`, `cache`, `num_retries`, `kwargs`.

3. **`load_state(state: dict[str, Any], **kwargs) -> BaseLM`**: Class method to reconstruct from `dump_state()`. Base implementation calls `cls(**state)`.

4. **`copy(**kwargs) -> BaseLM`**: Return a shallow copy with updated attributes. Used by DSPy's optimizer to create variants. Base implementation handles history and callbacks isolation.

5. **`supports_function_calling`, `supports_reasoning`, `supports_response_schema` (properties)**: Return bool. Signals to adapters what features the model has. Defaults to False.

6. **`supported_params (property)`**: Return `set[str]` of OpenAI-style param names the model accepts (e.g., `{"response_format", "temperature"}`). Adapters use this to validate before calling.

### Guarantees from Caller

- `__call__` (sync or async) wraps `forward()` / `aforward()` and handles history tracking, cost aggregation, and callbacks. Do not implement `__call__`.
- Config merging (`self.kwargs` + method `kwargs`) happens before `forward()` is called.
- Retries happen at the `dspy.LM` or caller level, not in `forward()` itself (though you may implement retries inside).

---

## 5. Surprises / Non-Obvious Load-Bearing Details

### A. The `model_type` Parameter Is Essential

`BaseLM.__init__` accepts `model_type: str` (default `"chat"`). DSPy uses this to dispatch response processing:
- `"chat"` → expects `response.choices[].message.content` (modern OpenAI format).
- `"text"` → expects `response.choices[].text` (legacy completion format).
- `"responses"` → expects `response.output[]` (new Response API format).

**In `LeavenDSPyLM.__init__`:** Store the mode and pass it to the parent. If the Leaven seam always produces chat-format responses, hardcode `model_type="chat"` in the parent init call.

**Code reference:** `repos/dspy/dspy/clients/base_lm.py:156-162`.

---

### B. `_process_completion()` Unpacks Nested Message Objects

DSPy expects `choice.message.content` (object attribute access) OR `choice["text"]` (dict key access), depending on model_type. If you return a dict with `{"message": {"content": "text"}}`, it works. If you return a raw string, it fails.

**In `LeavenDSPyLM.forward()`:** Ensure the lifted response object has `.message` as an attribute (not a dict key). Use `type("Response", (), {"message": type("Message", (), {"content": "..."})})` or a namedtuple, or just return a dict with the right structure.

**Code reference:** `repos/dspy/dspy/clients/base_lm.py:410-427`.

---

### C. Callbacks Are Run *Around* `forward()`, Not Inside

The `@with_callbacks` decorator on `__call__()` fires callbacks before and after `forward()`, not inside it. If you override `__call__()` (which you shouldn't), you lose callback instrumentation.

**For Leaven:** Do not override `__call__()` in `LeavenDSPyLM`. The base class handles it. Just implement `forward()` and optionally `aforward()`.

---

### D. `rollout_id` Is a Cache-Bypass Signal, Not a Provider Param

The `rollout_id` kwarg is used to differentiate cache keys but is stripped from the provider request. If you pass `rollout_id=5` to the LM, it will never reach the provider; it only affects cache lookup.

**For Leaven:** If we implement caching, reserve `rollout_id` in `lv.builders.lm` and use it in the cache key, but do not pass it to the underlying provider completion call.

---

### E. Demos Are Embedded in the Prompt, Not a Provider Param

DSPy's `Predict` module formats demos (few-shot examples) into the message content itself, not as a separate `examples` parameter. The adapter handles prompt construction; `forward()` only sees the final messages list.

**For Leaven:** This is invisible at the `BaseLM` level—demos are baked into the messages before `forward()` is called. No special handling needed.

---

## 6. Open Questions for Implementation

1. **How does Leaven handle model routing?** DSPy assumes a single configured LM (or context override). Does Leaven allow `cx.lm.complete(model="claude-3-opus")` to select a different model at call time, or is the model fixed per stage/context?

2. **Should `LeavenDSPyLM` expose the receipt object?** DSPy modules do not see receipts; they only see the completion text. Does Leaven want module-level access to cost, token usage, or cache metadata?

3. **What is the async story for Leaven's seam?** `BaseLM.aforward()` is optional but strongly recommended for async stages. Does `lv.builders.lm.complete()` already async, or will we need to add it?

4. **Error handling scope:** Should `LeavenDSPyLM` only catch and re-raise `ContextWindowExceededError`, or should it translate provider-specific errors (auth, rate limit) into Leaven's error types?

5. **History / receipt emit point:** Should `LeavenDSPyLM` populate the receipt in its `forward()` method, or should the engine do it at the stage-call level? (Base DSPy does it in `__call__`, which wraps `forward()`.)

---

## Summary

- **Top idiom #1 (kwargs merging):** Instance defaults + call-site overrides with call-site precedence. Copy this contract to `lv.builders.lm`.
- **Top idiom #2 (model-family branching):** Validate constraint-breaking config early (temperature, max_tokens). DSPy does it in `_get_initial_kwargs`; Leaven should do it in `complete()`.
- **Top idiom #3 (history + callbacks):** Wrap the actual provider call with instrumentation that the caller (Predict module) cannot see. DSPy does it in `__call__`; Leaven does it at the engine / context level.
- **Top anti-pattern #1 (response validation):** Validate response shape immediately after the provider call, not in post-processing. Fail hard with "response contract violated", not "field X not found".
