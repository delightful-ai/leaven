# Pydantic AI Patterns: Per-Repo Observation

**Vendor:** pydantic/pydantic-ai@main (vendored at `repos/pydantic-ai/`)
**Comparison Target:** Leaven's `RunContext`, decorator surface, multi-provider LM lowering, eval framework
**Date:** 2026-05-24

## 1. What to Read First Inside repos/pydantic-ai/

- **`pydantic_ai_slim/pydantic_ai/_run_context.py:34–174`** — Their `RunContext` class definition with all fields and methods. Essential to compare against Leaven's three-flavor context (RunContext, StageContext, EvalContext).
- **`pydantic_ai_slim/pydantic_ai/models/__init__.py:680–830`** — The `Model` abstract base class and the multi-provider lowering contract. Key to understanding how they unify Anthropic, OpenAI, Bedrock, etc. behind a single interface.
- **`pydantic_graph/pydantic_graph/basenode.py:27–35`** — Their lightweight `GraphRunContext` (only `state` and `deps`), used by the graph runner. Inverse of Leaven's rich context.
- **`pydantic_evals/pydantic_evals/dataset.py`** (60KB) — Dataset/Case API and evaluation lifecycle. Compare against Leaven's evaluator surface and evidence collection.
- **`examples/pydantic_ai_examples/bank_support.py`** — Concrete example of RunContext injection via `@agent.tool` and `@agent.instructions`. Shows dependency wiring (`ctx.deps`) and context-aware tool parameter resolution.

## 2. THE RunContext Comparison (LOAD-BEARING)

### Pydantic AI's RunContext (File:Line)

**Location:** `pydantic_ai_slim/pydantic_ai/_run_context.py:34–174`

```python
@dataclasses.dataclass(repr=False, kw_only=True)
class RunContext(Generic[RunContextAgentDepsT]):
    """Information about the current call."""

    deps: RunContextAgentDepsT
    model: Model
    usage: RunUsage
    agent: Agent[RunContextAgentDepsT, Any] | None = None
    prompt: str | Sequence[_messages.UserContent] | None = None
    messages: list[_messages.ModelMessage] = []
    validation_context: Any = None
    tracer: Tracer = field(default_factory=NoOpTracer)
    trace_include_content: bool = False
    instrumentation_version: int = DEFAULT_INSTRUMENTATION_VERSION
    retries: dict[str, int] = {}
    tool_call_id: str | None = None
    tool_name: str | None = None
    retry: int = 0
    max_retries: int = 0
    run_step: int = 0
    tool_call_approved: bool = False
    tool_call_metadata: Any = None
    partial_output: bool = False
    run_id: str | None = None
    conversation_id: str | None = None
    metadata: dict[str, Any] | None = None
    model_settings: ModelSettings | None = None
    pending_messages: list[PendingMessage] | None = None
    tool_manager: ToolManager[RunContextAgentDepsT] | None = None

    def enqueue(self, *content: EnqueueContent, priority: PendingMessagePriority = 'asap') -> None:
        """Enqueue content to inject into the conversation."""
        ...

    @property
    def last_attempt(self) -> bool:
        return self.retry == self.max_retries
```

### Leaven's RunContext (File:Line)

**Location:** `docs/specs/leaven_py/src/leaven/context.py:54–66` (scaffold)

```python
class RunContext(_ContextBase):
    """Context passed to @lv.runner and @lv.scorer stages."""

    @property
    def candidate_id(self) -> str:
        """The candidate being evaluated in this run."""
        ...

    @property
    def case_id(self) -> str:
        """The case being evaluated in this run."""
        ...
```

Plus shared base (`_ContextBase:28–52`):
```python
class _ContextBase:
    case: CaseBuilder          # cx.case.*
    workspace: WorkspaceBuilder # cx.workspace.*
    lm: LmBuilder              # cx.lm.*
    agent: AgentBuilder        # cx.agent.*
    sandbox: SandboxBuilder    # cx.sandbox.*
    assessments: AssessmentsBuilder
    proposals: ProposalsBuilder

    def batch(self) -> BatchBuilder:
        """Open a batch context."""
        ...

    @property
    def stage_id(self) -> str:
        """The stage call id (engine-minted)."""
        ...

    @property
    def capability_fingerprint(self) -> str:
        """Capability document fingerprint."""
        ...
```

