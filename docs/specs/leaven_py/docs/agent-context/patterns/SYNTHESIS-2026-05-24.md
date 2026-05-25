# Pattern Synthesis — 2026-05-24

Across 8 Round-4 vendored Python references (BAML, pydantic-ai,
temporal-python-sdk, marvin, weave, anthropic-sdk-python, jupyter-client,
python-lsp-jsonrpc), 8 parallel "what to steal / avoid / surprising / what
wouldn't survive adversarial review" pattern files now sit in this
directory. This file is the cross-cutting synthesis: convergent findings,
concrete refinement candidates for the scaffold + spec, and explicit
non-actions.

Authority: this is a synthesis of agent observations + my own focused
read of `repos/pydantic-ai/pydantic_ai_slim/pydantic_ai/_run_context.py`.
It does NOT propose unilateral changes to the scaffold or spec — every
load-bearing call here surfaces to the user for explicit approval.

## Per-repo files (read these directly when implementing related work)

- [baml-patterns.md](baml-patterns.md) — closest architectural peer (Rust core + Python)
- [pydantic-ai-patterns.md](pydantic-ai-patterns.md) — literal `RunContext` name match
- [temporal-python-sdk-patterns.md](temporal-python-sdk-patterns.md) — Python decorators + Rust + replay
- [marvin-patterns.md](marvin-patterns.md) — ergonomic surface above typed core
- [weave-patterns.md](weave-patterns.md) — `@weave.op()` UX target
- [anthropic-sdk-patterns.md](anthropic-sdk-patterns.md) — major LLM provider SDK shape
- [jupyter-client-patterns.md](jupyter-client-patterns.md) — battle-tested stdio RPC patterns
- [python-lsp-jsonrpc-patterns.md](python-lsp-jsonrpc-patterns.md) — minimal Python JSON-RPC reference

## Convergent findings (multiple agents independently)

### 1. Opaque Rust refs + typed payloads (BAML, temporal, jupyter-client)

The Python side never holds Rust internals — only opaque handles + serialized
payloads. Temporal: *"PyO3 module boundary is request/reply, not shared state."*

**Status in our scaffold:** validated. Our `CallReceipt`/`QueryReceipt`/`WriteReceipt`
already follow this pattern — `receipt_id: str` is opaque, Python threads them
through, can't forge.

**Action:** none required; the design is already right. Cite this convergence
in the spec's "What is preserved" section as design-pressure justification.

### 2. ContextVar for current context — mixed signals

- BAML, pydantic-ai use ContextVar cleanly for current-context lookup.
- Marvin agent flagged it as anti-pattern: *"implicit ContextVar thread
  binding conflicts with Leaven's RunContext capability-token model."*

**Status:** explicit divergence by design. We pass `cx` as an argument to
every stage function. No `get_current_context()` magic.

**Action:** **don't add ContextVar.** The cost of implicit context isn't
worth it for an audit-first system. Document the decision explicitly in
the spec's "Constraints on implementation" so a future contributor
doesn't add it without revisiting.

### 3. No Python sandbox

- Temporal agent: *"Temporal's sandbox (RestrictedPython + builtins patching)
  has a large attack surface. ... Current Leaven design (trusted Python, no
  sandbox) avoids the risk entirely — keep it."*

**Status:** validated by parallax. Our trust profiles are policy declarations
that the engine enforces at capability boundaries; we don't pretend to
sandbox arbitrary Python at the stage level.

**Action:** none. The decision is right.

### 4. Schema-locked typed validation (anthropic-sdk warning)

- Anthropic SDK uses `_strict_response_validation=False` as default. Schema
  mismatch from API drift is silently ignored.

