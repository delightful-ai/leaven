# Marvin Pattern Observations for Leaven

**Source**: Vendored Marvin (PrefectHQ/marvin@main) at `/Users/darin/src/personal/leaven/docs/specs/leaven_py/repos/marvin/`

**Scope**: Ergonomic surface design — what "even higher than typed core" looks like, and which patterns are worth stealing vs. which demand too much magic.

---

## 1. What to Read First Inside Marvin

- **`src/marvin/__init__.py`** (L1–84): top-level surface inventory — 6 structured-output fns + core Task/Agent/Thread/Memory/Team abstractions
- **`src/marvin/fns/`**: each function (`classify.py`, `extract.py`, `generate.py`, `run.py`) follows the same overload pattern (sync wrapper → async impl → type validation)
- **`src/marvin/agents/agent.py`** (L49–100): `Agent` dataclass with optional model, tools, memories, personality
- **`src/marvin/tasks/task.py`** (L75–100): `Task` generic container for instructions + result_type + context
- **`src/marvin/thread.py`** (L26–32): `Thread` context var for sharing conversation state across runs
- **`examples/hello_run.py`, `hello_classify.py`, `hello_extract.py`**: minimal working sketches (5–20 lines each)

---

## 2. The Top-Level User Surface

### Simplest Marvin Program

```python
import marvin

# 5-line sketch: untyped task
poem = marvin.run("Write a haiku about coding")
print(poem)

# 8-line sketch: typed output
answer = marvin.run("the answer to the universe", result_type=int)
print(answer)  # 42

# 10-line sketch: structured extraction
result = marvin.extract(
    "i found $30 on the ground and bought 5 bagels for $10",
    int,
    instructions="only USD"
)
print(result)  # [30, 10]
```

### Leaven's Smallest Sketch (20 lines)

```python
import leaven as lv

@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.Case, cx: lv.RunContext):
    response = await cx.lm.complete(prompt=prompt.template.format(**case.input))
    return response.text.strip()

@lv.scorer
async def score(output: str, case: lv.Case, cx: lv.RunContext):
    return lv.Score.exact_match(output, case.target["answer"])

result = await lv.optimize(
    seed=lv.PromptArtifact(template="Answer: {question}\nA:"),
    train=lv.cases.from_jsonl("train.jsonl"),
    val=lv.cases.from_jsonl("val.jsonl"),
    optimizer=lv.optimizers.gepa(population_size=8),
    runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    runner=run, scorer=score,
).run()
```

**Difference**: Marvin assumes a single-call task; Leaven is inherently multi-stage. Marvin's 5-liner works for "call an LLM now." Leaven's 20-liner works for "optimize this system end-to-end with audit + replay."

---

## 3. Ergonomic Patterns Worth Stealing

### Pattern 1: Overloaded fn-level API with Sync/Async Parity

**Location**: `src/marvin/fns/classify.py` (L24–51), `extract.py`, `generate.py`

**What it does**: Each public function has 2–3 overloads covering:
- Single-label vs. multi-label variants
- Sync wrapper (calls `.run_sync()`) vs. explicit `_async` variant
- Type-safe return types via `TypeVar` and overload annotations

```python
@overload
async def classify_async(
    data: Any,
    labels: Sequence[T] | type[T],
    multi_label: Literal[False] = False,
    *,
    instructions: str | None = None,
    agent: Agent | None = None,
    thread: Thread | str | None = None,
    context: dict[str, Any] | None = None,
    handlers: list[Handler | AsyncHandler] | None = None,
    prompt: str | None = None,
) -> T: ...
```

**Why steal it**: Users can `result = marvin.classify(data, labels)` (sync) or `result = await marvin.classify_async(...)` (async). No bifurcation in the mental model. Leaven's stage decorators are async-only by design; this pattern doesn't apply directly, but the *overload discipline* for variant handling is reusable in `lv.scorer`, `lv.judge`, etc. when they offer "pick a trust model" or "single vs. batch" variants.

**Cost to Leaven**: None — this is standard Python. Adopt for future multi-variant surfaces.

---

### Pattern 2: Dataclass Agent with Field Metadata + Defaults

**Location**: `src/marvin/agents/agent.py` (L49–95)

