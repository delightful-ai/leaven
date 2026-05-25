# Inspect AI Patterns: Decorator + Context + Async Orchestration

Vendored at `repos/inspect_ai/` as the closest Python design ancestor to Leaven's stage system.
Read this to understand the decorator shape, context threading, and evaluation loop that informed
`src/leaven/decorators.py`, `src/leaven/context.py`, and the composition story.

## 1. What to read first

These files establish the load-bearing surface:

- **`repos/inspect_ai/src/inspect_ai/solver/_solver.py`** (L115–265)
  Why: Decorator factory pattern. How `@solver` wraps user functions and patches state
  tracking. The `create_solver_wrapper` nested closure shows how registry metadata flows through
  instantiation.

- **`repos/inspect_ai/src/inspect_ai/scorer/_scorer.py`** (L88–199)
  Why: Scorer decorator—simpler than solver, shows how metrics bind at instantiation time
  via `registry_tag()`.

- **`repos/inspect_ai/src/inspect_ai/solver/_task_state.py`** (L139–418)
  Why: Context state object. Immutable sample data (`input`, `target`, `metadata`), mutable
  per-sample state (`messages`, `output`, `tools`, `completed`), and the `ContextVar`–based
  `sample_state()` getter for out-of-band access.

- **`repos/inspect_ai/src/inspect_ai/_eval/run.py`** (L69–150)
  Why: Top-level evaluation orchestration. Shows task iteration, solver/scorer resolution,
  async concurrency boundaries, and sample-to-evaluator threading.

- **`repos/inspect_ai/examples/scorer.py`** (full file)
  Why: End-to-end example. Decorator use, task/solver/scorer composition, and how the
  pieces fit together.

## 2. The decorator + registration pattern (load-bearing)

### Solver decorator (`_solver.py:156–265`)

The `@solver` decorator has **two layers**:

1. **Explicit name variant**: `@solver("my_name")` returns a wrapper function
2. **Bare variant**: `@solver` directly decorates the factory

Both paths hit `create_solver_wrapper()` (L187), which:

```python
def create_solver_wrapper(
    solver_type: Callable[P, SolverType],
    name: str | None = None
) -> Callable[P, Solver]:
    solver_name = registry_name(solver_type, name or "__name__")

    @wraps(solver_type)
    def solver_wrapper(*args: P.args, **kwargs: P.kwargs) -> Solver:
        solver = solver_type(*args, **kwargs)
        # ... convert Agent to Solver if needed
        
        # Patch state tracking into classes (L208):
        if inspect.isclass(type(solver)):
            original_call = solver.__call__
            @wraps(original_call)
            async def call_with_state(state, generate):
                state = await original_call(state, generate)
                set_sample_state(state)  # <-- ContextVar store
                return state
            setattr(registered_solver, "__call__", call_with_state)
        # ... or wrap functions (L226):
        else:
            @wraps(solver)
            async def registered_solver(state, generate):
                state = await solver(state, generate)
                set_sample_state(state)
                return state
        
        # Tag with registry metadata:
        registry_tag(solver_type, registered_solver,
                     RegistryInfo(type="solver", name=solver_name), ...)
        # Extract and store all user-provided args:
        named_params = extract_named_params(solver_type, True, *args, **kwargs)
        setattr(registered_solver, SOLVER_ALL_PARAMS_ATTR, named_params)
        
        return registered_solver
```

**Key moves**:
- Wraps both class-based and function solvers uniformly.
- Patches `__call__` for classes; wraps functions to preserve identity.
- **State tracking**: Every solver invocation calls `set_sample_state()` so downstream
  code (tools, model calls, etc.) can access via `sample_state()` ContextVar without
  plumbing it through signatures.
- **Registry binding**: Captures user-provided args as named params for later
  serialization/replay.

### Scorer decorator (`_scorer.py:129–199`)

Simpler—no state patching:

```python
@scorer(metrics=[accuracy(), stderr()])
def my_scorer() -> Scorer:
    async def score(state: TaskState, target: Target) -> Score:
        # ... compare state.output to target
        return Score(value=CORRECT, ...)
    return score
```

