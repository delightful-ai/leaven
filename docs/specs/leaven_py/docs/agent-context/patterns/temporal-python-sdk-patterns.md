# Temporal Python SDK: Patterns for Leaven

**Repo:** [temporalio/sdk-python@main](https://github.com/temporalio/sdk-python)  
**Context:** Vendored at `/Users/darin/src/personal/leaven/docs/specs/leaven_py/repos/temporal-python-sdk/`

Temporal is a production deterministic-replay workflow engine. Its Python SDK demonstrates how to bind a Rust core to Python decorators while preserving replayability. This is the gold standard for the Leaven engine + decorator pattern.

---

## 1. What to read first inside repos/temporal-python-sdk/

| File | Why |
|------|-----|
| `temporalio/workflow/_definition.py:56–250` | `@workflow.defn` and `@workflow.run` decorators. How they validate and attach markers to classes/methods, then `_Definition.from_class()` retrieves them. |
| `temporalio/activity.py:55–88` | `@activity.defn` decorator. Single-function wrapper; simpler than workflow because activities don't have signals/queries. Shows both decorator patterns. |
| `temporalio/worker/_workflow.py:60–80` | Worker class constructor. Accepts `workflows: Sequence[type]`, iterates them, and calls internal dispatch. Sets up the poll loop. |
| `temporalio/bridge/src/lib.rs:1–68` | PyO3 module init. Exposes `temporal_sdk_bridge` with `#[pymodule]`, `#[pyclass]`, `#[pyfunction]`. Exports `WorkerRef`, `RuntimeRef`, protocol messages. |
| `temporalio/bridge/src/worker.rs:33–100` | Rust-side `WorkerRef` and `WorkerConfig`. Shows type mapping across the boundary: Python dataclass ↔ Rust struct via `FromPyObject` / protobuf. |

These five files are the spine. You cannot understand Temporal's replay without `_definition.py`. You cannot understand the bridge without `lib.rs` + `worker.rs`.

---

## 2. The Rust bridge

**How Python reaches Rust: PyO3.**

The bridge is in `temporalio/bridge/` — a Rust crate compiled to a Python C extension module called `temporal_sdk_bridge`.

```python
# temporalio/bridge/src/lib.rs:12–17
#[pymodule]
fn temporal_sdk_bridge(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("RPCError", py.get_type::<client::RPCError>())?;
    m.add_class::<client::ClientRef>()?;
    m.add_function(wrap_pyfunction!(connect_client, m)?)?;
    // ... more exports
```

When Python imports `temporalio.bridge.temporal_sdk_bridge`, it gets Rust classes and functions. Example:

```python
# From temporalio/bridge/worker.py
class WorkerConfig:
    namespace: str
    task_queue: str
    versioning_strategy: WorkerVersioningStrategy
    # ... 60+ fields
```

The Python `WorkerConfig` is a `@dataclass`. When passed to `new_worker()`, it crosses into Rust as a `FromPyObject` struct via protobuf-serialized payloads:

```rust
// temporalio/bridge/src/worker.rs:43–66
#[derive(FromPyObject)]
pub struct WorkerConfig {
    namespace: String,
    task_queue: String,
    // ... mirrored fields
}
```

**Types crossing the boundary:**
- **Protobuf messages** (`temporalio::bridge::proto::workflow_activation`, `workflow_completion`): Serialized Python ↔ Rust.
- **Opaque refs** (`WorkerRef`, `ClientRef`, `RuntimeRef`): Hold Rust `Arc<Core>` handles, returned to Python as opaque objects.
- **Exceptions**: `PollShutdownError`, `RPCError` — raised in Python, defined in Rust via `pyo3::create_exception!`.

The bridge does NOT expose internal engine state. It exposes interfaces:
- Polling: `worker.poll(...)` → yields `WorkflowActivation` | `ActivityTask` (protobuf enums)
- Completion: `worker.complete(...)` ← takes `WorkflowActivationCompletion` | `ActivityTaskCompletion`
- This is the request/reply pattern, not shared state.

**Implication for Leaven:** The Python side never holds Rust state; it holds opaque refs and exchanges typed payloads. This is why receipts can be audit currency—they're minted on the Rust side and returned opaque to Python.

---

## 3. The decorator model

**Registration via marker attributes.**

```python
# temporalio/workflow/_definition.py:56–101
def defn(cls: ClassType | None = None, *, name: str | None = None, ...):
    def decorator(cls: ClassType) -> ClassType:
        _Definition._apply_to_class(cls, workflow_name=name or cls.__name__, ...)
        return cls
    if cls is not None:
        return decorator(cls)
    return decorator

def run(fn: CallableAsyncType) -> CallableAsyncType:
    if not inspect.iscoroutinefunction(fn):
        raise ValueError("Workflow run method must be an async function")
    setattr(fn, "__temporal_workflow_run", True)
    return fn
```

The `@workflow.defn` decorator:
1. Validates the class structure (one `@run` method, optional signals/queries).
2. Calls `_Definition._apply_to_class(cls, ...)` to construct and attach a `_Definition` object.
3. `_Definition` is stored as `cls.__temporal_workflow_definition`.

```python
# temporalio/workflow/_definition.py:220–236
@staticmethod
def from_class(cls: type) -> _Definition | None:
    defn = getattr(cls, "__temporal_workflow_definition", None)
    if defn and defn.cls == cls:
        return defn
    return None
```

**Dispatcher: Worker registration.**

```python
# temporalio/worker/_worker.py (simplified)
async with Worker(client, task_queue="default", workflows=[HelloWorkflow, MyOtherWorkflow]):
    # Worker constructor:
    # 1. For each class in workflows:
    for wf_cls in workflows:
        defn = workflow._Definition.must_from_class(wf_cls)
        # 2. Register by name:
        self._workflow_defs[defn.name] = (wf_cls, defn)
    # 3. When a WorkflowTask arrives with name="HelloWorkflow":
    # self._workflow_defs["HelloWorkflow"] is instantiated and run.
```

When the Rust core sends a `WorkflowActivation` (via `worker.poll()`), it includes the workflow type name. The Python worker looks up the name in its registry and instantiates the right class.

**Implication for Leaven:** Leaven decorators must build a `RegisteredStage` dataclass (not a marker attribute). The stage registry lives in `lv.optimize(..., runner=..., scorer=..., evaluator=...)`, not spread across decorated functions. This is simpler than Temporal—Leaven doesn't need introspection recovery because the optimizer knows the stages upfront.

---

## 4. Replay determinism — the load-bearing pattern

**Temporal's core claim: Replaying a workflow with the same inputs + history produces bit-for-bit identical results.**

The engine enforces determinism by:
1. **Sandbox isolation** (optional, default on): Workflows run in a restricted Python environment without `import datetime`, `import random`, `import os`. They must use `workflow.now()`, `workflow.new_random()`.
2. **Command recording**: Every external effect (activity call, sleep, signal) generates a history event. On replay, the sandbox feeds recorded events, not real ones.
3. **Non-determinism detection**: If a workflow produces a command not in history, it fails loudly.

```python
# temporalio/worker/workflow_sandbox.py (simplified idea)
# Replaying: feed history events to the workflow
history = [
    WorkflowExecutionStarted(...),
    WorkflowTaskScheduled(...),
    ActivityTaskScheduled(activity_id="fetch_data", ...),
    ActivityTaskCompleted(activity_id="fetch_data", result=cached_result),
    ...
]

# The workflow runs again. It calls activity("fetch_data", ...).
# The sandbox intercepts and returns cached_result instead of calling the real activity.
```

**User guarantees required:**
- No `import datetime` or `time.time()` in workflow code → use `workflow.now()`.
- No `import random` → use `workflow.new_random()`.
- No I/O outside activities.
- No `await asyncio.sleep(...)` → use `workflow.wait_until(workflow.now() + timedelta(...))`.

The Replayer class (`temporalio/worker/_replayer.py`) demonstrates this:

```python
# temporalio/worker/_replayer.py:33–54
class Replayer:
    def __init__(self, *, workflows: Sequence[type], ...):
        # Same config as Worker, but replays history instead of polling
        
    async def replay_workflow(self, history: History) -> WorkflowReplayResult:
        # Feed history into the workflow sandbox; capture the completion
```

**Implication for Leaven:** Leaven's receipt model mirrors this. Receipts are not logs—they are the audit record that, replayed, reproduces the assessment. A `CallReceipt` includes the input hash, output hash, and result. On replay, the engine re-invokes the scorer/runner with the same inputs, produces the same receipt, and validates it matches.

---

## 5. Patterns worth stealing for Leaven

### A. Decorator-as-marker-then-retrieve
**File:** `temporalio/workflow/_definition.py:56–250`

Pattern:
```python
@workflow.defn
class MyWorkflow:
    @workflow.run
    async def run(self, x: int) -> str:
        return f"result: {x}"

# Later, the worker retrieves it:
defn = workflow._Definition.from_class(MyWorkflow)
# Or from the run method:
defn = workflow._Definition.from_run_fn(MyWorkflow.run)
```

**Steal:** `@lv.runner`, `@lv.scorer`, etc. attach a `_marker` attribute to the function. At composition time, `lv.optimize(runner=my_runner, ...)` looks it up:
```python
defn = getattr(my_runner, "__leaven_stage_def", None)
```

This is simpler than Temporal's class-level marker because Leaven stages are functions, not classes. Applied in `leaven/decorators.py:69–86`.

### B. Opaque Rust refs + typed payloads
**File:** `temporalio/bridge/src/lib.rs`, `worker.rs`

Pattern: Never expose Rust state. Return opaque `PyObject` refs that the Rust-side `Arc` holds. Exchange only serialized payloads (protobuf).

**Steal:** Leaven receipts follow this exactly. A `CallReceipt` is `receipt_id: str` (opaque) + opaque handle. Python never constructs receipts; only the engine does. This prevents forgery and audit escape.

Applied in `leaven/_receipts.py:16–37`.

### C. Sandbox + determinism invariant
**File:** `temporalio/worker/workflow_sandbox.py`, `_replayer.py`

Pattern: All code runs in a restricted context (`SandboxedWorkflowRunner`). On first run, record effects. On replay, feed recorded effects, fail on divergence.

**Steal:** Leaven's replay model should enforce that a stage function produces the same `WriteReceipt` (hash, operation kind, timing) on replay. If the scorer produces a different score for the same input, that's a non-determinism error.

This is implicit in `leaven/result.py` but not yet operationalized. It should become explicit in the scorer/runner runtime contract.

### D. Exception mapping across the boundary
**File:** `temporalio/bridge/src/worker.rs`, `temporalio/exceptions.py`

Pattern: Define Rust exceptions via `pyo3::create_exception!`, then raise them in Python:
```rust
pyo3::create_exception!(temporal_sdk_bridge, PollShutdownError, PyException);
```

**Steal:** Leaven should define a few Rust exception types (e.g., `ReceiptMismatchError`, `NonDeterminismError`) and raise them from the engine when receipt validation fails. This makes error boundaries explicit in Python.

---

## 6. Patterns we should NOT copy

### A. Unbounded workflow lifetime + versioning
Temporal workflows can run for years, producing millions of history events. The SDK must support versioning (`@workflow.defn(versioning_behavior=...)`) so old code can coexist with new.

**Leaven doesn't need this.** Optimize loops are bounded (minutes to hours, fixed budget). There's no "update my optimizer in the middle of a run" use case. Version the Python decorator, re-run from checkpoint if needed—don't bake versioning into the stage interface.

### B. Signal/query interleaving
Temporal workflows can receive external signals (e.g., "pause this workflow") and answer queries (e.g., "what's the current progress?") during execution.

**Leaven doesn't need this.** Stages are atomic units of work. The engine drives them; they don't block on external events. If you need to pause a run, pause at the boundary (between assessments, proposals), not inside a stage.

### C. Child workflows + nested execution
Temporal workflows can start child workflows, creating a DAG of dependent runs.

**Leaven doesn't need this.** The engine orchestrates stages; stages don't call other stages. If you need nesting, that's the optimizer's job (via reflectors proposing new runs), not a language feature.

---

## 7. What would NOT survive adversarial review

### A. Sandbox escape surface
Temporal's sandbox (e.g., `SandboxedWorkflowRunner`) uses `RestrictedPython` and manual patching of builtins. This is a large surface for escape attacks.

**Risk for Leaven:** If stages use a sandbox (e.g., user code can't import `os`), the enforcement is only as good as the sandbox. Currently, there's no sandbox planned—stages are trusted Python. If that changes, the sandbox spec must be durable, not a footnote.

### B. Non-determinism detection is not foolproof
Temporal detects some non-determinism (command count mismatch) but not all (e.g., a Chaos Monkey that randomly does different things but still completes). The assumption is that users follow the discipline.

**Risk for Leaven:** Receipt validation (input hash + output hash) catches most non-determinism, but not all. If a scorer is buggy and sometimes returns different scores for the same input, replay will catch it. But if the scorer is intentionally randomized (e.g., Monte Carlo), receipts won't help. This is okay—it's a user contract, not an engine guarantee.

### C. PyO3 bindings are version-specific
The Rust bridge is built against a specific Python version. Distributing manylinux wheels (one per Python version) is necessary but costly.

**Risk for Leaven:** If Leaven uses PyO3, it inherits this burden. The `leaven_python.md` spec acknowledged this risk and chose in-house-ACP-over-PyO3. This is the right call. Don't use PyO3 for the engine; use typed JSON-RPC over stdio.

---

## 8. Surprises + open questions

### A. Marker attribute recovery is implicit
When you decorate a class with `@workflow.defn` and a method with `@workflow.run`, the decorator returns the unmodified class and method. The metadata is attached as a hidden attribute:
```python
setattr(fn, "__temporal_workflow_run", True)
```

This is not obvious. A reader might think the decorator is a no-op. **Leaven:** Make this explicit. Use `RegisteredStage` (a real dataclass, not hidden metadata) so the user knows they're composing a stage, not calling a plain function.

### B. Worker registration is implicit and depends on type equality
```python
Worker(client, workflows=[HelloWorkflow])
# Looks up HelloWorkflow by its __name__ in the protobuf message
```

This means you can't rename a workflow class without breaking old runs. **Leaven:** Use explicit stage IDs (`stage_id="my_runner"`, not inferred from `__name__`). This is already done in `leaven/decorators.py:83`.

### C. Sandbox isolation is opt-in
By default, workflows run sandboxed. But you can disable it:
```python
@workflow.defn(sandboxed=False)
```

If you disable the sandbox, the workflow can import `time` and call `time.time()`, breaking determinism. The burden is entirely on the user.

**Leaven:** Don't expose this option. Stage functions must be deterministic; don't let users opt out of the invariant.

### D. Activity results are cached naively
Temporal activities are cached by name + input hash. But there's no explicit TTL or invalidation. Long-running workflows can accumulate stale cache entries.

**Leaven:** Receipts are immutable, so cache invalidation doesn't apply. But batch operations (e.g., running 1000 assessments) could accumulate receipts. The spec should clarify receipt retention and GC.

---

## Appendix: File locations and line ranges

| Concept | File | Lines |
|---------|------|-------|
| `@workflow.defn` decorator | `temporalio/workflow/_definition.py` | 56–101 |
| `_Definition` class | `temporalio/workflow/_definition.py` | 205–250 |
| `@workflow.run` decorator | `temporalio/workflow/_definition.py` | 124–148 |
| `@activity.defn` decorator | `temporalio/activity.py` | 55–88 |
| Worker dispatch | `temporalio/worker/_worker.py` | 60–150 |
| PyO3 module init | `temporalio/bridge/src/lib.rs` | 1–68 |
| Rust `WorkerRef` | `temporalio/bridge/src/worker.rs` | 33–100 |
| Replayer | `temporalio/worker/_replayer.py` | 33–80 |
| Sandbox runner | `temporalio/worker/workflow_sandbox.py` | (see imports in `_workflow.py`) |

---

**Last reviewed:** 2026-05-24  
**Next review:** When Leaven stage runtime + receipt validation spec is drafted.