### Top-3 Deltas

1. **Injection Mechanism:** Pydantic AI passes `RunContext` as an optional parameter to agent-decorated functions (tools, instructions). Leaven passes it as the sole context parameter to all six stage roles (`@lv.runner`, `@lv.scorer`, `@lv.reflector`, etc.). Pydantic AI's approach is opt-in per tool; Leaven's is mandatory per stage.

2. **Builder Surface vs. Direct Fields:** Pydantic AI's `RunContext` exposes flat fields (`deps`, `model`, `usage`, `retry`, `run_id`, `conversation_id`, `metadata`). Leaven's context exposes *builders* (`cx.case`, `cx.workspace`, `cx.lm`, `cx.agent`) that are DSL entry points for cross-cutting operations. Pydantic AI's strategy is "pass state, let tools query it"; Leaven's is "provide operation namespaces."

3. **Message Queue vs. Detached Output:** Pydantic AI's `pending_messages` field and `enqueue()` method allow tools to inject messages into the live conversation mid-run. Leaven's contexts do not expose a message queue (stage outputs are returned, not queued), reflecting the decoupled engine-subprocess architecture.

4. **Conversation/Run Tracking:** Pydantic AI tracks `conversation_id` (multi-turn state), `run_id` (this execution), `run_step` (agentic loop iteration), `tool_call_id` (which tool), `tool_name`, retry state. Leaven tracks `candidate_id`, `case_id`, `stage_id`, `parent_candidate_id` (for proposers), `evaluation_request_id` (for evaluators)—focused on GEPA artifact identity, not LLM/tool state.

5. **Tracer, Usage, Instrumentation:** Pydantic AI's context carries OpenTelemetry `Tracer`, `RunUsage`, and instrumentation version. Leaven's context delegates telemetry to the engine and builders; the stage code is test/measurement-agnostic.

## 3. Multi-Provider Lowering

**Location:** `pydantic_ai_slim/pydantic_ai/models/__init__.py:680–830`

Pydantic AI's unified model interface:

```python
class Model(ABC, Generic[InterfaceClient]):
    """Abstract class for a model."""

    _provider: Provider[InterfaceClient]
    _profile: ModelProfileSpec | None = None
    _settings: ModelSettings | None = None

    async def request(
        self,
        messages: list[ModelMessage],
        model_settings: ModelSettings | None,
        model_request_parameters: ModelRequestParameters,
    ) -> ModelResponse:
        """Make a request to the model."""
        raise NotImplementedError()

    async def request_stream(
        self,
        messages: list[ModelMessage],
        model_settings: ModelSettings | None,
        model_request_parameters: ModelRequestParameters,
        run_context: RunContext[Any] | None = None,
    ) -> AsyncIterator[StreamedResponse]:
        """Make a streaming request."""
        raise NotImplementedError()

    def customize_request_parameters(self, params: ModelRequestParameters) -> ModelRequestParameters:
        """Customize tool/output schemas per model."""
        ...

    def prepare_request(
        self,
        model_settings: ModelSettings | None,
        params: ModelRequestParameters,
    ) -> tuple[ModelSettings | None, ModelRequestParameters]:
        """Merge settings and normalize request."""
        ...
```

**Key Patterns:**