**What it does**:
```python
@dataclass(kw_only=True)
class Agent(Actor):
    name: str = field(
        default_factory=lambda: random.choice(AGENT_NAMES),
        metadata={"description": "Name of the agent"},
    )
    
    tools: list[Callable[..., Any]] = field(
        default_factory=lambda: [],
        metadata={"description": "List of tools available to the agent"},
    )
    
    model: KnownModelName | Model | None = field(
        default=None,
        metadata={
            "description": "The language model configuration for the agent."
            " Can be a known model name, a Pydantic AI Model instance,"
            " or None to use the default."
        },
    )
```

Each field has a `metadata["description"]` that documents *intent*, not just type. `Agent(name="Poet", tools=[...])` is readable and self-documenting.

**Why steal it**: Leaven's decorators and records (`Case`, `Assessment`, `Evidence`) are already Pydantic BaseModel or dataclass. Adding field-level descriptions (vs. just docstrings) would let IDE tooltips show purpose without opening docs. Lower cognitive load.

**Cost to Leaven**: Minimal. Most surfaces are already structured. Adopt in `RegisteredStage`, `AssessmentWrite`, `StageSourceRef` on next pass.

---

### Pattern 3: Task Generic Container + Type Adapter Caching

**Location**: `src/marvin/tasks/task.py` (L53–59), (L75–100)

**What it does**:
```python
_type_adapters: dict[type[Any], TypeAdapter[Any]] = {}

def get_type_adapter(result_type: type[T]) -> TypeAdapter[T]:
    if result_type not in _type_adapters:
        _type_adapters[result_type] = TypeAdapter(result_type)
    return _type_adapters[result_type]

@dataclass(kw_only=True, init=False)
class Task(Generic[T]):
    instructions: str = field(...)
    result_type: ResultType[T] = field(...)
    context: dict[str, Any] = field(...)
```

Pydantic TypeAdapter is created once, then reused. Avoids repeated schema introspection.

**Why steal it**: Leaven's evidence serialization (`EvidenceEnvelope`, `Proposal`, `Assessment`) uses Pydantic for JSON/wire serialization. Caching TypeAdapters for frequently-used types (common case schemas, common scorer return types) would reduce per-call overhead. Marvin demonstrates the pattern works at scale.

**Cost to Leaven**: Trace overhead if evidence types are heterogeneous. Measure before adopting; benefit is real but modest if run counts are low.

---

## 4. Where Marvin's Ergonomics Go Too Far

### Anti-Pattern: Implicit Context Var + Global Thread State

**Location**: `src/marvin/tasks/task.py` (L48–51), `thread.py` (L31–33)

```python
_current_task: ContextVar["Task[Any] | None"] = ContextVar(
    "current_task",
    default=None,
)

# Later: task.run() calls marvin.run("...") inside, which implicitly uses the thread
# from context var, not from an explicit argument.
```

**Why it's too magical**: 
1. Silent action-at-a-distance when you call `marvin.run()` inside a task or agent handler — the thread is fished out of the ContextVar, not passed explicitly.
2. Breaks IDE traceability: "where does this thread come from?" requires knowing about ContextVars and how Marvin's run loop binds them.
3. Makes testing harder: tests must carefully set/unset the context var or thread lookups fail.
4. Leaven's audit model *requires* explicit data flow: every assessment is tied to a `StageContext` that carries the run/case/trust boundary. Implicit thread state would defeat the audit trail.

**Cost to Leaven if adopted**: Loses explicit wire traceability. Conflict with `RunContext` binding model — Leaven stages accept context as an argument so the engine can enforce capability tokens and target visibility. Implicit context vars would require the stage to fish the context out of somewhere, losing the "where did this come from?" proof.

**Recommendation**: Do NOT adopt this pattern. Leaven's explicit `cx` parameter is the right choice.

---

## 5. What Would NOT Survive Adversarial Review

### Concern 1: Tool/Memory Registration Without Proof of Capability

**Location**: `src/marvin/agents/agent.py` (L59–73)

```python
tools: list[Callable[..., Any]] = field(...)
memories: list[Memory] = field(...)
mcp_servers: list[Any] = field(...)  # Note: Any
```

An `Agent` can be constructed with a list of tools or MCP servers without:
- Proof that the agent has permission to call them
- Audit trail of what the agent *actually* called
- Trust boundary validation (does the tool access untrusted data?)

In Marvin, this is acceptable because Marvin is not a compliance engine. Leaven is. Leaven stages receive tools/capabilities through the `StageContext` (cf. `cx.sandbox.exec`, `cx.agent.run`), and those are bound to the stage's trust profile at registration time. An `AssessmentWrite` can prove which evidence it produced because the evidence is typed and enrolled.