The `@scorer` decorator:
- Takes `metrics` upfront (unlike Leaven, which adds them later).
- Wraps the scorer factory, validates it's async, tags with registry info.
- Does **not** patch state because scorer runs after solvers complete.

### Discovery & instantiation

Both decorators call `registry_add()` to store metadata, then `solver_create(name, **kwargs)`
or `scorer_create(name, **kwargs)` looks them up at eval time:

```python
def solver_create(name: str, **kwargs: Any) -> Solver:
    return registry_create("solver", name, **kwargs)
```

Registry internals (`_util/registry.py`) maintain a global dict keyed by `(type, name)`.

## 3. TaskState + context injection pattern

### TaskState shape (`_task_state.py:139–418`)

Inspect's `TaskState` is a **single mutable object** passed through the solver chain:

```python
class TaskState:
    def __init__(self, model, sample_id, epoch, input, messages, target=None, ...):
        self._model = model
        self._sample_id = sample_id
        self._input = input  # Immutable sample input
        self._target = target  # Immutable target
        self._messages = ChatMessageList(messages)  # Mutable
        self._output = ModelOutput()  # Mutable
        self._completed = False  # Mutable
        self._store = Store({})  # Shared dict for solver collaboration
        self._scores = None  # Set by scorer
```

Each solver either:
- Returns the same `state` (modified in place), or
- Returns a new state (often via `state.copy()`—not shown but inferred).

Most solvers mutate in place: `state.messages.append(...)`, `state.output = ...`.

### Context injection

Solvers receive a `generate` function—a capability, not context. The `Generate` protocol
(L36–60) says:

```python
async def __call__(
    self,
    state: TaskState,
    tool_calls: Literal["loop", "single", "none"] = "loop",
    **kwargs: GenerateConfigArgs,
) -> TaskState:
    """Generate using the model and add the assistant message to state."""
```

So solvers call `state = await generate(state, ...)` to run the model.
The `generate` function is injected by the eval loop (see run.py).

**ContextVar for out-of-band access**: `sample_state()` and `set_sample_state()` (L448–456)
allow code deep in the model/tool/sandbox layers to read the current TaskState without
plumbing it through every function signature.

### Leaven comparison

| Aspect | Inspect | Leaven |
|--------|---------|--------|
| **State carrier** | Single `TaskState` object | Multiple contexts (`RunContext`, `StageContext`, `EvalContext`) |
| **Mutability** | In-place mutation of `messages`, `output` | Immutable stage payloads; mutations collected via `cx.case.update()` |
| **Cross-stage sharing** | `TaskState.store` dict (untyped) | `cx.workspace` + `cx.assessments` builders (typed) |
| **Out-of-band access** | `ContextVar` for `sample_state()` | Plumbed as `cx` parameter to all stages |
| **Target access** | `state.target` readable by all | `target` hidden from reflectors (enforced by seam) |

Leaven's design is more **structured**: contexts are stages-specific, and payloads are explicit
Pydantic models. Inspect is more **flexible**: TaskState is one mutable object and stores are
untyped dicts.

## 4. The async eval loop

### Orchestration sequence (`_eval/run.py:69–150`)

The `eval_run()` function:

1. **Task setup**: For each resolved task, ensure sample ids are unique (L98–108).
2. **Sandbox startup**: If any task uses sandboxes, initialize them (L112–117).
3. **Task batching**: Create `TaskRunOptions` for each task (L132–150).
4. **Parallel task execution**: Submit tasks to an `anyio.TaskGroup` for concurrent runs.

Each task is then run by `task_run()` (imported from `task.run`), which:

1. **Build the solver chain**: Resolve `task.setup` + `task.solver` into a `Plan` (L203–217).
2. **Create TaskState**: Initialize `TaskState` for each sample with model, dataset input, target (implied in `task_run`).
3. **Sample iteration**: For each sample in the dataset (likely concurrent up to `max_samples`):
   - Call `state = await plan.solve(state, generate)` where `plan` is a `Chain` of solvers.
   - Each solver in the chain calls `generate()` as needed and returns updated `state`.