**Status:** our scaffold uses `extra="forbid"` on pydantic models by
convention (per spec's Public API discipline). This is the inverse of the
anti-pattern — we'd hard-fail on drift.

**Action:** none. The discipline is already in spec.

## Single-source refinement candidates (load-bearing, user decision needed)

### A. `RunContext[DepsT]` generic for typed user dependencies

**Source:** pydantic-ai's killer feature.

**Their shape:** `class RunContext(Generic[RunContextAgentDepsT])` with a
`deps: RunContextAgentDepsT` field. User provides `class MyDeps: db: ...`
and writes `async def evaluate(job, cx: RunContext[MyDeps])`. Inside the
stage: `cx.deps.db.query(...)`.

**Our current shape:** `RunContext` is a thin namespace with builder
properties (`cx.case`, `cx.workspace`, etc.). No user-typed deps slot.

**Tension:** pydantic-ai's flat-field god-object is the wrong shape for
us (validated by separate marvin pushback on flat-field-as-context).
Adding `cx.deps: DepsT` without bloating into the same anti-pattern is
the design challenge.

**Sketch of how it could land:**

```python
class RunContext[DepsT](_ContextBase):
    case: CaseBuilder
    workspace: WorkspaceBuilder
    # ... other builders ...

    @property
    def deps(self) -> DepsT:
        """User-provided typed dependencies (DB pools, custom validators, etc.).

        Type via `@lv.runner(deps=MyDeps)` decorator parameter; access in
        stage body as `cx.deps.<field>`. None when no deps registered.
        """
        ...
```

**Decision needed from user:** add to spec + scaffold, or skip?

### B. RunContext field discipline (load-bearing adversarial-review finding)

**Source:** pydantic-ai agent.

**Concrete failure mode:** their `RunContext.tool_manager` is unavailable in
`TemporalRunContext` (their durable-execution variant). User code that
checks `ctx.tool_manager` silently fails on durable replays.

**Discipline:** **don't put fields on `RunContext` that are optional across
execution boundaries.** Either every context type (RunContext, StageContext,
EvalContext) has the field, or it's not a context field at all.

**Action:** add this as an explicit rule to the spec's "Public API
discipline" section and to leaven_py/AGENTS.md. Currently implicit.

### C. PyO3 abi3-py38 stable ABI for Python wheels

**Source:** BAML.

**The pattern:** BAML uses PyO3 with `abi3-py38` so a single wheel works
across Python 3.10–3.13+. Zero version skew, simpler distribution matrix.

**Our context:** when `leaven-acp`'s Python client + the bundled `leaven`
binary need a PyO3 layer (if any), this is the right call from day one.
Currently our scaffold has no PyO3 (the spec rejected pyo3-as-canonical-API
per archived design notes), but if a thin pyo3-bridged transport later
shows up for in-process dev, we'd want abi3.

**Action:** note in `docs/working-memory/rust-leaven-usability.md` so it's
remembered if PyO3 ever comes up. No spec change needed today.

### D. Idempotency key in CallReceipt

**Source:** anthropic-sdk.

**Their pattern:** request options are deep-copied before retry loops; an
idempotency key is auto-generated once and reused across retries (so the
provider can deduplicate).

**Our spec implication:** `CallReceipt` should carry the idempotency key
used for the underlying request. Currently it has `receipt_id` only.
Adding `idempotency_key: str` (or making it part of the audited request
hash) lets us audit "this same request was attempted N times" and gives
providers their dedup signal.

**Action:** propose adding `idempotency_key` to `CallReceipt` shape +
spec's "What is preserved" → receipts section. User decision.

### E. Credential precedence chain

**Source:** anthropic-sdk.

**Their pattern:** explicit args → env vars → profile lookup →
workload-identity federation → disk. Enables zero-code-secret patterns.

**Our context:** `lv.lm.anthropic(api_key_env="ANTHROPIC_API_KEY")` only
covers env-var. We could add a `credentials=` provider interface.

**Action:** low-priority refinement candidate. Add to `lv.lm` builder
signatures when LM wiring becomes real implementation work.

### F. Stacked handler chain (input → output → finish)

**Source:** weave.

**Their pattern:** three orthogonal hooks instead of one postprocessor.
Integrations compose independently.

**Our context:** we don't have hooks today. When we add them (for tracing,
metrics, debugging), the three-hook shape is what to start with.

**Action:** save for when hooks become real spec work.

### G. Exception types come with recovery semantics

**Source:** BAML agent flagged as review blocker.

**The discipline:** every error type a user sees must have actionable
recovery. "Document how users recover from `LeavenValidationError` vs
`LeavenTimeoutError`, or flatten the hierarchy to 3–4 types."

**Action:** when we define our error hierarchy (not in the scaffold yet),
hold this discipline. Add as a rule to leaven_py/AGENTS.md and spec.

## Patterns to NOT copy (named per source)

- **marvin's implicit ContextVar binding** — conflicts with capability-token model
- **pydantic-ai's mid-turn `cx.enqueue()`** — solving a problem we don't have (we're bounded stage calls, not long-lived conversations)
- **temporal's workflow versioning** — designed for years-long workflows; ours are bounded optimize loops
- **BAML's global singleton context manager** — must support multiple optimizations in one process (Jupyter, tests)
- **weave's W&B-coupled `@op` decorator** — backend must stay pluggable
- **jupyter-client's HMAC-on-every-frame** — overkill for stdio subprocess
- **jupyter-client's traitlets configuration cascade** — use plain injection
- **anthropic-sdk's blocking retry loop with no backpressure** — surface rate-limit feedback explicitly

## What this synthesis does NOT do

- **Doesn't modify the scaffold.** Every refinement candidate (A–G above)
  requires explicit user approval before any `src/leaven/` change.
- **Doesn't claim convergence equals correctness.** Three agents agreeing
  on opaque-refs is parallax; three agents agreeing on ContextVar would
  still need taste check (and indeed marvin's agent pushed back).
- **Doesn't extend the vendor inventory.** Round 4 is closed; Phase 2
  (verifiers, harbor) was added in parallel by user; no further additions
  proposed here.
- **Doesn't update conformance matrix rows.** The ACP rows status remains
  per `docs/specs/public-seam-v1/conformance-matrix.yaml` — the
  trace2skill ACP external-worker proof commit is real but scoped to one
  deterministic benchmark case, NOT live provider/model reproduction.

## Provenance

Generated 2026-05-24 by reading the 8 Round-4 pattern files (written by
parallel Explore agents) and cross-referencing with main-thread focused
read of pydantic-ai's `RunContext`. Each agent was briefed with explicit
"what would NOT survive adversarial review" language to surface concrete
weaknesses, not just rosy idioms. Convergent findings (3+ agents) are
treated as parallax-validated; single-source findings (A–G) require
explicit user approval before scaffold changes.
