# Leaven Python — SURFACE.md

Status: build manifest. Single source of truth for the Python type-stub
scaffold. Derived from `docs/specs/leaven_python.md` (governing spec, revised
2026-05-25). Where the spec is silent on an exact field, this file MAKES A
TYPED CHOICE and marks it `[CHOICE]`. Builders MUST follow this manifest so the
package is internally coherent.

## Build rules (apply to every file)

- `from __future__ import annotations` is the first line of every module.
- Every body is `...` or `raise NotImplementedError("<one-line spec pointer>")`.
  NO real logic.
- Pydantic v2 models for wire-shaped / value records: `model_config =
  ConfigDict(frozen=True, extra="forbid")`. Dataclasses (`@dataclass(frozen=True,
  slots=True)`) for internal config objects (builder configs that are not wire
  records). Choice between them is noted per symbol below.
- Python >=3.12 syntax only: `X | None`, builtin generics (`list`, `dict`,
  `Sequence`), `class Foo[T]`, `type Alias = ...`. NEVER `typing.List/Dict/Optional`.
- Every public module ships a sorted `__all__` (ruff RUF022 on).
- `py.typed` ships in the package root.
- `cx` is passed explicitly to every stage fn. NO ContextVar.
- Context types have UNIFORM fields (no field optional-across-boundary).
- Hard cutover: delete the stale modules listed in "Deletions" below. No compat
  shims, no parallel old/new paths.

---

## CROSS-CUTTING SIGNATURES (verbatim — downstream builders inherit these exactly)

```python
# score.py
type Scorer = Callable[[RolloutResult[Any], Case, Context], Awaitable[Score]]

class Score(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    value: float
    feedback: str = ""


# rollout.py
class RolloutStatus(StrEnum):
    ok = "ok"
    timeout = "timeout"
    crash = "crash"
    refused = "refused"
    budget_exceeded = "budget_exceeded"
    error = "error"

class RolloutResult[Out](BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    output: Out
    workspace: WorkspaceView | None = None
    sessions: Sequence[AgentSession] = ()
    trajectory: TrajectoryView | None = None
    output_files: Sequence[WorkspacePath] = ()
    status: RolloutStatus = RolloutStatus.ok


# reflect.py
class Critique(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    summary: str
    failure_modes: Sequence[str] = ()
    suggestions: Sequence[str] = ()
    constraints: Sequence[str] = ()


# propose.py
class Proposal(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    instructions: str | None = None
    change: object | None = None          # artifact-native change-set (opaque to product)
    rationale: str = ""
    effect: ProposalEffect = ProposalEffect.change   # wire.ProposalEffect


# stages.py — EXACTLY four slots
class Stages:
    def __init__(
        self,
        *,
        rollout: RolloutFn | Rollout,
        score: Scorer | Sequence[Scorer],
        reflect: ReflectFn | Reflect | None = None,
        propose: ProposeFn | Propose | None = None,
    ) -> None: ...

    @classmethod
    def evaluator(
        cls,
        evaluate: EvaluatorFn,
        *,
        reflect: ReflectFn | Reflect | None = None,
        propose: ProposeFn | Propose | None = None,
    ) -> Stages: ...


# The four stage-fn signatures (callable protocols; live in stages.py)
type RolloutFn = Callable[[Any, Case, Context], Awaitable[Any | RolloutResult[Any]]]
type ReflectFn = Callable[[ReflectiveBatch, Any, Context], Awaitable[Critique]]
type ProposeFn = Callable[[Any, ReflectionResult, Context], Awaitable[Proposal]]
# score fn IS a Scorer (above)
# evaluator fn (advanced): Callable[[EvaluationJob, EvalContext], Awaitable[None]]


# context.py — product cx (uniform fields across all context types)
@runtime_checkable
class Context(Protocol):
    lm: LmHandle
    agent: AgentHandle
    sandbox: SandboxHandle
    workspace: WorkspaceHandleProto
    async def trace(self, *events: object) -> None: ...
    def batch(self) -> BatchContext: ...
```

Notes on the choices above:
- `Scorer` arg position 1 is `RolloutResult[Any]` (the type alias cannot be
  generic-parameterised at the alias site without losing the annotation-only
  goal). Concrete scorers annotate `RolloutResult[Answer]` directly.
- `RolloutFn` return is `Any | RolloutResult[Any]` because a bare `Out` is
  wrapped by the engine (spec lines 534-576).
- `ReflectiveBatch` is the product-facing pre-built batch (`batch.cases[...]`);
  its typed record members (`ReflectiveCase/ReflectiveRun/Attachment/TraceRef`)
  live in `lv.adapters.reflective`. `[CHOICE]` name `ReflectiveBatch` for the
  product-facing top-level type used in `ReflectFn`; it is re-exported from
  `lv.adapters` (not top-level `__all__`).
