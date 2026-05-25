# BAML Architectural Patterns: Leaven Python Cross-Read

**Author**: Agent | **Date**: 2026-05-24 | **Scope**: Vendored BAML@canary vs Leaven Python scaffold

BAML is our closest architectural peer: Rust engine + per-language typed SDKs + schema-driven codegen. This file observes what works, what doesn't, and what is load-bearing on BAML's specific constraints that wouldn't apply to Leaven.

---

## 1. What to read first in repos/baml/

| File | Why |
|------|-----|
| `engine/language_client_python/src/lib.rs:56-100` | PyO3 module initialization; how BAML exports Rust classes to Python. |
| `engine/language_client_python/python_src/baml_py/ctx_manager.py:33-189` | Context-var + threading model for Rust state. Directly applicable to `StageContext` lifecycle. |
| `engine/generators/languages/python/src/lib.rs:47-80` | Codegen entry point: IR → Pydantic pipeline. Precedent for `leaven-types` JSON Schema → generated Python. |
| `engine/generators/languages/python/src/ir_to_py/classes.rs:20-95` | Class→Pydantic lowering; template-driven (Askama). |
| `integ-tests/python/baml_example_app.py:8-33` | Async idiom: decorated functions calling generated code. Mirrors `@lv.runner` / `@lv.scorer`. |

---

## 2. The Rust→Python boundary

**Transport**: PyO3 with stable ABI (not cffi).

**Key shape** (`Cargo.toml:41-48`):
```toml
pyo3 = { version = "0.23.3", features = [
  "abi3-py38",  # Stable ABI across Python 3.8+
  "extension-module", "generate-import-lib",
] }
```

**Serialization flow**: Python async call → PyO3 method → Rust `extract::<T>()` → tokio work → return Pydantic model or typed exception.

**Context threading** (`ctx_manager.py:60-70`): `ContextVar` holds thread-local `RuntimeContextManager`. Async tasks clone context before spawning (line 118-120, `start_trace_async`).

---

## 3. The codegen pipeline

**Entry**: `engine/generators/languages/python/src/lib.rs:47-80` (`generate_sdk_files`).

**Trace**: BAML DSL → compiler IR → `IntermediateRepr` → Python codegen.

**Class codegen** (`ir_to_py/classes.rs:20-95`): IR lowered to `ClassPy` struct, rendered via Askama templates into Pydantic `BaseModel` + validators + docstrings.

**Layout**: `baml_client/__init__.py` (hand-written) + `types.py` (generated) + `async_client.py` (hand-written wrapper calling PyO3).

---

## 4. Patterns worth stealing for Leaven

### Pattern A: PyO3 stable ABI
Single wheel works across Python 3.8–3.13+. Zero version skew. Essential for FFI.

**Implication**: Leaven exports more classes than BAML (`Context`, `Budget`, `Receipt`, etc.). Pattern scales.

### Pattern B: contextvars for thread-local context
Clean async propagation without manual parameter passing.

**Concrete** (`ctx_manager.py:60-70`):
```python
self.ctx = contextvars.ContextVar("baml_ctx", default={})
```

**Caveat**: BAML's per-thread dict works for tokio; Leaven may use simpler per-task model.

### Pattern C: Askama templates for codegen
Type-safe IR→code lowering. Separates data from rendering.

**Direct application**: JSON Schema → Pydantic codegen via templates.

### Pattern D: Async decorator with exception preservation
Tracing + transparency without breaking async semantics.

**Key** (`ctx_manager.py:141-167`): `except BaseException` (not `Exception`) captures `KeyboardInterrupt`, `CancelledError`.

**For Leaven**: Decide: swallow exceptions → error receipt, or propagate + log?

### Pattern E: Pydantic + streaming result types
`FunctionResult` (success) or exception; streaming is separate class wrapping partials.

**Not in scope yet**, but pattern is sound for future streaming stages.

---

## 5. Patterns we should NOT copy

### Anti-pattern A: Global singleton context manager
BAML's `prev_ctx_manager` (lines 44-56, `ctx_manager.py`). First instance becomes global.

**Why not**: Leaven must support multiple optimizations in one process (Jupyter, test suites). Use explicit handle passing instead.

### Anti-pattern B: Tight coupling to DSL + codegen
BAML requires `baml-cli generate` before using the SDK. You cannot `import baml` without generated code.

**Why not**: Leaven is not a DSL. Types come from external JSON Schema. Users should `import leaven as lv` without a build step.

**What to steal**: Codegen shape (Askama, IR lowering) without the requirement.

### Anti-pattern C: Re-exporting generated types at top level
BAML has `from baml_client import ExtractResume` (generated type at top level) alongside `baml_client.types.ExtractResume`.

**Why not**: Leaven already hides submodule names (`__init__.py` lines 108-116). Keep it clean.

---

## 6. What would NOT survive adversarial review

### Issue A: Exception type proliferation
BAML has 8+ types: `BamlError`, `BamlValidationError`, `BamlClientHttpError`, `BamlTimeoutError`, etc.

**Problem**: Users don't know what to do when catching `BamlValidationError`. Reviewer pushback: "Error surface is too granular."

**For Leaven**: Narrow hierarchy: `LeavenError` → `LeavenEngineError`, `LeavenValidationError`, `LeavenTimeoutError` (3–4 types). Document recovery.

### Issue B: contextvars in concurrent-optimization scenario
BAML stores per-thread dict in `ContextVar`. If two Leaven optimizations run in same thread (different asyncio tasks), they share context.

**Problem**: Thread dict approach breaks with task-level concurrency.

**For Leaven**: Bind `RunContext` to `RunHandle`, not global contextvars. Explicit parameter passing.

---

## 7. Surprises (load-bearing decisions)

### Surprise A: Feature-flag runtime selection
BAML has two runtimes (THIR interpreter vs VM bytecode). You choose at compile time.

**Implication**: Can't swap runtimes at runtime. Leaven should decide early: feature flag or runtime enum?

### Surprise B: Deep context cloning for async isolation
BAML overwrites contextvars with clone when starting async trace (`ctx_manager.py:111-120`). Traces do not share context state.

**For Leaven**: Formalize: "Proposers run independently, do not mutate shared graph." Deep cloning makes this auditable.

### Surprise C: Pickle support for process serialization
`CtxManager` is pickleable; Rust state survives via PyO3.

**For Leaven**: Test whether `RunHandle` is pickleable. Document if it's not.

---

## 8. Open questions

1. How does BAML handle capability tokens in Python layer? (Not visible in this read.)
2. What is streaming backpressure model? Does Rust block if Python consumer is slow?
3. Why separate `Collector` and `FunctionLog`? (`src/lib.rs:79-81`)
4. How do BAML v1 and v2 generated `__init__.py` differ?
5. Can users define custom types in Python via `TypeBuilder`, or codegen only?

---

## Summary

**Top 3 patterns to steal**:
1. PyO3 stable ABI (`abi3-py38`) — zero version skew.
2. contextvars for thread-local context — clean async propagation.
3. Askama templates for codegen — type-safe IR→code.

**Top anti-pattern**: Global singleton context manager. Use explicit handle passing.

**Top adversarial blocker**: Exception type explosion. Document recovery semantics or flatten hierarchy.

**Top surprise**: Deep context cloning for async isolation. Formalize assumptions about concurrent traces.

---

**Next steps**: Update `/Users/darin/src/personal/leaven/docs/specs/leaven_py/src/leaven/context.py` and `decorators.py` (stubs) to sketch contextvars + async decorator patterns. This document provides architectural precedent.