- **Unified Message Format:** All providers accept `list[ModelMessage]` (pydantic_ai's canonical wire format: `UserPromptPart`, `SystemPromptPart`, `ToolCallPart`, `TextPart`, etc.). Each provider's subclass (e.g., `Anthropic`, `OpenAI`) translates this to its native API (Anthropic's `TextBlock`, OpenAI's `ChatCompletionMessageParam`, etc.).

- **Dual-Channel Settings:** `ModelSettings` (generic, provider-agnostic: `temperature`, `max_tokens`, `thinking`) + provider-specific settings (stored in `ModelSettings` dict or subclass). Example: `anthropic_` prefix for Anthropic-only fields. `prepare_request()` merges defaults with call-time overrides.

- **Tool Schema Customization:** `ModelRequestParameters` holds the canonical `ToolDefinition` list. `customize_request_parameters()` applies provider-specific JSON Schema transformations (Anthropic's `cache_control`, OpenAI's function schema dialect, etc.) *before* sending.

- **Provider Routing:** The `@dataclass` on `Model` is generic over `InterfaceClient` (the provider's HTTP client type). Each subclass (`AnthropicModel(Model[AsyncAnthropicClient])`, `OpenAIModel(Model[AsyncOpenAI])`) holds the right client and implements `request()` to call the right API.

- **Streaming Symmetry:** Both `request()` and `request_stream()` apply identical response parsing. This prevents feature drift (e.g., tool calling only works in non-streaming, or vice versa).

**Leaven Parallel:** Leaven's LM builder (`cx.lm.*`) would be the user-facing surface; the multi-provider lowering would live in the engine's `crates/leaven-lm` or equivalent, handling provider routing after the Rust stage publishes its LM request via ACP.

## 4. Durable Execution / Graph

**Location:** `pydantic_graph/pydantic_graph/basenode.py` + `pydantic_ai_slim/pydantic_ai/_agent_graph.py`

### GraphRunContext (Minimal)

```python
@dataclass(kw_only=True)
class GraphRunContext(Generic[StateT, DepsT]):
    """Context for a graph."""
    state: StateT
    deps: DepsT
```

Pydantic Graph is a type-hint-driven graph executor. The agent loop is a graph: `UserPromptNode` → `ModelRequestNode` → `CallToolsNode` → (repeat) → `End`. The `GraphRunContext` is deliberately *minimal*—only state and deps. Node implementations (`BaseNode` subclasses) extract what they need from state and make explicit transitions.

The pydantic-ai agent graph (`_agent_graph.py:~3500 lines) includes nodes for:
- User prompt assembly (with instructions)
- Model request (with LM call, token counting, model request hooks)
- Tool calling (dispatch to tool manager, track retries, call hooks)
- Output validation (parse structured output, retry on validation error)
- End (return result)

**Leaven Parallel:** Leaven's engine does NOT expose a graph API to user code. The engine's *internal* graph is similar (Case → Run → Reflection → Proposal loop), but the public-seam stages (`@lv.runner`, `@lv.reflector`, etc.) are ACP RPC calls, not graph nodes. Leaven's `optimize(...).run()` orchestrates the graph; pydantic-ai's graph is the orchestrator itself.

## 5. Eval Surface

**Location:** `pydantic_evals/pydantic_evals/dataset.py` (60KB) + `pydantic_evals/pydantic_evals/__init__.py`

### Public API

```python
# From pydantic_evals/__init__.py
Case       # A (input, expected_output) pair, optionally with metadata
Dataset    # A collection of Cases, with evaluation tracking
CaseLifecycle  # Hooks: on_case_start, on_case_evaluate, etc.

# Core operations
case = Case(input='...', expected='...')
dataset = Dataset(cases=[case, ...])

# Evaluation is declarative:
# User defines evaluators (async functions -> Score/metric dict).
# User calls dataset.run_evals(my_evaluator, ...) -> DataFrame results.
```

**Key Patterns:**

- **Case-Centric:** A `Case` is input + expected output + optional metadata. Evaluators receive a Case and produce a score. No internal split between "runner case" and "scorer case"—they are the same object.

- **Async Evaluators:** Evaluators are plain async functions `async def eval_case(case: Case) -> dict[str, float]`. No decorator, no context injection (at least not in the public surface).

- **Inline Metrics:** Pydantic Evals uses `set_eval_attribute()` to mutate case metadata during evaluation. Results are collected into a pandas DataFrame.

**Leaven Parallel:** Leaven's public seam has `Case` (target + metadata), separate `@lv.runner` and `@lv.scorer` stages (not a single evaluator), and `EvalContext` for evaluators. Pydantic Evals is simpler (one evaluator function per case), while Leaven's decoupling allows runner and scorer to run in different sandboxes with different trust levels.

## 6. Patterns Worth Stealing

### A. Generic Over User Dependencies: The `Generic[RunContextAgentDepsT]` Pattern

**Pydantic AI:** `RunContext[DepsT]` is generic over the user's dependency type (e.g., `RunContext[SupportDependencies]`). The typing is strict: `ctx.deps` is typed as `SupportDependencies`, and IDE autocomplete works.

**Excerpt:**
```python
@support_agent.tool
async def customer_balance(ctx: RunContext[SupportDependencies]) -> str:
    balance = await ctx.deps.db.customer_balance(id=ctx.deps.customer_id)
    return f'${balance:.2f}'
```

**Why We Should Steal It:** Leaven's `RunContext[RunnerDepsT]` could be generic over user context objects (workspace state, LM config, sandbox config) injected by the engine. Current scaffold uses property methods (`cx.workspace`, `cx.lm`); generics would enable static type checking of user-provided context without runtime magic.

**Why Blind Copy Would Be Wrong:** Leaven's context is three-flavored (RunContext, StageContext, EvalContext) with different privilege levels and lifecycle rules. Adding generics requires careful typing at the public seam; it's not just `Generic[T]` on RunContext.

### B. Optional Context Injection: Check `ctx is None` Pattern

**Pydantic AI:** Tools and instructions can request `RunContext` as an optional parameter:
```python
@agent.instructions
async def add_customer_name(ctx: RunContext[SupportDependencies]) -> str:
    # ctx is injected by the agent; always present here.
    customer_name = await ctx.deps.db.customer_name(...)
```

But agent-adjacent code can also use `get_current_run_context()` (a ContextVar):
```python
ctx = get_current_run_context()
if ctx is not None:
    # Safe to use ctx.
```

**Why We Should Steal It:** Leaven stages always have a context (it's mandatory), but utility functions nested inside a stage might want optional access. Using a ContextVar allows library code to "peek at" the live context without threading it through 10 function parameters.

**Why Blind Copy Would Be Wrong:** Leaven's ACP subprocess model means the ContextVar is thread-local, not process-local. Each subprocess stage gets a fresh Python process, so ContextVar state is isolated.

### C. Lazy Instrumentation Version Tracking

**Pydantic AI:** `RunContext.instrumentation_version` is an int that increments when instrumentation configuration changes. Hooks can check `if ctx.instrumentation_version != expected: ...` to gate behavior.

**Why We Should Steal It:** Leaven's capability fingerprint is similar (it's a hash of the granted capability document). Adding a version field allows stages to adapt behavior to engine capabilities without parsing the full doc each time.

### D. Unified Tool Definition + Schema Customization

**Pydantic AI:** `ModelRequestParameters` holds canonical `ToolDefinition` (Pydantic schema + docstring). Each provider's `customize_request_parameters()` transforms the schema to the provider's JSON Schema dialect.

**Why We Should Steal It:** Leaven's tool registry is currently implicit (tools are marked `@lv.tool`). Explicit `ToolDefinition` objects (with schema, docstring, signature) would enable:
1. Evaluators to inspect available tools without running the stage.
2. Proposers to suggest tool calls based on schema alone.
3. Sandboxing to validate tool calls before dispatch.

**Why Blind Copy Would Be Wrong:** Pydantic AI's tools are tied to the agent (tools are added via `@agent.tool` or `Agent(..., tools=[...])`). Leaven's tools are implicit in the stage function signature; lifting them to explicit definitions requires schema inference or explicit registration.

### E. Message Enqueuing for Mid-Turn Injection

**Pydantic AI:** Tools call `ctx.enqueue(UserPromptPart("Follow-up question"))` to inject messages mid-run. The agent loop drains the queue before each LLM call.

**Why We Should Steal It:** Leaven's reflectors might want to queue follow-up queries without returning early. Enqueue semantics allow tool-like code to extend the conversation without exiting the stage.

**Why Blind Copy Would Be Wrong:** Leaven runs stages as subprocesses; there's no shared message queue. The engine owns the queue. A stage would `yield` or return a `PendingMessage` type, and the engine would inject it.

## 7. Patterns We Should NOT Copy

### A. Flat RunContext Fields Instead of Builders

**Pydantic AI's Choice:** `RunContext.deps`, `RunContext.model`, `RunContext.usage`, `RunContext.conversation_id`, `RunContext.metadata` are all flat fields.

**Why It Works for Pydantic AI:** The agent is a single, stateful object. Tools are methods on the agent. All tool code is in-process, so the agent can mutate state, track message history, and flush changes.

**Why It Won't Work for Leaven:** Leaven's stages are subprocesses. A stage cannot directly mutate the workspace or case state; it must return a value or call a builder (which is RPC'd to the engine). Exposing flat fields (`cx.case_id`, `cx.model`) would tempt users to try to mutate them, leading to silent failures or race conditions.

**Decision:** Keep builder namespaces (`cx.case.*`, `cx.workspace.*`, `cx.lm.*`). They make the RPC boundary explicit.

### B. RunContext as Optional Parameter

**Pydantic AI's Choice:** `@agent.tool async def my_tool(ctx: RunContext[Deps]) -> str` is optional; if the signature doesn't include `ctx`, the tool doesn't receive it.

**Why It Works for Pydantic AI:** The agent can inspect the function signature and inject or omit `ctx` accordingly. All tools are defined at agent construction time.

**Why It Won't Be Straightforward for Leaven:** Leaven's stages are discovered dynamically by the engine (via ACP RPC). The stage function signature is not known at public-seam definition time. Stages always receive a context (it's in the ACP payload).

**Decision:** Keep context mandatory for all stage roles. If a stage doesn't use it, that's fine—it's in scope but unused. (This matches the scaffold in `decorators.py`.)

### C. Dual-Channel Dependency + Tool Parameters

**Pydantic AI's Choice:** `ctx.deps` is the agent's injected dependencies (database, config, etc.). Tool parameters are separate (`@agent.tool def my_tool(customer_id: int, ctx: RunContext[Deps])`). The agent framework handles parameter binding.

**Why It Works for Pydantic AI:** The decorator can parse the tool signature and auto-fill `customer_id` from the user's prompt or previous tool calls.

**Why It Won't Be Straightforward for Leaven:** Leaven's stages receive `(artifact, case, cx)` or `(request, cx)` as fixed signatures. There's no per-stage parameter binding at the public seam.

**Decision:** Leaven's context carries builders (`cx.case`, `cx.workspace`) that provide the "tool parameter" interface implicitly. If a runner needs case metadata, it calls `cx.case.inputs()`, not receives it as a parameter.

## 8. What Would NOT Survive Adversarial Review

### A. No Formal Audit of RunContext Serialization Across Temporal Boundaries

**Issue:** Pydantic AI uses `TemporalRunContext` for durable execution across Temporal activity boundaries (see `pydantic_ai_slim/pydantic_ai/durable_exec/temporal/_run_context.py`). The scaffold in Leaven's `repos/pydantic-ai/pydantic_ai_slim/pydantic_ai/durable_exec/` suggests this exists but is not tested.

**Risk:** If a reviewer runs the full test suite, Temporal-backed durability might fail silently or with opaque errors.

**Mitigation:** Leaven should audit `TemporalRunContext` fields and confirm all are serializable before adopting durable patterns.

### B. Tool Manager Not in TemporalRunContext

**Issue:** Pydantic AI's `RunContext.tool_manager` field is marked "not available in TemporalRunContext" (line 120–122 of `_run_context.py`). This means tools that rely on `ctx.tool_manager` will break in durable execution.

**Risk:** A tool that calls `ctx.tool_manager.validate_tool_call(...)` will raise `AttributeError` when replayed across a Temporal boundary. Tests might not catch this if they only test the happy path (no retries, no Temporal).

**Mitigation:** Leaven's stages must not access context fields that are not guaranteed to be present across subprocess boundaries.

### C. Enqueue Safety Under Concurrency

**Issue:** Pydantic AI's `enqueue()` comment says "The drain only iterates the queue between graph nodes (in `before_model_request` and `after_node_run`), never concurrently with the tool body, so `list.append` from a worker thread doesn't race." This is true for in-process agents but fragile.

**Risk:** If a tool spawns a thread that calls `enqueue()`, there's a race. The code relies on the agent loop being single-threaded.

**Mitigation:** Leaven's stages are processes (not threads), so race conditions are impossible. But if Leaven ever adopts async/await within a stage, concurrent `enqueue()` calls could break.

### D. Reliance on Pydantic V2 Semantics for Validation Context

**Issue:** `RunContext.validation_context` (line 50) is passed to Pydantic validators. This is a Pydantic V2 feature. If Leaven upgrades Pydantic or changes validator behavior, this field may become obsolete.

**Risk:** An evaluator or stage might use `validation_context` to pass secrets or case metadata to validators. If the mechanism breaks, silent data loss (validator doesn't receive the context) is possible.

**Mitigation:** Document that `validation_context` is a Pydantic-specific hook, not a general context field.

## 9. Surprises + Open Questions

1. **No Built-in Observability Hooks in RunContext:** Pydantic AI's `RunContext.tracer` is a raw OpenTelemetry `Tracer`. There's no `on_tool_call`, `on_model_request` hooks in the context itself—they live on the agent and in capabilities. Leaven's context might want explicit hook points for consistent instrumentation across all stages.

2. **Conversation ID Persistence:** Pydantic AI's `conversation_id` is resolved at `Agent.run()` time from three sources (explicit arg, message history, fresh UUID7). Leaven's context doesn't expose conversation ID. Should it? Or is conversation lifetime an engine concern?

3. **No Schema Validation in RunContext Fields:** Pydantic AI's `metadata: dict[str, Any]` is unvalidated. If a stage mistakenly sets `metadata['run_id'] = 123` (int instead of str), there's no type error. Leaven might want typed metadata (e.g., `@dataclass class RunMetadata: ...`).

4. **RunContext Doesn't Expose Tool Search Results:** Pydantic AI has a `ToolSearchTool` (native tool) that searches available tools by description. `RunContext` doesn't expose the search results, so a stage can't inspect which tools are available. This might be intentional (avoid tool enumeration attacks).

5. **No Direct Access to Agent Definition:** `RunContext.agent` is `Agent[AgentDepsT, Any] | None`. If it's not None, a tool can call `ctx.agent.run(...)` recursively, leading to complex control flow. No safeguards against infinite recursion.

6. **Callback Chains in ModelSettings:** Pydantic AI's `ModelSettings` supports before/after request hooks. `RunContext.model_settings` is populated only in model request hooks, not in tool hooks. Why the asymmetry? Leaven should clarify when `cx.lm.*` operations are available in each stage role.

7. **Graph Persistence Separate from RunContext:** Pydantic Graph has a separate `Snapshot` type for persistence (not part of `GraphRunContext`). If Leaven adopts graph semantics, how does the snapshot relate to the stage context and engine resumability?

---

## Summary

**Top-3 RunContext Deltas:**
1. Pydantic AI injects `RunContext` as an *optional* tool parameter; Leaven passes it *mandatory* to all stages.
2. Pydantic AI exposes flat fields (deps, model, usage, retry); Leaven exposes builder namespaces (cx.case, cx.lm, cx.workspace).
3. Pydantic AI tracks LLM state (run_id, conversation_id, run_step, retry); Leaven tracks artifact state (candidate_id, case_id, stage_id).

**Top-2 Patterns Worth Stealing:**
1. **Generic RunContext over user dependencies:** Enables static typing of `ctx.deps` without reflection. Requires care with three-flavor context hierarchy.
2. **Message enqueuing for mid-turn injection:** Allows tools to extend conversations without exiting. Leaven would need to adapt for subprocess model (return PendingMessage instead of mutating a queue).

**1 Thing NOT to Survive Adversarial Review:**
- `RunContext.tool_manager` is not available in `TemporalRunContext`. Tests that check `ctx.tool_manager` will silently fail on durable replays. Leaven must not depend on context fields that are optionally present.

**Output:** `/Users/darin/src/personal/leaven/docs/specs/leaven_py/docs/agent-context/patterns/pydantic-ai-patterns.md` (this file)