**Cost to Leaven**: If Leaven imported Marvin's agent surface directly, it would lose the capability-token proof model. Leaven stages would have to re-do Marvin's tool list → tool call audit on their own, losing the engine's enforcement.

**Recommendation**: Inherit the *Agent with tool registration* concept, but keep the capability binding in `RegisteredStage` (trust_profile, input_classes, forbidden_input_classes). Stages are not general-purpose agents; they are auditable labor units.

---

### Concern 2: Event Handler + Stream API Adds Unbounded Complexity

**Location**: `src/marvin/handlers/`, `src/marvin/engine/events.py`

Marvin provides:
- `Handler` and `AsyncHandler` for custom event processing
- `run_stream()` and `run_tasks_stream()` to yield `Event` objects in real-time
- Multi-level tracing (agent tool use, message routing, task state transitions)

This is powerful for instrumentation and custom UI. But the event system is *not enforced* — a handler can silently drop events, or a test can pass without consuming the stream. Events are advisory.

**Cost to Leaven**: Leaven's audit model requires durable, complete evidence. If stages emit assessment evidence through an event handler (vs. returning it directly), the engine must verify that evidence made it to storage. Optional event streams would require extra proof work.

**Recommendation**: Leaven's `AssessmentWrite` + `RunContext` model is correct: stages return structured results, and the engine applies them. Event tracing is good for development and debugging (Leaven could add event logs later), but must not be the *source* of assessment truth.

---

## 6. Surprises + Open Questions

### Surprise 1: Marvin's Plan API Is Skeletal

**Location**: `src/marvin/fns/plan.py` (not fully shown above)

Marvin provides `marvin.plan("Create a blog post")` which returns a list of `Task` objects. But:
- The planner is an LLM (no explicit plan IR)
- Dependencies are implicit in task ordering, not explicit
- No plan validation or "can this plan be executed?" check

Leaven's optimizer, by contrast, works with *explicit* plan IR (the proposal envelope). This is intentional: Leaven needs to prove that a plan satisfies the search space constraints before executing it.

**Question for Leaven**: Should the Python surface offer a high-level `lv.plan(...)` convenience? Marvin shows the UX is easy (`plan("make a blog post") → [Task(...), Task(...)]`). But Leaven's proposals are typed and must satisfy constraints. A Python plan API would need to:
1. Accept a user prompt + constraints
2. Return typed `Proposal` objects (not just tasks)
3. Validate each proposal against the optimizer's acceptance gate

This is out of scope for the initial surface, but the pattern is interesting.

---

### Surprise 2: Marvin Uses Both Classes + Functions

Marvin has:
- `Agent` and `Task` classes (stateful, mutable in some cases)
- `marvin.run()`, `marvin.classify()`, `marvin.extract()` functions (stateless, single-call)
- `@lv.fn` decorator (for function-based task prediction — somewhat experimental)

Leaven is purely function + decorator (`@lv.runner`, `@lv.scorer`, etc.). This is more restrictive but ensures:
- Stages are pure serializable artifacts (can be shipped as code)
- Composition is explicit (via `lv.optimize(runner=..., scorer=...)`)
- No hidden mutable state

**Question for Leaven**: The Marvin dual-API is ergonomic for "quick task" use cases (5-line script). Leaven's decorator API is ergonomic for "repeatable, auditable, reproducible" use cases (research, papers). Is there a Leaven use case for a one-off task surface similar to `marvin.run(...)`? Currently, no — even a quick eval is typed and composed through `lv.optimize()`. This is probably correct, but worth revisiting if users ask for "just call the LLM without setting up the full framework."

---

## Summary

**Top 3 Patterns Worth Stealing**:
1. **Overloaded fn-level API with sync/async parity** (`classify.py` L24–51) — reusable for future Leaven surfaces with variants
2. **Dataclass fields with `metadata["description"]`** (`agent.py` L53–95) — better IDE tooltips and self-documenting records
3. **Type adapter caching** (`task.py` L53–59) — modest performance win for frequent Pydantic serialization

**1 Too-Magical Anti-Pattern**:
- **Implicit ContextVar thread state** (`task.py` L48–51) — loses traceability, conflicts with Leaven's explicit `RunContext` binding model

**1 Review Concern**:
- **Tool registration without capability proof** (`agent.py` L59–73) — acceptable for general-purpose agents, unsuitable for auditable optimization stages. Keep Leaven's capability-token model.

**File Path**: `/Users/darin/src/personal/leaven/docs/specs/leaven_py/docs/agent-context/patterns/marvin-patterns.md`