- `Proposal.change: object | None` keeps the artifact-native change-set opaque
  at the product layer (`[CHOICE]`; spec calls it "typed artifact-native
  change" but does not name a product type).
- `Context` is a structural `Protocol`. The concrete typed `RunContext /
  StageContext / EvalContext` live in `lv.adapters.contexts` and are the
  annotation surface for advanced authoring.

---

## TOP-LEVEL `__all__` (allow-list — EXACTLY these, nothing more)

`src/leaven/__init__.py` `__all__` (sorted):

```python
__all__ = [
    "Case",
    "Critique",
    "Proposal",
    "Propose",
    "Reflect",
    "Rollout",
    "RolloutResult",
    "Runtime",
    "Score",
    "Scorer",
    "Stages",
    "Task",
    "__version__",
    "agent",
    "artifacts",
    "assets",
    "budget",
    "cases",
    "evaluator",
    "evolve",
    "gepa",
    "layouts",
    "lm",
    "optimizers",
    "output",
    "proposer",
    "reflector",
    "reflector",
    "runner",
    "runs",
    "runtime",
    "sandbox",
    "scorer",
    "serve",
    "setup",
    "trust",
    "workspace",
    "x",
]
```

(De-duplicate `reflector`; the canonical sorted list is: `Case, Critique,
Proposal, Propose, Reflect, Rollout, RolloutResult, Runtime, Score, Scorer,
Stages, Task, __version__, agent, artifacts, assets, budget, cases, evaluator,
evolve, gepa, layouts, lm, optimizers, output, proposer, reflector, runner,
runs, runtime, sandbox, scorer, serve, setup, trust, workspace, x`.)

`runtime` is BOTH the callable `runtime(...)` and carries `.local`/`.acp`
attributes. `Runtime` is the class. `__version__` is a `str`.

### FORBIDDEN from top-level `__all__` (must live in adapters/wire/_engine)

A surface test asserts NONE of these appear in `lv.__all__`:
`OutputRecord, Visibility, EvaluationJob, EvaluationItem, Granularity, Purpose,
EvidenceEnvelope, EvidencePublic, EvidencePrivate, AssessmentWrite,
Replayability, ProposalBatch, ProposalEffect, ReflectRequest, ProposeRequest,
JudgeRequest, ReflectExample, ReflectionResult, ReflectiveCase, ReflectiveRun,
Attachment, TraceRef, StageSourceRef, StageRole, RegisteredStage, RunContext,
StageContext, EvalContext, RunCase, ScoreCase, CandidateHandle, WorkspaceHandle,
WorkspaceLifetime, WorkspaceSurface, QueryReceipt, CallReceipt, WriteReceipt`.

---

## MODULE-BY-MODULE MANIFEST

### `task.py` — owns `Task`

```python
class Task(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    cases: Sequence[Case]
    sandbox: SandboxConfig | None = None
```
- Purpose: immutable task world. `cases` may be supplied directly or via
  `lv.cases.from_jsonl(...)` (which returns `Sequence[Case]`).
- `__all__ = ["Task"]`. `sandbox` value comes from `lv.sandbox.*` builders.
  `[CHOICE]` `SandboxConfig` is the dataclass base from `sandbox/config.py`.

### `case.py` — owns `Case`

```python
class Case(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    id: str
    input: Mapping[str, Any]
    target: Mapping[str, Any] | None = None
    files: Mapping[str, AssetRef] | None = None
    setup: SetupStep | None = None
    split: str | None = None
    metadata: Mapping[str, Any] | None = None
```
- Purpose: one immutable case. `split` is a free user-defined label string (NOT
  an enum). `files` values are `lv.assets.path(...)` refs. `setup` is a
  `lv.setup.bash(...)` step.
- `[CHOICE]`: `target` defaulted `None` (spec example always supplies it but
  signature shows it optional via "Case(id, input, target, ...)").
- `__all__ = ["Case"]`. (Splits helpers, if any, live in `cases/splits.py`, not
  here — keep `case.py` to the record.)

### `score.py` — owns `Score`, `Scorer`

```python
class Score(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    value: float
    feedback: str = ""

type Scorer = Callable[[RolloutResult[Any], Case, Context], Awaitable[Score]]
```
- Purpose: tiny scorer return value (number + why) and the annotation-only
  callable alias. NO `Scorer` class/constructor. NO `metrics`/`output` fields.
- `__all__ = ["Score", "Scorer"]`.

### `rollout.py` — owns `Rollout`, `RolloutResult`, `RolloutStatus`

`Rollout` is a NAMESPACE class (not instantiable as a record) with classmethod
built-in factories returning a frozen config object:

```python
class RolloutStatus(StrEnum): ok / timeout / crash / refused / budget_exceeded / error

class RolloutResult[Out](BaseModel):  # fields per cross-cutting section above

class Rollout:
    @staticmethod
    def agent(
        agent: AgentConfig,
        *,
        layout: Layout | None = None,
        output: OutputContract | None = None,
        instructions: str | None = None,
    ) -> Rollout: ...
    @staticmethod
    def command(
        argv: Sequence[str],
        *,
        layout: Layout | None = None,
        output: OutputContract | None = None,
        cwd: str | None = None,
        env: Mapping[str, str] | None = None,
    ) -> Rollout: ...
    @staticmethod
    def manifest(
        path: str,
        *,
        layout: Layout | None = None,
        output: OutputContract | None = None,
    ) -> Rollout: ...
```
- Purpose: declarative engine-mediated rollout built-ins. `instructions=` is the
  STABLE invocation envelope; mutable instructions live in the artifact.
- `[CHOICE]`: `Rollout` instances are returned opaque (a frozen dataclass under
  the hood, e.g. `_RolloutAgentSpec`/`_RolloutCommandSpec`/`_RolloutManifestSpec`
  private dataclasses); only the three classmethods are public. `output=` value
  is an `OutputContract` from `output.py`. `layout=` is a `Layout` from
  `layouts.py`.
- `__all__ = ["Rollout", "RolloutResult", "RolloutStatus"]`.

### `reflect.py` — owns `Reflect`, `Critique`

```python
class Critique(BaseModel):  # fields per cross-cutting section

class Reflect:
    @staticmethod
    def agent(
        agent: AgentConfig,
        *,
        layout: Layout | None = None,
        instructions: str | None = None,
    ) -> Reflect: ...
```
- Purpose: declarative engine-mediated reflection built-in + the reflect output
  record. Reflect produces a `Critique`, never a candidate change.
- `[CHOICE]`: `Reflect.agent` params mirror `Rollout.agent` minus `output`
  (reflection has no parsed output contract). `layout` optional.
- `__all__ = ["Critique", "Reflect"]`.

### `propose.py` — owns `Propose`, `Proposal`

```python
class Proposal(BaseModel):  # fields per cross-cutting section

class Propose:
    @staticmethod
    def agent_edit(
        agent: AgentConfig,
        *,
        layout: Layout | None = None,
        instructions: str | None = None,
    ) -> Propose: ...
```
- Purpose: declarative engine-mediated artifact-edit built-in + propose output
  record. `agent_edit` materializes parent under `target/current/`, runs the
  agent as editor with the critique attached, reads workspace back as typed
  change; edits outside `mutable=` are rejected on readback.
- `Proposal.effect: wire.ProposalEffect` (re-imported type; the VALUE lives in
  product surface but the enum type is owned by `wire.proposal`).
- `__all__ = ["Proposal", "Propose"]`.

### `stages.py` — owns `Stages` + the stage-fn type aliases

Signature per cross-cutting section. Adds the callable-protocol aliases
`RolloutFn / ReflectFn / ProposeFn / EvaluatorFn`.
- Purpose: the four-slot composition. EXACTLY `{rollout, score, reflect,
  propose}`. `rollout` + `score` required; `reflect`/`propose` optional (GEPA
  installs Codex-backed defaults). `Stages.evaluator(...)` is the advanced
  alternate constructor that replaces `rollout`+`score`.
- A `leaven-py` test locks `Stages.__init__` to exactly these four params (plus
  `self`). NO `evaluate`, `improve`, `judge`, `layout`, `sampler` slot.
- `__all__ = ["EvaluatorFn", "ProposeFn", "ReflectFn", "RolloutFn", "Stages"]`.
  `[CHOICE]`: export the fn-type aliases for annotation reuse.

### `evolve.py` — owns `evolve`, `Evolve`, `Evolved`, `Candidate`, `RunSummary`, `ReplayResult`, `Assessment`

```python
def evolve(
    *,
    artifact: Artifact,
    task: Task,
    stages: Stages,
    optimizer: Optimizer,
    runtime: Runtime,
) -> Evolve: ...

class Evolve:
    async def run(self) -> Evolved[Any]: ...

class Evolved[A]:
    best: Candidate[A]
    frontier: list[Candidate[A]]
    summary: RunSummary
    def test_assessments(self) -> Iterable[Assessment]: ...
    def assessment(self, case_id: str) -> Assessment: ...
    async def replay(self, case_id: str) -> ReplayResult: ...
    def lineage(self, candidate_id: str) -> Iterable[Candidate[A]]: ...

class Candidate[A](BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    id: str
    artifact: A
    proposal: Proposal | None = None
    scores: Mapping[str, float] = {}

class RunSummary(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    run_dir: str | None = None
    iterations: int = 0
    cost_usd: float = 0.0
    calls: int = 0
    replayable: bool = False

class Assessment(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    case: Case
    score: Score
    scorer_name: str
    replayable: bool = True

class ReplayResult(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    case_id: str
    output: Any = None
    score: Score | None = None
    deterministic: bool = True
```
- Purpose: entry point + typed result facade. `evolve(...)` returns an `Evolve`
  builder whose `.run()` awaits to `Evolved[A]`.
- `[CHOICE]`: `evolve` keyword-only; `Evolve` is the returned builder (spec
  writes `lv.evolve(...).run()`). `Candidate.scores`, `RunSummary`, `Assessment`,
  `ReplayResult` field sets are typed choices (spec names them but not fields).
- DEPRECATED alias: `optimize = evolve` with a `DeprecationWarning` (spec lines
  902-904). Place the alias here; do NOT export in top-level `__all__`.
- `__all__ = ["Assessment", "Candidate", "Evolve", "Evolved", "ReplayResult",
  "RunSummary", "evolve"]`. (`Candidate/Evolved/...` are returned types, not
  top-level product nouns — they live here, reached via the `Evolved` result,
  not imported by name from `lv`.)

### `runtime.py` — owns `runtime`, `Runtime`, `environment` (deprecated alias)

```python
class Runtime(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    lm: LmConfig | Sequence[LmConfig] | Mapping[str, LmConfig] | None = None
    agent: AgentConfig | Mapping[str, AgentConfig] | None = None
    sandbox: SandboxConfig | None = None
    workspace: WorkspaceConfig | None = None
    trust_profile: TrustProfile | str = TrustProfile.managed_sandbox
    budget: Budget | None = None
    cache: CacheConfig | None = None

def runtime(
    *,
    lm: LmConfig | Sequence[LmConfig] | Mapping[str, LmConfig] | None = None,
    agent: AgentConfig | Mapping[str, AgentConfig] | None = None,
    sandbox: SandboxConfig | None = None,
    workspace: WorkspaceConfig | None = None,
    trust_profile: TrustProfile | str = TrustProfile.managed_sandbox,
    budget: Budget | None = None,
    cache: CacheConfig | None = None,
) -> Runtime: ...

# attached to the runtime callable:
runtime.local: Callable[..., Runtime]   # def local(*, budget=None, workspace=None, ...) -> Runtime
runtime.acp:   Callable[..., Runtime]   # def acp(*, worker="leaven serve --stdio", budget=None, ...) -> Runtime

def environment(*args, **kwargs) -> Runtime: ...   # DEPRECATED ALIAS, emits DeprecationWarning
```
- Purpose: execution substrate. `runtime.agent` is the engine-mediated executor
  (runtime config, NOT artifact state).
- `[CHOICE]`: implement `runtime` as a callable object/function carrying `.local`
  and `.acp` static methods (e.g. a module-level instance of a class with
  `__call__`, `local`, `acp`). `runtime.local` signature: `(*, budget=None,
  workspace=None, lm=None, agent=None, sandbox=None, trust_profile=...,
  cache=None)`. `runtime.acp` adds `worker: str = "leaven serve --stdio"`.
- `cache=lv.cache.sqlite_default()` appears in spec; `[CHOICE]` `CacheConfig`
  lives in a small `cache.py` (NOT in top-level `__all__`; reached via
  `lv.cache.*`? — spec lists `cache` only inside runtime example, NOT in the
  allow-list). RESOLUTION: `lv.cache` is NOT in the top-level allow-list, so
  expose `cache` config via `runtime(cache=...)` accepting a `CacheConfig`, and
  provide builders on a non-exported `cache` module only if needed. Keep
  `CacheConfig` typed; do not add `cache` to `__all__`.
- `__all__ = ["Runtime", "environment", "runtime"]`.

### `budget.py` — owns `budget`, `Budget`

```python
class Budget(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    usd: float | None = None
    calls: int | None = None

def budget(*, usd: float | None = None, calls: int | None = None) -> Budget: ...
```
- Purpose: budget declaration. `calls=` counts metric/LM calls; agent-session
  budget is a flagged unspecified gap (note in docstring).
- `__all__ = ["Budget", "budget"]`.

### `decorators.py` — owns `runner`, `scorer`, `reflector`, `proposer`, `evaluator`, `serve`

```python
def runner[F: RolloutFn](fn: F) -> F: ...
@overload
def scorer[F: Scorer](fn: F) -> F: ...
@overload
def scorer(*, name: str) -> Callable[[F], F]: ...
def reflector[F: ReflectFn](fn: F) -> F: ...
def proposer[F: ProposeFn](fn: F) -> F: ...
def evaluator(
    *, id: str, trust_profile: TrustProfile | str = ..., granularity: str = "per_case"
) -> Callable[[EvaluatorFn], EvaluatorFn]: ...

def serve(
    *,
    rollout: RolloutFn | None = None,
    score: Scorer | Sequence[Scorer] | None = None,
    reflect: ReflectFn | None = None,
    propose: ProposeFn | None = None,
) -> None: ...
```
- Purpose: role-tagging sugar (optional in-process; load-bearing only for served
  ACP workers) + the out-of-process worker entry point.
- `scorer` is overloaded: bare `@lv.scorer` (default `__name__`) OR
  `@lv.scorer(name="...")`. Decorators do NOT register globally.
- `evaluator` is the advanced decorator (returns the fn for use in
  `Stages.evaluator(...)`); `granularity` is a free string `[CHOICE]` (wire owns
  `Granularity` enum; product passes a string).
- `serve` registers ONLY Python-authored stages; engine-mediated built-ins are
  not served.
- `__all__ = ["evaluator", "proposer", "reflector", "runner", "scorer", "serve"]`.

### `context.py` — owns `Context` (product cx Protocol) + the handle protocols

```python
@runtime_checkable
class Context(Protocol):
    lm: LmHandle
    agent: AgentHandle
    sandbox: SandboxHandle
    workspace: WorkspaceHandleProto
    async def trace(self, *events: object) -> None: ...
    def batch(self) -> BatchContext: ...

@runtime_checkable
class LmHandle(Protocol):
    async def complete(self, *args: object, **kwargs: object) -> object: ...
    async def complete_text(self, prompt: str, **kwargs: object) -> str: ...

@runtime_checkable
class AgentHandle(Protocol):
    async def run(
        self, *, workspace: WorkspaceView | None = None, instructions: str,
        output: OutputContract | None = None, **kwargs: object,
    ) -> AgentRunResult: ...

@runtime_checkable
class SandboxHandle(Protocol):
    async def exec(self, *, workspace: object, argv: Sequence[str], **kwargs: object) -> object: ...

@runtime_checkable
class WorkspaceHandleProto(Protocol):
    async def materialize_candidate(self, candidate: object) -> WorkspaceView: ...
```
- Purpose: the explicit `cx` passed to every stage fn. UNIFORM fields across all
  context types (no optional-across-boundary). `complete_text` and `agent.run`
  match the spec examples verbatim.
- `[CHOICE]`: `AgentRunResult` is a small frozen model `{parsed: Any, ...}` so
  `verdict.parsed.score` (spec line 622) type-checks. Define it here or in
  `_handles.py`; expose via `Context`/handles only.
- `BatchContext` is the `async with cx.batch() as b:` transaction handle (its
  `.workspace/.sandbox/.agent` mirror the same handles). Lives here, NOT in
  top-level `__all__`.
- `__all__ = ["AgentHandle", "Context", "LmHandle", "SandboxHandle",
  "WorkspaceHandleProto"]`. `Context` is NOT in top-level `__all__` (cx is
  passed, not imported by product users; advanced annotation uses
  `lv.adapters.contexts`).

### `_handles.py` — engine-owned views read by scorers

```python
class WorkspaceView(Protocol):
    async def read_text(self, path: str, *, missing_ok: bool = False) -> str: ...
    async def read_bytes(self, path: str, *, missing_ok: bool = False) -> bytes: ...
    def exists(self, path: str) -> bool: ...

class WorkspacePath(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    path: str

class AgentSession(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    id: str
    agent: str
    cost_usd: float = 0.0
    trajectory: TrajectoryView | None = None

class TrajectoryView(Protocol):
    def commands(self) -> Sequence[object]: ...
    def tool_calls(self) -> Sequence[object]: ...
    def messages(self) -> Sequence[object]: ...
    def files(self) -> Sequence[WorkspacePath]: ...
```
- Purpose: engine-owned read views referenced by `RolloutResult`. Exact
  trajectory normalization is downstream (spec line 1446); these are typed
  placeholders. `WorkspaceView`/`TrajectoryView` are `Protocol`s (engine-side
  concrete); `WorkspacePath`/`AgentSession` are frozen models.
- `__all__ = ["AgentSession", "TrajectoryView", "WorkspacePath", "WorkspaceView"]`.
  NONE are in top-level `__all__` (read off `RolloutResult`, not imported).

### `trust.py` — owns `TrustProfile`, `trust`

```python
class TrustProfile(StrEnum):
    trusted_local_operator = "trusted_local_operator"
    managed_sandbox = "managed_sandbox"
    package_scorer = "package_scorer"
    remote_untrusted = "remote_untrusted"

# `trust` namespace exposing the profiles ergonomically:
class _Trust:
    trusted_local_operator: TrustProfile
    managed_sandbox: TrustProfile
    package_scorer: TrustProfile
    remote_untrusted: TrustProfile
trust = _Trust()
```
- Purpose: fixed trust-profile enum + `lv.trust` ergonomic namespace.
- `[CHOICE]`: top-level exports `trust` (the namespace). `TrustProfile` is the
  enum type, importable from `lv.trust` module but NOT in top-level `__all__`
  (it is the type behind `trust_profile=` strings/values). Allow-list has
  `trust` only.
- `__all__ = ["TrustProfile", "trust"]`.

### `layouts.py` — owns `case_workspace`, `edit_artifact`, `workspace`, `Layout`

```python
class Layout(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    kind: str

def case_workspace(**kwargs: object) -> Layout: ...
def edit_artifact(**kwargs: object) -> Layout: ...
def workspace(*args: object, **kwargs: object) -> Layout: ...
```
- Purpose: stage workspace layouts passed to built-ins (`layout=`).
- `[CHOICE]`: `Layout` is a frozen marker model; `kind` discriminates. `workspace(...)`
  is the parameterized form (spec: `lv.layouts.workspace(...)`).
- `lv.layouts` is a namespace module; `__all__ = ["Layout", "case_workspace",
  "edit_artifact", "workspace"]`.

### `output.py` — owns `json`, `text`, `files`, `OutputContract`

```python
class OutputContract(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    kind: str
    path: str | None = None
    parse_as: type | None = None

def json(*, path: str | None = None, parse_as: type | None = None) -> OutputContract: ...
def text(*, path: str | None = None) -> OutputContract: ...
def files(*paths: str, **kwargs: object) -> OutputContract: ...
```
- Purpose: output contracts that tell the engine how to parse produced files
  into `RolloutResult.output`. `parse_as=` is a pydantic model type.
- `[CHOICE]`: `json` accepts both `path=` (rollout file) and bare `parse_as=`
  (judge-scorer inline, spec line 621). `text`/`files` typed similarly.
- `__all__ = ["OutputContract", "files", "json", "text"]`.

### `setup.py` — owns `bash`, `SetupStep`

```python
class SetupStep(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    kind: str
    script: str | None = None

def bash(script: str, **kwargs: object) -> SetupStep: ...
```
- Purpose: per-case setup steps (`lv.setup.bash("chmod +x ...")`).
- `__all__ = ["SetupStep", "bash"]`.

### `assets.py` — owns `path`, `AssetRef`

```python
class AssetRef(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    kind: str
    location: str

def path(location: str, **kwargs: object) -> AssetRef: ...
```
- Purpose: asset references for `Case.files` (`lv.assets.path("assets/...")`).
- `__all__ = ["AssetRef", "path"]`.

### `runs.py` — owns `open`, `Run`

```python
def open(path: str) -> Run: ...

class Run:
    best: Candidate[Any]
    frontier: list[Candidate[Any]]
    summary: RunSummary
    def test_assessments(self) -> Iterable[Assessment]: ...
    def assessment(self, case_id: str) -> Assessment: ...
    def lineage(self, candidate_id: str) -> Iterable[Candidate[Any]]: ...
    async def replay(self, case_id: str) -> ReplayResult: ...
```
- Purpose: open a completed run dir for inspection. Mirrors the `Evolved` surface
  (same `.best/.frontier/.summary/.test_assessments/.lineage/.replay`).
- `[CHOICE]`: `Run` is a distinct read-only class (not `Evolved`) since it lacks
  the live builder; shares the result types from `evolve.py`.
- `lv.runs` is the namespace; `__all__ = ["Run", "open"]`.

---

## NAMESPACE PACKAGES

### `artifacts/` — `prompt`, `directory`, `codex_kit`, `skill_bank`, `repo`, `unsafe`, `Artifact`

```python
# artifacts/__init__.py
class Artifact(BaseModel):                      # base marker for all adapters
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    kind: str

class PromptArtifact(Artifact):
    template: str
    def render(self, **kwargs: object) -> str: ...

def prompt(template: str, **kwargs: object) -> PromptArtifact: ...
def directory(root: str, *, mutable: Sequence[str], **kwargs: object) -> Artifact: ...
def codex_kit(root: str, *, mutable: Sequence[str | UnsafePath]) -> CodexKitArtifact: ...
def skill_bank(root: str, **kwargs: object) -> Artifact: ...
def repo(root: str, *, mutable: Sequence[str] | None = None, **kwargs: object) -> Artifact: ...

class UnsafePath(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    path: str
def unsafe(path: str) -> UnsafePath: ...        # warns at construction; allows out-of-surface mutable path
```
- Purpose: artifact adapters describing the mutable behavior package. `prompt`
  exposes `.template` + `.render(**input)`. `codex_kit.mutable=` REQUIRED and
  validated against the known surface (below).
- `lv.unsafe(...)` lives in `artifacts/` (`[CHOICE]`: spec uses `lv.unsafe` in
  text but it is NOT in the top-level allow-list; therefore it is reached as
  `lv.artifacts.unsafe`). Note this clearly in the docstring.
- `CodexKitArtifact` carries `root: str`, `mutable: Sequence[str]`, plus
  `.summary()` for inspection.
- `__all__ = ["Artifact", "CodexKitArtifact", "PromptArtifact", "UnsafePath",
  "codex_kit", "directory", "prompt", "repo", "skill_bank", "unsafe"]`.

#### codex_kit known surface (validated against `mutable=`)

```
default mutable (optimized unless excluded):
  AGENTS.md
  .agents/skills/**/SKILL.md
  dev_instructions.md

opt-in mutable (must be named in mutable= to be optimized):
  task_message.md
  hooks.toml
  mcp.json
  tool_policy.toml

not artifact state (frames artifact, never optimized):
  codex_kit.toml
  .codex/
```
Builder note: store these two frozensets as module constants
`DEFAULT_MUTABLE_SURFACE` and `OPT_IN_MUTABLE_SURFACE` (and
`NON_ARTIFACT_SURFACE`) in `artifacts/codex_kit.py`; `codex_kit(...)` validates
`mutable=` entries are in the known surface OR wrapped in `unsafe(...)`.

### `lm/` — `anthropic`, `openai`, `local`, `mock`, `LmConfig`

```python
# lm/config.py
@dataclass(frozen=True, slots=True)
class LmConfig:
    provider: str
    model: str | None = None
    # plus provider-specific fields via subclasses or kwargs-captured mapping

def anthropic(*, model: str, **kwargs: object) -> LmConfig: ...
def openai(*, model: str, **kwargs: object) -> LmConfig: ...
def local(*, model: str | None = None, **kwargs: object) -> LmConfig: ...
def mock(**kwargs: object) -> LmConfig: ...
```
- Purpose: provider-neutral LM config builders. `[CHOICE]`: `LmConfig` is a
  frozen dataclass (internal config, not a wire record).
- `__all__ = ["LmConfig", "anthropic", "local", "mock", "openai"]`.

### `agent/` — `codex`, `command`, `config`, `claude_code` (reserved), `opencode` (reserved), `AgentConfig`

```python
# agent/config.py
@dataclass(frozen=True, slots=True)
class AgentConfig:
    kind: str
    model: str | None = None

def codex(*, model: str | None = None, **kwargs: object) -> AgentConfig: ...
def command(argv: Sequence[str], **kwargs: object) -> AgentConfig: ...
def config(**kwargs: object) -> AgentConfig: ...
def claude_code(*args: object, **kwargs: object) -> AgentConfig:
    raise NotImplementedError("reserved scaffold name; see leaven_python.md agents section")
def opencode(*args: object, **kwargs: object) -> AgentConfig:
    raise NotImplementedError("reserved scaffold name; see leaven_python.md agents section")
```
- Purpose: agent builders. `codex` is the ONLY first-class agent. `command`/
  `config` are generic escape hatches. `claude_code`/`opencode` RESERVED, raise
  `NotImplementedError`.
- `__all__ = ["AgentConfig", "claude_code", "codex", "command", "config", "opencode"]`.

### `sandbox/` — `docker`, `local`, `SandboxConfig`

```python
@dataclass(frozen=True, slots=True)
class SandboxConfig:
    kind: str
    image: str | None = None

def docker(*, image: str, **kwargs: object) -> SandboxConfig: ...
def local(**kwargs: object) -> SandboxConfig: ...
```
- `__all__ = ["SandboxConfig", "docker", "local"]`.

### `workspace/` — `local`, `git`, `docker`, `firkin`, `WorkspaceConfig`

```python
@dataclass(frozen=True, slots=True)
class WorkspaceConfig:
    kind: str
    root: str | None = None

def local(*, root: str | None = None, **kwargs: object) -> WorkspaceConfig: ...
def git(*, root: str | None = None, **kwargs: object) -> WorkspaceConfig: ...
def docker(**kwargs: object) -> WorkspaceConfig: ...
def firkin(**kwargs: object) -> WorkspaceConfig: ...
```
- `__all__ = ["WorkspaceConfig", "docker", "firkin", "git", "local"]`.

### `cases/` — `from_jsonl`, `from_parquet`, `from_csv`, splits helpers

```python
# cases/__init__.py
def from_jsonl(path: str, *, splits: Mapping[str, slice] | None = None, **kwargs: object) -> Sequence[Case]: ...
def from_parquet(path: str, *, splits: Mapping[str, slice] | None = None, **kwargs: object) -> Sequence[Case]: ...
def from_csv(path: str, *, splits: Mapping[str, slice] | None = None, **kwargs: object) -> Sequence[Case]: ...
```
- Purpose: generic dataset loaders → `Sequence[Case]`. `splits=` maps a label to
  a `slice` (spec line 459). NO bundled benchmark catalogs.
- `cases/splits.py` holds any split-label helper utilities (`[CHOICE]`: keep
  minimal; export only if a helper is genuinely needed).
- `__all__ = ["from_csv", "from_jsonl", "from_parquet"]`.

### `optimizers/` — `gepa`, `mipro` (reserved), `textgrad` (reserved), `trace` (reserved), `Optimizer`

```python
# optimizers/__init__.py
class Optimizer(BaseModel):                     # base marker for optimizer configs
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    kind: str

def gepa(
    *,
    score: Scorer | str | CompareConfig,
    train: SamplingPolicy | str | None = None,
    validation: ValidationPolicy | str | None = None,
    population_size: int = 8,
    frontier: FrontierPolicy | None = None,
    reflective_dataset: ReflectiveDatasetHook | None = None,
    gate: GatePolicy | None = None,
    component: ComponentPolicy | None = None,
    **kwargs: object,
) -> Optimizer: ...

def mipro(*args, **kwargs) -> Optimizer:
    raise NotImplementedError("reserved optimizer name; GEPA is the only behavior-bearing optimizer in V1")
def textgrad(*args, **kwargs) -> Optimizer: raise NotImplementedError(...)
def trace(*args, **kwargs) -> Optimizer: raise NotImplementedError(...)
```
- Purpose: optimizer registry. `gepa` is the ONLY behavior-bearing optimizer.
  `score=` accepts the scorer OBJECT (typed default), a name string (convenience),
  or a `lv.gepa.compare.*` `CompareConfig`. `train=`/`validation=` accept the
  policy objects from `lv.gepa.*` OR a split-name string.
- `[CHOICE]`: defaults `population_size=8` (spec example). `reflective_dataset=`
  is the build-once-pass-down hook (engine-side).
- `__all__ = ["Optimizer", "gepa", "mipro", "textgrad", "trace"]`.

### `gepa/` — GEPA policy namespace

```python
# gepa/__init__.py re-exports the sub-namespaces:
#   sampling, validation, frontier, gate, component, compare
# plus the reflective_dataset hook type.
```

`gepa/sampling.py`:
```python
@dataclass(frozen=True, slots=True)
class SamplingPolicy: kind: str
def minibatch(*, split: str, size: int) -> SamplingPolicy: ...
def full(*, split: str) -> SamplingPolicy: ...
```
`gepa/validation.py`:
```python
@dataclass(frozen=True, slots=True)
class ValidationPolicy: kind: str
def full(*, split: str) -> ValidationPolicy: ...
def minibatch(*, split: str, size: int) -> ValidationPolicy: ...
```
`gepa/frontier.py`:
```python
@dataclass(frozen=True, slots=True)
class FrontierPolicy: kind: str
def top_k(k: int) -> FrontierPolicy: ...
def pareto(**kwargs: object) -> FrontierPolicy: ...
```
`gepa/gate.py`:
```python
@dataclass(frozen=True, slots=True)
class GatePolicy: kind: str
def improvement(*, min_delta: float = 0.0) -> GatePolicy: ...      # [CHOICE] sample gate
```
`gepa/component.py`:
```python
@dataclass(frozen=True, slots=True)
class ComponentPolicy: kind: str
def all(**kwargs: object) -> ComponentPolicy: ...                  # [CHOICE]
def named(*names: str) -> ComponentPolicy: ...                     # [CHOICE]
```
`gepa/compare.py`:
```python
@dataclass(frozen=True, slots=True)
class CompareConfig: kind: str
def weighted(weights: Mapping[Scorer, float]) -> CompareConfig: ...
def lexicographic(order: Sequence[Scorer]) -> CompareConfig: ...   # [CHOICE]
```
`gepa/reflective_dataset.py`:
```python
type ReflectiveDatasetHook = Callable[[ReflectiveContext], Awaitable[Sequence[ReflectiveCase]]]
# ReflectiveCase/ReflectiveContext are adapters types; this is the hook alias.
```
- Purpose: GEPA-namespaced policy. `lv.gepa.compare.weighted({correctness: 0.8,
  trajectory_quality: 0.2})` keys by SCORER OBJECT (spec line 1143).
- Each submodule ships sorted `__all__`. `lv.gepa.__init__` `__all__` lists the
  sub-namespace module names: `["compare", "component", "frontier",
  "reflective_dataset", "sampling", "validation", "gate"]` — sorted:
  `["compare", "component", "frontier", "gate", "reflective_dataset", "sampling", "validation"]`.

---

## ADVANCED + WIRE RINGS

### `adapters/` — advanced authoring

`adapters/__init__.py` re-exports:
`Evaluator` support, `RegisteredStage`, typed contexts, reflective types.

`adapters/contexts.py`:
```python
class RunContext(Context, Protocol): ...        # typed product cx for runner/scorer/reflector/proposer
class StageContext(Context, Protocol): ...       # alias-shaped; uniform fields
class EvalContext(Context, Protocol):            # adds batched-effect surface
    def batch(self) -> BatchContext: ...
    def submit(self, write: AssessmentWrite) -> None: ...
```
- UNIFORM fields rule: every context type carries the same fields (`lm/agent/
  sandbox/workspace/trace/batch`). `EvalContext` adds `submit` (the only delta,
  and it is present on the eval boundary, not optional across boundaries — a
  runner simply never receives an `EvalContext`).
- `__all__ = ["EvalContext", "RunContext", "StageContext"]`.

`adapters/registered_stage.py`:
```python
class RegisteredStage(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    role: StageRole          # wire.StageRole
    name: str
    fn: Callable[..., Awaitable[object]]
```
- `__all__ = ["RegisteredStage"]`.

`adapters/evaluator.py`:
```python
type EvaluatorFn = Callable[[EvaluationJob, EvalContext], Awaitable[None]]
class Evaluator(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    id: str
    trust_profile: TrustProfile | str
    granularity: str
    fn: EvaluatorFn
```
- Purpose: support types for `@lv.evaluator`. (`EvaluationJob`/`AssessmentWrite`
  imported from `lv.wire`.)
- `__all__ = ["Evaluator", "EvaluatorFn"]`.

`adapters/reflective.py`:
```python
class TraceRef(BaseModel):                       # handle to heavy trajectory data
    model_config = ConfigDict(frozen=True, extra="forbid")
    id: str
class Attachment(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    name: str
    ref: TraceRef
class ReflectiveRun(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    output: Any
    score: float
    feedback: str = ""
    sessions: Sequence[str] = ()                 # by handle
    trajectory: TraceRef | None = None           # by handle (heavy data not inlined)
class ReflectiveCase(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    input: Mapping[str, Any]
    expected: Mapping[str, Any] | None = None    # target-safe
    runs: Sequence[ReflectiveRun] = ()
class ReflectiveBatch(BaseModel):                # product-facing pre-built batch
    model_config = ConfigDict(frozen=True, extra="forbid")
    cases: Sequence[ReflectiveCase] = ()
class ReflectiveContext(BaseModel):              # input to reflective_dataset hook
    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)
    cases: Sequence[ReflectiveCase] = ()
```
- Purpose: typed reflection records. `batch` in a `ReflectFn` is a
  `ReflectiveBatch`; `batch.cases[i].input/.expected`, `.runs[j].output/.score/
  .feedback/.sessions/.trajectory` per spec lines 681-688. Heavy data by
  `TraceRef`. These uphold build-once-pass-down (target-safe; `feedback` is the
  only target-derived channel).
- `[CHOICE]` `ReflectiveBatch`/`ReflectiveContext` named here. `ReflectiveBatch`
  re-exported to top of `adapters` for `ReflectFn` annotation.
- `__all__ = ["Attachment", "ReflectiveBatch", "ReflectiveCase",
  "ReflectiveContext", "ReflectiveRun", "TraceRef"]`.

`adapters/__init__.py` `__all__` (sorted):
`["Attachment", "EvalContext", "Evaluator", "EvaluatorFn", "ReflectiveBatch",
"ReflectiveCase", "ReflectiveContext", "ReflectiveRun", "RegisteredStage",
"RunContext", "StageContext", "TraceRef"]`.

### `wire/` — generated public-seam schema records (pydantic frozen, extra="forbid")

All wire records are pydantic v2 frozen, `extra="forbid"`, snake_case fields, no
`null` literals where the seam forbids them, JS-safe ints. Builders treat fields
as the minimal typed projection; exact schema is owned by
`docs/specs/public-seam-v1/schemas/`.

`wire/visibility.py`:
```python
class Visibility(StrEnum): public / private / trace
```
`wire/output_record.py`:
```python
class OutputRecord(BaseModel): kind: str; path: str | None; visibility: Visibility = Visibility.public
```
`wire/evaluation_job.py`:
```python
class Granularity(StrEnum): per_case / aggregate
class Purpose(StrEnum): rollout / score / reflect / propose / judge
class EvaluationItem(BaseModel): case_id: str; candidate: object
class EvaluationJob(BaseModel): id: str; items: Sequence[EvaluationItem]; granularity: Granularity; purpose: Purpose
```
`wire/evidence.py`:
```python
class EvidencePublic(BaseModel): ...
class EvidencePrivate(BaseModel): ...
class EvidenceEnvelope(BaseModel): visibility: Visibility; public: EvidencePublic | None; private: EvidencePrivate | None; target_derived: bool = False
```
`wire/assessment.py`:
```python
class Replayability(StrEnum): replayable / non_replayable
class AssessmentWrite(BaseModel):
    case_id: str; value: float; feedback: str = ""; replayability: Replayability = Replayability.replayable
    @classmethod
    def independent_case(cls, *, case_id: str, value: float, feedback: str = "") -> AssessmentWrite: ...
```
`wire/proposal.py`:
```python
class ProposalEffect(StrEnum): create / change
class ProposalBatch(BaseModel): proposals: Sequence[object]; effect: ProposalEffect
```
`wire/stage_payloads.py`:
```python
class StageRole(StrEnum): runner / scorer / reflector / proposer / evaluator
class StageSourceRef(BaseModel): stage: StageRole; receipt_id: str
class ReflectExample(BaseModel): input: Mapping[str, Any]; output: Any; feedback: str = ""; source: StageSourceRef | None = None
class ReflectRequest(BaseModel): examples: Sequence[ReflectExample]; parent: object | None = None
class ReflectionResult(BaseModel): summary: str; failure_modes: Sequence[str] = (); suggestions: Sequence[str] = (); constraints: Sequence[str] = ()
class ProposeRequest(BaseModel): parent: object; reflection: ReflectionResult; reflector_receipt: StageSourceRef | None = None
class JudgeRequest(BaseModel): items: Sequence[object]; rubric: str | None = None
```
`wire/receipts.py`:
```python
class QueryReceipt(BaseModel): request_hash: str; result_hash: str; kind: str; revision: str; policy_fingerprint: str; elapsed_ms: int = 0
class CallReceipt(BaseModel):  request_hash: str; result_hash: str; kind: str; revision: str; policy_fingerprint: str; cost_usd: float = 0.0; elapsed_ms: int = 0
class WriteReceipt(BaseModel): request_hash: str; result_hash: str; kind: str; revision: str; policy_fingerprint: str; elapsed_ms: int = 0
```
- Purpose: the public-seam wire records. ALL forbidden from top-level `__all__`.
- Each module ships sorted `__all__`. `wire/__init__.py` `__all__` (sorted):
  `["AssessmentWrite", "CallReceipt", "EvaluationItem", "EvaluationJob",
  "EvidenceEnvelope", "EvidencePrivate", "EvidencePublic", "Granularity",
  "JudgeRequest", "OutputRecord", "ProposalBatch", "ProposalEffect",
  "ProposeRequest", "Purpose", "QueryReceipt", "ReflectExample", "ReflectRequest",
  "ReflectionResult", "Replayability", "StageRole", "StageSourceRef",
  "Visibility", "WriteReceipt"]`.

### `_engine/` — private placeholder

`_engine/__init__.py`: `__all__ = []`. No user reach. Holds future private engine
helpers; ships empty in the scaffold.

---

## `x/` — external-ecosystem adapters

`x/__init__.py`: namespace exposing `dspy`, `verifiers`, `harbor` sub-namespaces.
`__all__ = ["dspy", "harbor", "verifiers"]`.

`x/dspy/__init__.py`:
```python
class LeavenDSPyLM:                              # subclass of dspy.BaseLM (import guarded)
    def __init__(self, *, model: str, **kwargs: object) -> None: ...
def artifact(*, program: object, **kwargs: object) -> Artifact: ...   # lowers DSPy program param-state into a Leaven artifact
```
- `__all__ = ["LeavenDSPyLM", "artifact"]`.

`x/verifiers/__init__.py`, `x/harbor/__init__.py`: reserved adapter namespaces;
`__all__ = []` in the scaffold (members raise `NotImplementedError` if any are
named). `[CHOICE]`: ship as empty stub packages so `lv.x.verifiers` /
`lv.x.harbor` import without error.

---

## DELETIONS (hard cutover — remove these stale modules)

- `environment.py` → folded into `runtime.py`
- `optimize.py` → `evolve.py` (`optimize` becomes deprecated alias in `evolve.py`)
- `scoring.py` + old `score.py` → consolidated `score.py`
- `agent_instructions.py` → folded into `agent/` or dropped
- `evaluation_job.py`, `output_record.py`, `evidence.py`, `assessment.py`,
  `proposal.py`, `stage_payloads.py`, `_receipts.py` → `wire/`
- `data_class.py` → `wire/` (if it holds data-class/visibility) or drop
- `frontier.py` → `gepa/frontier.py`
- `result.py` → `evolve.py`
- `builders/` → fold into the relevant namespaces or delete
- `context.py` keeps the product `Context`; the typed `RunContext/StageContext/
  EvalContext` MOVE to `adapters/contexts.py`.

---

## SURFACE TESTS the build must satisfy (named gates, not test names)

1. Top-level `lv.__all__` equals the allow-list exactly (both directions: all
   allow-list names exported; no forbidden name present).
2. `inspect.signature(Stages.__init__)` parameters are exactly
   `{self, rollout, score, reflect, propose}` (keyword-only for the four slots).
3. Every public module defines a sorted `__all__` (RUF022).
4. `optimizers.mipro/textgrad/trace`, `agent.claude_code/opencode` raise
   `NotImplementedError`.
5. `environment` and `optimize` emit `DeprecationWarning`.
6. No top-level symbol requires importing from a private (`_`-prefixed) module.
7. Context types share a uniform field set (no field present on one context type
   and absent on another except the documented `EvalContext.submit`/`batch`
   advanced surface, which is gated by boundary, not optionality).