4. **Scoring**: After solvers complete, `state = await scorer(state, target)` produces a `Score`.
5. **Result aggregation**: Collect `Score`s, apply reducers (e.g., "mean" across epochs).

### State threading

The eval loop injects the `generate` function into each solver:

```python
async def task_generate(state: TaskState, generate: Generate, ...) -> TaskState:
    # Internal orchestration of model calls, tool resolution, retries
    ...
```

Each solver invocation is wrapped in `solver_transcript()` (L85–87 in `_chain.py`), which
tracks transcript events for logging. The chain terminates early if `state.completed` is set.

### Concurrency model

- **Task-level parallelism**: Multiple tasks (or multiple samples across tasks) run in parallel
  via `anyio.TaskGroup`.
- **Within-task sample parallelism**: Implied by sampling strategy and `max_samples` config.
- **No within-solver parallelism**: Solver chains are sequential by design—each solver waits for
  the previous one to return.

## 5. Idioms worth stealing

### Nested decorator factory pattern

Inspect's `@solver` shows a battle-tested approach:

```python
@overload
def solver(name: str) -> Callable[[Callable[P, Solver]], Callable[P, Solver]]: ...

@overload
def solver(name: Callable[P, SolverType]) -> Callable[P, Solver]: ...

def solver(name: str | Callable[P, SolverType]) -> ...:
    if isinstance(name, str):
        def wrapper(solver_type: Callable[..., Solver]) -> Callable[..., Solver]:
            return create_solver_wrapper(solver_type, name)
        return wrapper
    else:
        return create_solver_wrapper(name)
```

Both `@solver` and `@solver("name")` work. Leaven's decorators do this already, so keep it.

**Snippet**: `repos/inspect_ai/src/inspect_ai/solver/_solver.py:156–165`

### Registry + instantiation arguments capture

The `extract_named_params()` call (L242) captures what the user passed to the decorator:

```python
named_params = extract_named_params(solver_type, True, *args, **kwargs)
setattr(registered_solver, SOLVER_ALL_PARAMS_ATTR, named_params)
```

This enables **deterministic replay**: the eval log can store the solver spec with arguments,
and a later run can reconstruct the exact same solver instance. Leaven should do this too.

**Snippet**: `repos/inspect_ai/src/inspect_ai/solver/_solver.py:242–243`

### ContextVar for state access

The `sample_state` ContextVar (L456) allows deep code paths (model APIs, sandboxes, tools) to
access the current TaskState without threading it through every function:

```python
_sample_state: ContextVar[TaskState] = ContextVar("sample_state")

def sample_state() -> TaskState | None:
    return _sample_state.get(None)

def set_sample_state(state: TaskState) -> None:
    _sample_state.set(state)
```

This is **especially useful** for tool implementations and model-call interceptors that don't
have direct access to the context. Leaven's Rust engine does not have a direct ContextVar
equivalent, but the Python boundary layer could use this pattern to share `RunContext` for
tool implementations.

**Snippet**: `repos/inspect_ai/src/inspect_ai/solver/_task_state.py:448–456`

### Metrics binding at decorator time

Inspect's `@scorer(metrics=[...])` captures metrics upfront:

```python
@scorer(metrics=[accuracy(), stderr()])
def my_scorer() -> Scorer: ...
```

This allows the framework to **validate metrics early** and to report schema at eval setup time
rather than at runtime. Leaven builds assessments in the evaluator or scorer, which is
more flexible but less statically visible.

**Snippet**: `repos/inspect_ai/src/inspect_ai/scorer/_scorer.py:129–160`

## 6. Anti-patterns / decisions Leaven made differently

### Rust + Python boundary

Inspect is **pure Python** async. Leaven is **Rust engine + Python worker**.

**Where Inspect's patterns don't fit**:
- Inspect's in-place `TaskState` mutations rely on Python's reference semantics and sync
  evaluation. Leaven's payloads must serialize across process/wire boundaries.
- Inspect's ContextVar for `sample_state()` works because the whole eval runs in one async
  context. Leaven's stages may run in separate processes, so context must be explicit (`cx`).
- Inspect's untyped `Store` dicts are fine for a single-threaded eval loop. Leaven's
  `workspace` builders are strongly typed because they cross wire boundaries and must validate
  on the Rust side.

**Leaven's choice**: Stage payloads are Pydantic models, not in-place mutations. The engine
composes stages via an ACP loop, not a sequential Python chain.

### Target visibility

Inspect's `TaskState.target` is readable by **all solvers and scorers**.

Leaven explicitly **hides `target` from reflectors**. The seam enforces this at the boundary.
This prevents reflectors from accidentally leaking the answer to the outside world.

**Snippet to compare**: `repos/inspect_ai/src/inspect_ai/solver/_task_state.py:401–404` (target
property) vs. Leaven `src/leaven/decorators.py:114–136` (reflector decorator forbids target
access in docstring).

### Async solver chains

Inspect solvers are **sync functions that return async callables**:

```python
@solver
def my_solver() -> Solver:  # Returns Solver (an async callable)
    async def solve(state: TaskState, generate: Generate) -> TaskState:
        ...
    return solve
```

Leaven stages are **directly async functions** decorated with `@lv.evaluator`, etc.

Inspect's indirection enables **composition and protocol polymorphism** (Chain can unroll nested
Chains). Leaven's direct async is simpler and aligns with Rust engine expectations.

### State completeness vs. streaming

Inspect's eval loop waits for all solvers to finish before scoring (see `task_run()` full loop).
Leaven's model allows **streaming results and incremental evaluation** via the optimizer loop.

## 7. Surprises (load-bearing non-obvious decisions)

### Solver wrapping preserves type

The `create_solver_wrapper()` function has two paths:

- **For classes** (L208): Patches `__call__` in place to preserve the class type.
- **For functions** (L226): Wraps with a new async function and uses `@wraps()`.

This is subtle: code that checks `isinstance(solver, Chain)` or `isinstance(solver, Plan)` must
work after wrapping. The class-patching approach ensures that.

**Significance for Leaven**: If users compose stages into classes (e.g., a custom `Optimizer`
class with `__call__`), the wrapping must preserve the type for composition logic. Leaven's
current decorators don't do this, but if custom compositions emerge, this pattern is worth
adopting.

**Snippet**: `repos/inspect_ai/src/inspect_ai/solver/_solver.py:208–232`

### Registry is global, not per-module

The registry (in `_util/registry.py`) is a global dict keyed by `(type, name)`. This means:

- Multiple solvers can have the same name if they differ in registry `type`. (A "solver" named
  "my_solver" and a "scorer" named "my_solver" coexist.)
- **Name collisions within a type are not prevented**. If two files define `@solver def
  my_solver()`, the second one silently overwrites the first.

Inspect handles this by prefixing names with module paths (see `registry_name()` logic), but
it's still a sharp edge.

**Leaven implication**: If Leaven uses registry for stage lookup, enforce unique stage ids
upfront or document the shadowing risk clearly.

**Snippet**: `repos/inspect_ai/src/inspect_ai/_util/registry.py` (not shown, inferred from use
in `_solver.py:190–191`)

## 8. Open questions

1. **Deterministic replay**: Inspect captures solver args via `extract_named_params()`. How does
   Leaven serialize the stage configuration for resume/replay? Are stage id + payload enough?

2. **Cross-sample state**: Inspect's `Store` is per-sample. Can solvers share state across
   samples? (Likely not by design, but the pattern suggests per-sample isolation.)

3. **Tool approval**: Inspect has `ApprovalPolicy` for tool use (see `_eval/eval.py:108`).
   How does Leaven handle tool approval at the Rust boundary?

4. **Metrics late-binding**: Inspect binds metrics at `@scorer` time. Leaven builds assessments
   in the evaluator/scorer. Can Leaven statically validate the assessment schema upfront?

5. **Out-of-band access patterns**: Leaven's reflectors and proposers run in different stages.
   If they need to share workspace state (e.g., a reflector writes to `cx.workspace`, and a
   proposer reads it), what's the durable boundary? (This is more about the ACP loop than the
   Python surface.)

---

**Last updated**: 2025-05-24  
**Confidence**: High (core patterns are stable; edge cases in registry and state sharing are
experimental in both systems).
