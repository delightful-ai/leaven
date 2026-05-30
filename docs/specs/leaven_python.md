# Leaven Python

Status: governing product spec.
Created: 2026-05-24.
Revised: 2026-05-25 — replaced the six-decorator authoring model with the
four-stage transform model, added Task/Case/Rollout/RolloutResult/Stages as
top-level nouns, renamed `environment` to `runtime`, made Codex the default
agent with explicit dual-role framing, made GEPA the only behavior-bearing
optimizer, and evicted engine/wire nouns from the top-level surface.

## What this is

Leaven Python is the way you use Leaven. You write Python that configures
and drives a full Leaven optimization run end-to-end — runtime, optimizer,
stages, run, inspection — and the result is a typed, replayable, auditable
optimization with every safety property of the locked public seam preserved
across the wire. Capability tokens are real. Data-class propagation is
enforced. Receipts are audit currency, not log decoration. Replay is
per-assessment honest.

Python users do not learn Rust to use Leaven. The small set of people who
write optimizer crates, workspace backends, agent runtime adapters, and
artifact semantics work in Rust. Everyone else — the people who compose
runs, write stages, inspect results, reproduce papers, do the actual
research — works in Python. The Python surface is not an afterthought
added because someone wanted bindings. It is the product. Rust is the
substrate.

## What this is not

It is not a stage-authoring kit. A user who can only write a Python
`@lv.scorer` and has to drop into Rust to compose a run does not have the
product. The user composes the whole thing from Python or this surface
has failed.

It is not a pyo3 wrapper of `leaven-run`. The
`docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md`
named the reasons at line 29 — manylinux wheel matrix, Python ABI
versioning, GIL + Tokio integration, language-locks Leaven forever. The
decision is durable.

It is not a wrapper around `leaven-run`. `leaven-run` is the Rust
product-builder API; Leaven Python is the Python product-builder API.
They are peers that share the same engine, not a stack.

It is not the same as the public seam V1 spec. The public seam V1 spec
locks the wire contract for external-language workers — plan IR, capability
tokens, result receipts, stage payloads, evidence envelopes, the Leaven
ACP profile, JSON schemas. This spec uses that wire and adds the Python
projection on top of it. If this spec and `docs/specs/public-seam-v1/`
disagree about the wire, the seam wins.

## Why this exists

In Rust, Leaven is currently hard to set up. Environment setup bugs.
Topology that needs to be held in working memory. Trait bounds that fail
in non-obvious places. The user surfaced this directly during the design
conversation that produced this spec: *"in Rust, Leaven is not usable.
It's really fucking hard to set up, dude."*

This is the load-bearing motivation. If Rust is hard AND Python is
afterthought, the usability problem isn't solved. The Python surface is
not "let's add bindings" — it is "let's make Leaven actually usable for
the people who do the research." Everything else in this spec serves
that.

## The core law

Leaven Python composes one sentence:

```text
evolution = artifact × task × stages × optimizer × runtime
```

Each noun has one job:

```text
artifact   the mutable behavior package being evolved
task       the immutable task world (cases, files, setup, sandbox needs, splits)
stages     the four swappable evolution transforms
optimizer  search policy: when stages run, which evidence flows, how children admit
runtime    execution substrate: workspaces, sandboxes, agents, budget, sessions
```

The four stage transforms are:

```text
rollout    current artifact + selected case  → completed attempt (RolloutResult)
evaluate   completed attempt + case          → named Scores
reflect    scored evidence + selection       → critique
propose    parent artifact + critique        → child artifact (proposal)
```

These four are the **only** members of `Stages`. Anything else that wants
to look like a stage — scorers, layouts, output contracts, samplers,
gates, validation policies, reflective-dataset builders — is a component
of one of those four, or a piece of optimizer policy, not a stage.

The artifact-behavior law decides what is artifact state and what is not:

```text
if the optimizer can change it, it belongs in the artifact.
if it is fixed execution machinery, it belongs in rollout/runtime/task config.
```

Concretely, for an agent harness:

```text
artifact     target/current/{AGENTS.md, skills/, task_message.md, hooks.toml,
             dev_instructions.md, run.py, prompts/, tools.toml, ...}

not artifact case input, hidden target, scoring rubric, rollout command
             wrapper, workspace layout, runtime provider config,
             API credentials, budget/trust policy
```

The engine — graph, receipts, evidence, replay, target isolation — owns
proof. It is not narrated to the product API.

## What the user writes

The minimal full Leaven program in Python is composition + stages + run.
The smallest meaningful sketch is ~25 lines:

```python
import leaven as lv

task = lv.Task(
    cases=[
        lv.Case(id="q1", input={"question": "what is 6*7?"},
                target={"answer": "42"}, split="train"),
        lv.Case(id="q2", input={"question": "what is 9*9?"},
                target={"answer": "81"}, split="val"),
    ],
)

artifact = lv.artifacts.prompt(
    "Answer the question. Return only the answer.\n\nQuestion: {question}"
)

@lv.runner
async def run(artifact, case, cx) -> str:
    return (await cx.lm.complete_text(artifact.render(**case.input))).strip()

@lv.scorer
async def correctness(run: lv.RolloutResult[str], case, cx) -> lv.Score:
    return lv.Score(
        value=float(run.output == case.target["answer"]),
        feedback=f"got {run.output!r}; expected {case.target['answer']!r}",
    )

result = await lv.evolve(
    artifact=artifact,
    task=task,
    stages=lv.Stages(
        rollout=run,
        score=correctness,
        reflect=lv.Reflect.agent(lv.agent.codex()),
        propose=lv.Propose.agent_edit(lv.agent.codex()),
    ),
    optimizer=lv.optimizers.gepa(score=correctness),
    runtime=lv.runtime.local(budget=lv.budget(usd=20)),
).run()

print(result.best.artifact.template)
```

Stages are plain async functions. `rollout` and `score` are functions
you write; `reflect` and `propose` here use the declarative Codex-backed
built-ins because this example has no custom reflection logic. The
decorators (`@lv.runner`, `@lv.scorer`) tag the role and name the scorer
— optional sugar (a bare function in the slot works too; its name is its
`__name__`). Agentic anything (an LLM-judge scorer, a tool-using
rollout) is just `cx.agent.run(...)` or `cx.lm.complete(...)` *inside*
the function; there is no `Scorer.agent` or `Scorer.fn` constructor.

The shape is the same at any scale. The EvoSkill-shaped paper repro is
the same composition with a richer artifact, agent-backed rollout, and a
sandboxed task world — the composition glue does not grow with the
paper's complexity; the stage bodies and the artifact do.

A Codex-harness example:

```python
import leaven as lv
from pydantic import BaseModel

class Answer(BaseModel):
    answer: str

task = lv.Task(
    cases=[
        lv.Case(
            id="ctf-001",
            input={"instructions": "find the flag."},
            target={"flag": "picoCTF{...}"},
            files={"challenge": lv.assets.path("assets/challenge")},
            setup=lv.setup.bash("chmod +x case/files/challenge"),
            split="train",
        ),
    ],
    sandbox=lv.sandbox.docker(image="python:3.12"),
)

artifact = lv.artifacts.codex_kit(
    "./agent_kit",
    mutable=[
        "AGENTS.md",
        ".agents/skills/**/SKILL.md",
        "task_message.md",
        "hooks.toml",
        "dev_instructions.md",
    ],
)

rollout = lv.Rollout.agent(
    lv.agent.codex(),
    layout=lv.layouts.case_workspace(),
    output=lv.output.json(path="output/result.json", parse_as=Answer),
)

@lv.scorer
async def correctness(run: lv.RolloutResult[Answer], case, cx) -> lv.Score:
    log = await run.workspace.read_text("output/run.log", missing_ok=True)
    return lv.Score(
        value=float(run.output.answer == case.target["flag"]),
        feedback=f"answer={run.output.answer!r}; "
                 f"sessions={len(run.sessions)}; log={log[:200]!r}",
    )

result = await lv.evolve(
    artifact=artifact,
    task=task,
    stages=lv.Stages(
        rollout=rollout,
        score=correctness,
        reflect=lv.Reflect.agent(lv.agent.codex()),
        propose=lv.Propose.agent_edit(
            lv.agent.codex(),
            layout=lv.layouts.edit_artifact(),
        ),
    ),
    optimizer=lv.optimizers.gepa(score=correctness),
    runtime=lv.runtime.local(budget=lv.budget(usd=50)),
).run()
```

Here `rollout`, `reflect`, and `propose` are all engine-mediated Codex
(declarative built-ins — no Python logic to run); the only Python the
engine calls back into is the `correctness` scorer. That is the
codex_kit MVP shape: evolve Codex-shaped behavior, and the sole
custom-Python surface is how you judge the attempt.

Notice what is absent: `OutputRecord`, `RunCase`, `ScoreCase`,
`EvaluationJob`, `Granularity`, `Purpose`, `QueryReceipt`,
`EvidenceEnvelope`, `RegisteredStage`. Those exist — they are part of the
engine and the wire — but they are floorboard creatures, not product
nouns.

## What the user does not write

The user does not write the optimizer loop. They configure
`lv.optimizers.gepa(...)` from a registry by passing typed configs to
`lv.evolve(...)`. The optimizer loop runs in Rust. New optimizers
require a new Rust crate (`leaven-textgrad`, `leaven-trace`, future
entries), not Python authoring.

This is not a temporary limitation. Optimizer strategy state binds
tightly to engine state — parent selectors mutate frontiers, samplers
consume populations, reflection requests cross trust boundaries with
typed envelopes. Authoring optimizer rhythm from Python would either
require exposing engine internals through the wire (defeating the
seam's separation) or accepting that Python optimizers are second-class
(defeating the equal-power promise).

The user does not write graph mutation. `RunContext` is the only graph
mutation authority and it is engine-side. Python authors emit typed
assessment and proposal envelopes; the engine applies them through
`RunContext` on the Rust side. This is per the locked seam.

The user does not write capability minting, trust ledgers, or budget
accounting. They declare `trust_profile="managed_sandbox"`,
`input_classes=[...]`, `forbidden_input_classes=[...]`,
`budget=lv.budget(usd=200)`, and the engine enforces. The declarations
cross the wire; the enforcement is engine-side.

The user does not write paper-close bring-up code (refusal cascades,
source pinning, manifest validators, fingerprint binders). That domain
work currently lives in Rust (~9,000 LOC for a paper like EvoSkill) and
will continue to live there in V1. A future V2 may add typed bring-up
primitives to the Python SDK; V1 does not.

## The wire

The wire is the locked Leaven ACP profile per
`docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`.
Stdio JSON-RPC, kind-discriminated JSON Schema payloads, capability
tokens via env-var bearer + fingerprint, session-bound permission
decisions, bounded progress updates, cancellation-with-receipts.

Leaven owns this wire. The implementation is in-house, in a Leaven crate
(`leaven-acp`). It does not depend on the upstream `agent-client-protocol`
SDK. The reasons are:

- **Schema lock.** Leaven's seam is schema-locked at
  `docs/specs/public-seam-v1/schemas/`; conformance is gated by
  `docs/specs/public-seam-v1/conformance-matrix.yaml`. The MCP-style
  schemaless-drift failure mode is inverted by construction. Inheriting
  an upstream SDK risks inheriting its release cadence and its
  abstractions for things Leaven explicitly does not have (proxy chains,
  protocol-version negotiation, MCP-over-ACP, dynamic handler
  registration).
- **Surface size.** The SDK is ~13,500 LOC; the in-house Leaven version
  is ~1,200 LOC because Leaven omits ~6,500 LOC of features it does not
  consume.
- **Multi-language safety.** Owning the wire keeps the schema-codegen
  pipeline uniform across future TS / Go / shell SDKs.

The wire is stable across language SDKs. The Python SDK speaks it. A
future TypeScript SDK will speak the same wire. Multi-language safety is
verified by the wire's properties (snake_case, no `null` literals,
JS-safe integers, opaque cursors) — not by hoping the Python
implementation generalizes.

The CLI consumes this wire too. The same `leaven` binary that the Python
SDK spawns as a child process via `leaven serve --stdio` also offers
`leaven query lineage`, `leaven runs list`, `leaven artifact show` as
human-facing subcommands.

## What is preserved

The Python SDK does not relax any seam property. The user writes Python;
the wire carries:

- **Capability tokens.** Opaque bearer at the env-var boundary, resolved
  to structured capability documents on the engine side, enforced per
  call through grant-envelope authorization.
- **Data-class propagation.** Monotonic; forbidden intersections deny
  calls before host execution. Reflector LM calls cannot egress
  `case.target`. Declared input classes must cover actual dependency
  classes.
- **Receipts as audit currency.** Query, call, and write receipts bind
  request hashes, result hashes, operation kind, timing, policy
  fingerprint, revision. Decorative receipts are rejected.
- **Per-assessment replayability.** A plan-level replayable flag cannot
  hide one non-replayable assessment. Roll-up only.
- **Failed costs counted.** Failed calls that spend money still produce
  charge receipts.
- **Stage payload typing.** Reflector / proposer / runner / scorer
  payloads have semantic owners with cross-stage binding (reflector
  example source refs carried by the request; proposer cites reflector
  receipt; etc.).
- **Evidence visibility.** Public / private / trace projection data
  classes preserved through the wire. Target-derived evidence flagged
  honestly. Source receipts present and typed.
- **Replay determinism.** Receipts plus inputs plus locked seam
  validation reproduce the assessment.

Whatever the Python user declares about input classes, trust profile,
output contracts, sandbox requirements — the engine enforces. Whatever
the Python user wires for cases, candidates, runs — the engine remembers
in typed receipts. The Python surface is the ergonomic projection of the
seam; it is not an alternative trust path.

## The Python authoring surface

The product surface is small and named:

```text
Task, Case           the task world
artifact adapters    prompt, directory, codex_kit, skill_bank, repo, ...
Score                the tiny scorer return value (value + feedback)
RolloutResult[Out]   completed attempt (what a scorer/reflector reads)
Stages               the four-slot composition: rollout / score / reflect / propose
Runtime              .local / .acp / ...
evolve               the entry point
```

Stages are **plain async functions** you write; the `Stages` slots name
the role. Two decorators tag a function with its role (optional sugar,
and required only when the function is served out-of-process as an ACP
worker): `@lv.runner`, `@lv.scorer`, `@lv.reflector`, `@lv.proposer`.

For stages with **no custom Python logic** — "just run Codex against the
artifact," "just run this command," "just let Codex edit the artifact" —
there are declarative built-ins instead of a function:

```text
lv.Rollout.agent(agent, ...)     engine runs the agent against the artifact
lv.Rollout.command(argv, ...)    engine runs a command against the projected artifact
lv.Rollout.manifest(path, ...)   engine reads invocation from a file in the artifact
lv.Reflect.agent(agent, ...)     engine-mediated reflection
lv.Propose.agent_edit(agent, ...) engine-mediated artifact edit + readback
```

A slot accepts **either** a function (custom logic; agentic work via
`cx` primitives inside) **or** one of these built-ins. There is no
`Scorer.fn` / `Scorer.agent` / `Evaluate` constructor — scoring is just
named functions in the `score` slot (one, or a list).

Plus namespaces:

```text
lv.artifacts.*       artifact adapters
lv.optimizers.*      optimizer registry (gepa today; mipro/textgrad/trace reserved)
lv.lm.*              LM provider builders
lv.agent.*           agent builders (codex is the only first-class agent today)
lv.sandbox.*         sandbox builders
lv.layouts.*         stage workspace layouts
lv.output.*          output contracts (json / text / files / parse_as)
lv.runs              run inspection (`lv.runs.open(path)`)
lv.x.*               external-ecosystem adapters (dspy, verifiers, harbor, ...)
```

### Task and Case

`lv.Task` declares an immutable task world. It owns the case inventory
and any task-global runtime requirements (sandbox, splits).

```python
task = lv.Task(
    cases=[
        lv.Case(
            id="ctf-001",
            input={"instructions": "find the flag."},
            target={"flag": "picoCTF{...}"},
            files={"challenge": lv.assets.path("assets/challenge")},
            setup=lv.setup.bash("chmod +x case/files/challenge"),
            split="train",
            metadata={"difficulty": "medium"},
        ),
    ],
    sandbox=lv.sandbox.docker(image="python:3.12"),
)
```

`Task` is inert. It does not allocate workspaces, does not own layout,
does not enforce splits — it declares facts the optimizer and the
runtime read.

Splits are **user-defined labels**, not a fixed train/val/test enum.
`split=` on a `Case` is any string the user chooses; the optimizer
references splits by name (`gepa(train="train", validation="held_out")`).
Leaven does not impose the three-way split; a user with `"dev"`,
`"smoke"`, `"full"` splits wires those names through. A case carries one
split label. (If multi-membership ever proves necessary, it is an
additive change; V1 keeps one-label-per-case for legibility.)

Cases project differently depending on which stage consumes them. A
rollout sees `case.id`, `case.input`, materialized files; a scorer sees
the same plus `case.target` and rubric metadata; reflect/propose see the
projection allowed by visibility policy. The product API does not name
those projections — they are internal to the engine. Public users always
write `lv.Case`.

Loader sugar still works for big datasets:

```python
task = lv.Task(cases=lv.cases.from_jsonl("aime.jsonl", splits={
    "train": slice(0, 80), "val": slice(80, 100),
}))
```

### Artifact adapters

Artifact adapters describe the mutable behavior package:

```python
artifact = lv.artifacts.prompt("Answer: {question}")
artifact = lv.artifacts.directory("./harness", mutable=["prompts/**", "run.py"])
artifact = lv.artifacts.skill_bank("./skills")
artifact = lv.artifacts.codex_kit("./agent_kit", mutable=[...])
artifact = lv.x.dspy.artifact(program=dspy_program)
```

Each adapter knows its own:

- identity / fingerprint
- projection into a workspace
- read-back of changes into a typed artifact-native diff
- mutable-paths contract (what counts as legitimate change)

`codex_kit` is described in its own section below; it is the flagship
off-the-shelf harness.

### Rollout

Rollout is the interpretation of the current artifact on one case. It is
**either a function you write, or a declarative built-in** for the cases
that need no custom Python.

The function form: an async function `(artifact, case, cx)` that returns
`Out` (or a `RolloutResult`). The engine wraps a bare `Out` into a
`RolloutResult`. Agentic/tool-using rollouts call `cx.*` primitives
inside:

```python
@lv.runner
async def run(artifact, case, cx) -> str:
    return (await cx.lm.complete_text(artifact.render(**case.input))).strip()

# in Stages: rollout=run
```

The declarative built-ins, for "no Python logic, just run the artifact":

```python
# Engine runs an agent (Codex) against the projected artifact. No Python
# rollout body — the artifact IS the behavior; the agent executes it.
lv.Rollout.agent(agent, *, layout=None, output=None, instructions=None)

# Engine runs a command against the projected artifact workspace.
lv.Rollout.command(argv, *, layout=None, output=None, cwd=None, env=None)

# Engine reads the invocation from a file inside the artifact, so the
# rollout shape itself is mutable artifact state.
lv.Rollout.manifest(path, *, layout=None, output=None)
```

```python
rollout = lv.Rollout.agent(
    lv.agent.codex(),
    layout=lv.layouts.case_workspace(),
    instructions="Solve the case in target/current. Write output/result.json.",
    output=lv.output.json(path="output/result.json", parse_as=Answer),
)
```

The built-ins' `instructions=` is the **stable invocation envelope**;
the *mutable* instructions (the thing being optimized) live in the
artifact. The output contract (`output=`) says how the engine parses the
produced file(s) into the `RolloutResult.output` the scorer reads.

Whether function-backed or built-in, a rollout produces a
`RolloutResult`. The difference is only who runs the body: your Python
(function) or the engine + agent/command (built-in).

### RolloutResult

The scorer receives the completed attempt, not just the output:

```python
class RolloutResult[Out]:
    output: Out
    workspace: WorkspaceView          # live, readable; cleanup deferred until scoring done
    sessions: Sequence[AgentSession]  # every engine-mediated agent session in this rollout
    trajectory: TrajectoryView        # normalized commands / tool calls / messages / files
    output_files: Sequence[WorkspacePath]
    status: RolloutStatus             # ok | timeout | crash | refused | budget_exceeded | ...
```

Scorers consume it:

```python
async def correctness(run: lv.RolloutResult[Answer], case, cx) -> lv.Score:
    log = await run.workspace.read_text("output/run.log", missing_ok=True)
    return lv.Score(
        value=float(run.output.answer == case.target["answer"]),
        feedback=f"sessions={len(run.sessions)}; status={run.status}; log={log[:200]!r}",
    )
```

A function rollout that returns `Out` produces a `RolloutResult[Out]`
with `sessions=[]`, `trajectory` reduced to whatever the function
recorded via `cx.trace(...)`, and `output_files=[]`. Command/agent
rollouts populate sessions and trajectory from the engine-mediated
execution.

For advanced cases, a function rollout may return `RolloutResult` itself
to attach extra captured state:

```python
return lv.RolloutResult(output=answer, trace=["parsed", "verified"])
```

This is opt-in; the common path is `return Out`.

### Scorer and Score

`Score` stays tiny:

```python
class Score(BaseModel, frozen=True):
    value: float
    feedback: str = ""
```

No `metrics` dict, no `output` field, no evidence blob. A `Score` is a
judgment: a number and why. Multiple objectives are **multiple named
scorers**, not a heavier `Score`.

A scorer is **just a function**. There is no `Scorer` constructor to
call; `Scorer` is only a type alias for the callable shape
(`Callable[[RolloutResult, Case, Context], Awaitable[Score]]`, exported
for annotations). The canonical form:

```python
@lv.scorer
async def correctness(run: lv.RolloutResult[Out], case: lv.Case, cx) -> lv.Score:
    return lv.Score(value=..., feedback=...)
```

A scorer carries a **name** — its `__name__` by default, or
`@lv.scorer(name="...")` to override. The name is how the optimizer and
the run report refer to this score. The name lives on the scorer, not in
an external dict key, so there is no stringly-typed cross-reference to
keep in sync.

An **agentic** scorer — an LLM-judge, or a verifier that runs in the
final environment — is the same function with `cx.agent.run(...)` /
`cx.lm.complete(...)` inside it. There is no separate constructor for
"agentic scoring"; you have `cx`, you compose:

```python
@lv.scorer
async def judged(run: lv.RolloutResult[Answer], case, cx) -> lv.Score:
    verdict = await cx.agent.run(
        workspace=run.workspace,                      # judge sees the final env
        instructions=f"Grade against rubric: {case.target['rubric']}",
        output=lv.output.json(parse_as=Verdict),
    )
    return lv.Score(value=verdict.parsed.score, feedback=verdict.parsed.reason)
```

Sugar return shapes: a scorer may return `float` or `bool`; the engine
normalizes to `Score(value=..., feedback="")`.

### The `score` slot

There is no `Evaluate` stage. "Apply scorers to a completed attempt" is
not a transform peer to rollout/reflect/propose — it is just *the
scorers*. So `Stages` carries `score` directly: one scorer, or a list of
named scorers.

```python
score = [correctness, trajectory_quality, safety_check]   # typed: Sequence[Scorer]
# or just:
score = correctness
```

Each entry is a named scorer function (the name is the scorer's, not a
dict key). The optimizer references the primary score **by the scorer
object** — `gepa(score=correctness)` — so a rename or a typo is caught by
the type checker, not discovered at runtime. (`gepa(score="correctness")`
by name string is accepted as a convenience, but the object form is the
typed default.) This is Inspect's `scorer=[accuracy(), f1()]` shape: a
list of self-named scorers, no dict, no stringly cross-reference.

Sampling, split selection, batching, and aggregation are **not** here —
they are optimizer policy. The `score` slot only says "given a completed
`RolloutResult` and `Case`, here are the named judgments to compute."

For the rare evaluator that needs multi-op batched effects or custom
evidence shapes beyond per-case named scores, the advanced
`@lv.evaluator` decorator replaces the `score` slot entirely; see
[Advanced authoring](#advanced-authoring-evaluator) below.

### Reflect

Reflect turns a minibatch of scored attempts into a critique. Like
rollout, it is **either a function or a declarative built-in**:

```python
reflect = lv.Reflect.agent(lv.agent.codex())     # engine-mediated (the MVP default)

@lv.reflector                                      # custom Python
async def reflect(batch, parent, cx) -> lv.Critique: ...
```

The load-bearing rule — **build-once-pass-down**: the optimizer
(engine-side) constructs the reflective batch *once*, target-safe, and
hands it to reflect already built. The reflect function does **not**
query run history to assemble its own evidence. This is what keeps
LM-backed and agent-backed reflectors seeing byte-identical input, and
what keeps hidden targets out of reflection by construction.

So `batch` is a pre-built, target-safe view: a minibatch of cases, each
with its scored runs. The product shape:

```text
batch.cases[i].input              target-safe input projection (never the raw target)
batch.cases[i].expected           optional, target-safe
batch.cases[i].runs[j].output     what the rollout produced
batch.cases[i].runs[j].score      the scorer's value
batch.cases[i].runs[j].feedback   the scorer's feedback (the only target-derived channel)
batch.cases[i].runs[j].sessions   agent sessions, by handle
batch.cases[i].runs[j].trajectory by handle (TraceRef) — heavy data is not inlined
```

The detailed typed records (`ReflectiveCase`, `ReflectiveRun`,
`Attachment`, the `TraceRef` handles) live in `lv.adapters`; ordinary
reflect code reads `batch` and returns a `Critique`. Reflect does **not**
produce a candidate change — that is Propose's job.

**Customizing what reflection sees** — which cases are sampled, which
examples are featured, how the dataset is projected — is an
**optimizer-config hook**, not logic inside the reflector:

```python
optimizer = lv.optimizers.gepa(
    score=correctness,
    reflective_dataset=my_projection,   # case evidence → ReflectiveCase records
)
```

This preserves build-once-pass-down: the projection runs engine-side as
part of GEPA policy; the reflector still receives a finished batch.

### Propose

Propose turns a parent artifact + the reflector's critique into a child
artifact. Either a built-in or a function:

```python
propose = lv.Propose.agent_edit(                   # engine-mediated (the MVP default)
    lv.agent.codex(),
    layout=lv.layouts.edit_artifact(),
)

@lv.proposer                                        # custom Python
async def propose(parent, reflection, cx) -> lv.Proposal: ...
```

`Propose.agent_edit` materializes the parent artifact under
`target/current/`, runs an agent (Codex) as editor with the critique
attached, and reads the workspace back as a typed artifact-native
change. The artifact adapter's `mutable=[...]` paths define what edits
are admissible; edits outside that surface are rejected on readback.

Propose receives the reflector's **digested** output (a `ReflectionResult`
— summary, failure modes, suggestions, constraints), **not** the raw
example batch again. Reflect and propose are separate stages on purpose:
reflection produces diagnosis, proposal produces graph-mutation intent.

Proposal admission, child screening, validation, and frontier update are
optimizer policy, not Propose authoring. Propose just emits a typed
proposal; the engine (via `RunContext`) decides what becomes graph truth.

### Stages

`Stages` has exactly four slots:

```python
stages = lv.Stages(
    rollout=run,                                 # fn or Rollout.agent/.command/.manifest
    score=[correctness, trajectory_quality],     # Scorer | Sequence[Scorer]
    reflect=lv.Reflect.agent(lv.agent.codex()),  # fn or Reflect.agent
    propose=lv.Propose.agent_edit(lv.agent.codex()),  # fn or Propose.agent_edit
)
```

The four slots are exhaustive: `rollout`, `score`, `reflect`, `propose`.
There is no `evaluate`, no `improve`, no `judge`, no `layout`, no
`sampler`. A test in `leaven-py` locks `Stages.__init__` to exactly these
four parameters.

Each slot takes **either a plain async function** (custom logic; agentic
work via `cx` primitives inside) **or a declarative built-in**
(`Rollout.agent`, `Reflect.agent`, `Propose.agent_edit`, ...) for the
engine-mediated no-Python-logic case. `score` takes one named scorer or
a list of them (`Scorer | Sequence[Scorer]`).

Required vs optional: `rollout` and `score` are required (`score` is one
scorer or a non-empty list). If `reflect` or `propose` is omitted,
`lv.optimizers.gepa` installs a Codex-backed default for each.

The decorators `@lv.runner`, `@lv.scorer`, `@lv.reflector`,
`@lv.proposer` tag a function with its role. They are optional sugar for
the in-process case (a bare function in the slot works), and become
load-bearing only when a stage is served out-of-process as an ACP worker
(the engine needs the role to route the callback). Decorators do not
register globally.

### Advanced authoring: `@lv.evaluator`

The four-stage model covers the common path. For evaluators that need
multi-op batched effects (the locked ACP example's `cx.batch()`,
`cx.workspace.materialize_candidate`, `cx.sandbox.exec`, `cx.agent.run`
inside one transaction), `@lv.evaluator` remains the advanced authoring
surface:

```python
@lv.evaluator(id="ctf-evaluator", trust_profile="managed_sandbox",
              granularity="per_case")
async def evaluate(job: lv.EvaluationJob, cx: lv.EvalContext):
    for item in job.items:
        async with cx.batch() as b:
            ws = b.workspace.materialize_candidate(item.candidate)
            diff = b.workspace.git_diff(ws, against="parent")
            tests = b.sandbox.exec(workspace=ws, argv=[...])
            agent = b.agent.run(workspace=ws, instructions=...)
        cx.submit(lv.AssessmentWrite.independent_case(
            case_id=item.case_id, value=..., feedback=...,
        ))
```

When used, `@lv.evaluator` replaces the `rollout` + `score` slots: pass
`stages=lv.Stages.evaluator(evaluate, reflect=..., propose=...)`. The
evaluator owns the rollout-and-score composition itself (it runs cases
and submits assessments directly), so `rollout=` and `score=` are not
provided alongside it. This is the explicit escape valve for evaluators
that need multi-op batched effects beyond per-case named scores.

The `@lv.evaluator` decorator and its `EvalContext` / `EvaluationJob` /
`AssessmentWrite` companions live in `lv.adapters.*` for typed
authoring; the decorator alias `lv.evaluator` and the supporting types
remain importable from top-level for the advanced surface, but ordinary
runner/scorer code never imports them.

### Pairwise/listwise judging

There is no `@lv.judge`. Pairwise comparison expresses itself either as
a `Scorer` whose `RolloutResult` includes the pair, or as a custom
`@lv.evaluator` that emits comparison assessments. The third decorator
role added accidental concept weight; folding it removes a noun without
losing capability.

### Runtime

`runtime` (formerly `environment`) is the execution substrate:

```python
runtime = lv.runtime(
    lm=lv.lm.anthropic(model="claude-opus-4-7"),
    agent=lv.agent.codex(model="gpt-5.5"),
    sandbox=lv.sandbox.docker(image="python:3.12"),
    workspace=lv.workspace.local(root=".leaven/work"),
    trust_profile="managed_sandbox",
    budget=lv.budget(usd=200, calls=2000),
    cache=lv.cache.sqlite_default(),
)
```

The `runtime.agent` is the **engine-mediated executor** used by the
engine-mediated stages (`Rollout.agent`, `Reflect.agent`,
`Propose.agent_edit`) and by `cx.agent.run(...)` calls inside your
functions. It is **runtime config, not artifact state**: mutating the
artifact's Codex-shaped behavior (skills, instructions, hooks) does
**not** change which agent `runtime.agent` spawns. This is the
load-bearing half of the Codex dual-role (see below) — the executor is
fixed substrate; the behavior it executes is the evolving artifact.

> **Unspecified in V1 (flagged):** `budget` accounting semantics are not
> fully pinned. `calls=` here counts metric/LM calls; **budget tracking
> for agent execution (engine-mediated Codex sessions) is not yet
> specified.** This is a known gap to resolve before the budget surface
> is real.

Convenience constructors short-circuit common shapes:

```python
runtime = lv.runtime.local(budget=lv.budget(usd=20))
runtime = lv.runtime.acp(worker="leaven serve --stdio", budget=...)
```

The rename from `environment` to `runtime` matches Verifiers / Harbor /
Inspect convention: in evaluation ecosystems, "environment" means the
task world (sandbox, files, the world the agent acts on), not the
execution substrate. `Task`/`Case` already own task-world facts. The
substrate that runs them is `runtime`.

For backward compatibility, `lv.environment(...)` is a deprecated alias
through 0.2.x with a `DeprecationWarning`; removed in 0.3.

Multiple LMs and multiple agents are allowed (role-bound by key):

```python
runtime = lv.runtime(
    lm=[lv.lm.anthropic(...), lv.lm.openai(...)],
    agent={
        "executor": lv.agent.codex(model="gpt-5-codex"),
        "judge":    lv.agent.command(["my-judge-cli", "--stdio"]),
    },
    ...
)
```

Codex is the only first-class agent in V1. `lv.agent.command(argv)` and
`lv.agent.config(...)` are the generic escape hatches for any other CLI
agent. `lv.agent.claude_code(...)` and `lv.agent.opencode(...)` are
**reserved scaffold names** (the Rust adapters exist but do not yet prove
session/cost/protocol behavior); they raise `NotImplementedError` until
real. The trust profile bundles execution policy + capability defaults
from a fixed enum (`trusted_local_operator`, `managed_sandbox`,
`package_scorer`, `remote_untrusted`).

### evolve

`lv.evolve(...).run()` is the entry point:

```python
result = await lv.evolve(
    artifact=artifact,
    task=task,
    stages=stages,
    optimizer=lv.optimizers.gepa(score="correctness"),
    runtime=runtime,
).run()
```

The renamed verb (from `optimize` to `evolve`) reflects what Leaven
actually does: an artifact lineage with frontier admission, not
parameter tuning. `lv.optimize(...)` remains as a deprecated alias
through 0.2.x.

The result is typed `Evolved[Artifact]`:

```python
result.best                    # Candidate[Artifact]
result.frontier                # list[Candidate[Artifact]]
result.summary                 # RunSummary with cost, replayability, etc.
result.test_assessments()      # Iterable[Assessment]
result.assessment(case_id)     # Assessment
await result.replay(case_id)   # ReplayResult, deterministic
```

Inspection of completed runs uses the same surface, opened externally:

```python
run = lv.runs.open(".leaven/runs/2026-05-25-codex-ctf")
print(run.best.artifact.summary())
for a in run.test_assessments():
    print(a.score.value, a.case.id)
for ancestor in run.lineage(run.best.id):
    print(ancestor.id, ancestor.proposal.summary())
```

### How Python code reaches the engine (the ACP worker model)

This is the part that is easy to get backwards, so it is stated plainly.

The locked seam **inverts** ordinary ACP: **the engine is the ACP
client and drives; your Python stage code is the ACP agent (the
worker)**; the engine spawns the worker with a capability token. ACP
exists for exactly one reason — so Python (and later TS/Go) can drive
Leaven *without pyo3*. Your `cx.lm.complete(...)`, `cx.agent.run(...)`,
`cx.sandbox.exec(...)` calls are ACP extension-method callbacks from the
worker up to the engine, which is the only thing that touches policy,
receipts, cost, and graph mutation.

Two things are **not** the same and must not be conflated:

- **Engine-mediated stages** (`Rollout.agent`, `Rollout.command`,
  `Reflect.agent`, `Propose.agent_edit`): the engine runs these itself.
  A `Rollout.agent(codex)` means "the engine materializes the artifact
  and runs Codex against it" — *the artifact does the rollout*. There is
  **no Python function to serve** for these. (Codex is reached over the
  internal `codex-app-server` protocol, not the public seam — Codex is
  never a public-seam worker.)
- **Python-authored stages** (a `rollout`/`reflect`/`propose` function,
  or a `score` function): *these* are what the engine calls back into
  the worker for.

So in the codex_kit MVP — where rollout/reflect/propose are all
engine-mediated Codex and only the scorer is Python — the only
public-seam worker callback is the scorer.

You normally never write a worker entry point. In `lv.evolve(...).run()`
the SDK manages the worker lifecycle for you: it stands up the engine,
hands it the composed plan, and serves your Python stage functions back
over the seam transparently.

The explicit `lv.serve(...)` entry point exists only for the **other
deployment mode** — when something other than your script drives the run
(a `leaven` CLI invocation, a cloud/multi-tenant engine), and your
script is launched purely as a stage worker:

```python
# my_worker.py — only needed when an external engine drives the run
import leaven as lv
from my_stages import run, correctness   # the Python-authored stages

if __name__ == "__main__":
    lv.serve(rollout=run, score=correctness)
```

`lv.serve` registers only the **Python-authored** stages it is given;
engine-mediated stages (`Rollout.agent`, etc.) are configured in the
plan, not served here. The functions are identical to the ones you pass
to `lv.evolve(stages=...)`; only the lifecycle differs.

## Codex as the default agent

Codex is the default Leaven agent and shows up in two distinct roles.
The distinction is load-bearing:

**Engine-mediated executor.** When `lv.agent.codex(...)` appears inside
`Rollout.agent`, `Reflect.agent`, or `Propose.agent_edit`, Leaven runs
the agent. Leaven owns the workspace allocation, the session capture,
the trajectory normalization, the cost accounting, the cleanup. The
agent session is engine-mediated: it shows up in `RolloutResult.sessions`
and the trajectory is recorded as graph evidence.

**Codex-shaped behavior as artifact.** When the *thing being evolved*
is the configuration that drives Codex — the developer instructions,
the skill bank, the task message template, the hooks config, the tool
policy — that lives in the artifact. The artifact is not Codex; the
artifact is the mutable package that, when projected into a workspace
and consumed by Codex-as-executor, produces specific behavior.

> **Rust work required (flagged):** the codex_kit artifact adapter
> (projection of the mutable surface into a workspace, typed readback of
> agent edits into an artifact-native change, identity/fingerprint over
> the mutable surface) does not exist in Rust yet. It is upcoming work in
> `crates/leaven-artifact-codex-kit`. The Python `lv.artifacts.codex_kit`
> lowers to it; the Python surface can be specified now, but the
> end-to-end behavior is gated on that Rust adapter landing.

The same `lv.agent.codex(...)` constructor configures the executor; the
`lv.artifacts.codex_kit(...)` adapter configures the mutable behavior
package. They are different objects with different ownership:

```text
lv.agent.codex(...)        engine-mediated executor; runtime config
lv.artifacts.codex_kit(...) mutable behavior package; artifact state
```

When `Propose.agent_edit(lv.agent.codex())` runs, one Codex session
inspects evidence and edits `target/current/`; the artifact adapter
reads those edits back as a typed change. The agent doing the editing
and the artifact being edited can both be Codex; they are still
different things.

Codex is the **only first-class agent in V1** — it is the one with real
session/cost/protocol behavior (via `codex-app-server`). For any other
agent, the generic escape hatches are `lv.agent.command(argv)` (run an
arbitrary CLI agent) and `lv.agent.config(...)` (custom provider
config). `lv.agent.claude_code(...)` and `lv.agent.opencode(...)` are
**reserved scaffold names** whose Rust adapters do not yet prove
behavior; they raise `NotImplementedError` until real. Codex is both the
default and, today, the only blessed agent because the MVP is to evolve
Codex-shaped behavior end-to-end.

### Foreign / nested agents

If the artifact's code spawns its own agent process (a script that
shells out to a model API directly), Leaven only sees the spawned
process's files and stdout. The session is not engine-mediated and does
not appear in `RolloutResult.sessions`. To route a nested agent through
Leaven's runtime — and gain normalized session capture, cost
accounting, and trajectory recording — call it through `cx.agent.run`
from inside a rollout function or `@lv.evaluator`.

## codex_kit: the flagship harness artifact

`lv.artifacts.codex_kit` is the first off-the-shelf harness artifact.
It is intended to be decked out, well-documented, and the canonical
demonstration of agentic evolution with Codex.

The codex_kit known surface (paths the adapter understands how to
project and read back). It is split into a **default mutable set** and
an **opt-in mutable set**:

```text
default mutable (optimized unless excluded):
  AGENTS.md                    repo-rooted developer instructions
  .agents/skills/**/SKILL.md   skill library entries
  dev_instructions.md          dev-time instructions block

opt-in mutable (named explicitly in mutable= to be optimized):
  task_message.md              initial user message template (case-rendered)
  hooks.toml                   Codex hooks config
  mcp.json                     MCP server config
  tool_policy.toml             tool allowlist / denylist

not artifact state (frames the artifact, never optimized):
  codex_kit.toml               manifest: schema version, mutable globs, identity rule
  .codex/                      Codex-side config Leaven does not edit
```

`task_message.md` is **opt-in** rather than default because it is
case-rendered — it straddles artifact (the *template* is behavior worth
optimizing) and task (the case fills its slots). It is high-signal, so
it is right there in the known surface, but you opt into evolving it
explicitly rather than having it on by default.

Construction:

```python
artifact = lv.artifacts.codex_kit(
    "./agent_kit",
    mutable=[
        "AGENTS.md",
        ".agents/skills/**/SKILL.md",
        "dev_instructions.md",
        "task_message.md",   # opt-in
        "hooks.toml",        # opt-in
    ],
)
```

The `mutable=[...]` argument is required and validated against the known
surface. Paths outside it require explicit `lv.unsafe("custom/path")`,
which warns at construction time.

`hooks.toml`, `mcp.json`, and `tool_policy.toml` are first-class
optimization targets, not a hazard to hedge around. The entire premise
is code execution — evolving how the agent hooks, which MCP servers it
mounts, and what tools it may call is *exactly* the kind of behavior
worth optimizing. They are gated like any other execution in Leaven: the
runtime trust profile and capability tokens bound what the executed
hooks/servers may do, and data-class propagation bounds what they may
egress. That gating is the same machinery every engine-mediated action
already passes through — not special-case codex_kit policy.

The readback law: after `Propose.agent_edit` runs, the agent's workspace
edits under `target/current/` are diffed against the parent projection;
only paths matching the `mutable=` patterns are accepted as artifact
change. Edits outside the mutable surface are rejected as out-of-scope
and surface as a typed proposal-rejection assessment.

The identity / fingerprint law: codex_kit content-hashes its mutable
surface plus a manifest fingerprint; changes to immutable files do not
affect identity (they are not the artifact).

The Rust implementation lives in `crates/leaven-artifact-codex-kit`
(upcoming — see the flagged Rust-work note above). The Python adapter
`lv.artifacts.codex_kit` lowers to that Rust adapter via the locked ACP
wire; it does not embed a Python re-implementation.

## Optimizers

The only behavior-bearing optimizer in V1 is `lv.optimizers.gepa`. The
spec acknowledges this directly rather than designing against
hypothetical generic optimizers:

```python
optimizer = lv.optimizers.gepa(
    score=correctness,
    train=lv.gepa.sampling.minibatch(split="train", size=3),
    validation=lv.gepa.validation.full(split="val"),
    population_size=8,
    frontier=lv.gepa.frontier.top_k(3),
)
```

`score=` is which scorer GEPA uses for primary comparison — passed as the
**scorer object** (typed; rename-safe), or by name string as a
convenience. Multi-objective comparison:

```python
optimizer = lv.optimizers.gepa(
    score=lv.gepa.compare.weighted({correctness: 0.8, trajectory_quality: 0.2}),
)
```

**The four stages are the optimizer-agnostic phase model** — rollout →
score → reflect → propose is the shape of any reflective-evolutionary
optimizer. What is GEPA-specific is the *policy that orchestrates them*:
parent selection, minibatch sampling, gating, validation, frontier
representation, and reflective-dataset construction. That policy all
lives under `lv.gepa.*`, not at top level:

```text
lv.gepa.sampling.*           train minibatch sampling policy
lv.gepa.validation.*         accepted-candidate validation policy
lv.gepa.frontier.*           frontier representation
lv.gepa.gate.*               child acceptance criterion
lv.gepa.component.*          which artifact part to target
lv.gepa.compare.*            multi-score comparison policy
reflective_dataset=          hook: case evidence → ReflectiveCase records
```

The `reflective_dataset=` hook is how you customize what reflection sees
(see the Reflect section's build-once-pass-down rule): it runs
engine-side as GEPA policy and hands the reflector a finished batch.

These are deliberately GEPA-namespaced, not a generic optimizer-agnostic
phase interface. GEPA is the only behavior-bearing optimizer in V1, so a
generic policy interface would be designed against zero real second
cases. When a second optimizer arrives, the shared surface gets
re-examined then; the four-stage phase model is already the part that
generalizes.

`lv.optimizers.mipro`, `lv.optimizers.textgrad`, and
`lv.optimizers.trace` are **reserved names**. They exist in the
namespace and raise `NotImplementedError` with a pointer to the
roadmap. The spec does not pretend they are behavior-bearing.

## DSPy

DSPy users live in Python and have specific expectations about LM
adapters. The integration shape is drop-in:

> **Rust implementation question (flagged, out of scope for this spec):**
> `leaven-lm` may be worth splitting into its own standalone project —
> e.g. a Rust wrapper over `rig` that exposes a DSPy-LM-compatible shape.
> That is a question about what *backs* `lv.lm.*`; it does not change
> this Python surface (`lv.lm.openai(...)` etc. stay the same either
> way). Captured for the `leaven-lm` crate, not decided here.

```python
import dspy
import leaven as lv

dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(model="claude-opus-4-7"))

# All existing DSPy code works unchanged.
program = dspy.ChainOfThought("question -> answer")
result = program(question="...")
```

`lv.x.dspy.LeavenDSPyLM` is a `dspy.BaseLM` subclass that lowers into
Leaven's `leaven-lm` neutral types. DSPy lives in the `x.dspy` adapter
namespace, not in core. The pattern generalizes: future adapters
(`x.verifiers.*`, `x.harbor.*`, `x.skill_bank.*`) follow the same shape
— typed integration with an external ecosystem, lowered into core
Leaven types, lifted back for the user.

DSPy `Program` instances can be carried as artifacts via
`lv.x.dspy.artifact(program=...)`, which lowers the program's
parameter state into a Leaven-native artifact change-set for GEPA-style
optimization.

## The acceptance gate

A P5-shaped GEPA optimization with an agentic reflector runs through
the Python SDK end-to-end against a live LM provider, producing a
measurable `Evolved[Artifact]` with improvement over the seed. The
demo:

- Python composition: `lv.evolve(artifact=codex_kit, task=task,
  stages=lv.Stages(...), optimizer=lv.optimizers.gepa(...),
  runtime=lv.runtime.local(...))`
- Rollout via `lv.Rollout.agent(agent=lv.agent.codex())` against real
  cases inside a real sandbox
- GEPA loop driving multiple iterations (parent selection, minibatch
  sampling, rollout, score, reflect, propose, admit, accept)
- Real LM provider calls through `cx.lm.complete`
- Real agent calls through engine-mediated Codex sessions
- Frontier admission, validation pareto state, capability tokens, data
  classes, receipts all preserved end-to-end
- `RolloutResult.sessions` and `.trajectory` populated and queryable
- Output queryable via `lv.runs.open(...)` with receipts, lineage,
  evidence intact

This is the only gate. A smaller demo (single Python scorer, no
agentic reflector, no real optimizer loop, no codex_kit) does not
satisfy it. The gate is unfakeable: it requires every load-bearing
piece to be real.

## What success is not

These shapes look like success but are not the product. If
implementation produces any of them, the spec has been compromised:

- A pyo3 wrapper of `leaven-run` shipped as `leaven`. Bypasses the
  wire; recreates the rejected manylinux/ABI/language-lock failure
  mode.
- ACP conformance rows promoted to `proven` from in-process Rust tests
  labeled as ACP. The matrix explicitly names this as
  `fake_pass_rejected`.
- A working `@lv.scorer` decorator with no `lv.evolve()` /
  `lv.runtime()` / `lv.optimizers.gepa()`. The stage-only framing this
  spec rejects.
- Typed Python records that don't validate against the same JSON
  Schemas the Rust engine uses. Typed lipstick on schemaless drift;
  recreates the MCP failure mode.
- Python SDK demoed against a mock engine only. Doesn't prove the wire
  carries capability / receipts / data classes across a real process
  boundary.
- A toy Python optimization without an agentic stage. Doesn't exercise
  the load-bearing complexity that the acceptance gate names.
- Engine/wire nouns leaking into `lv.__all__` (`OutputRecord`,
  `EvaluationJob`, `Granularity`, `EvidenceEnvelope`, `AssessmentWrite`,
  `ProposalBatch`, `RegisteredStage`, `ReflectRequest`, etc.). These
  belong in `lv.adapters` or `lv.wire`, not the product surface.
- A `Stages` object that accepts a fifth field, or an `Improve` stage,
  or a `Reflect.gepa()` preset. The four-transform discipline is
  load-bearing.
- A `codex_kit` artifact that accepts mutable paths outside the
  declared safe surface without opt-in. Capability evasion via artifact
  state.

## Public API discipline

The Python surface has rings, not just tiers:

```text
lv.*                product nouns only
lv.adapters.*       advanced authoring: Evaluator, RegisteredStage,
                    RunContext / StageContext / EvalContext typed annotations
lv.wire.*           generated public-seam schemas (OutputRecord,
                    EvaluationJob, EvidenceEnvelope, AssessmentWrite, ...)
lv._engine.*        private engine helpers; not stable; no user reach
```

The rule: **a user reading the top-level `lv.*` import surface sees
product nouns only.**

Top-level `lv.__all__` (product nouns):

```text
__version__
Task, Case
Score                          tiny scorer return value
Scorer                         type alias for the scorer callable (annotations only)
RolloutResult                  completed attempt (scorer/reflector-facing)
Critique, Proposal             reflect output / propose output
Stages                         the four-slot composition
Rollout, Reflect, Propose      declarative built-in namespaces
                               (.agent / .command / .manifest / .agent_edit)
Runtime, runtime, budget
evolve, serve

# convenience top-level decorators (sugar; required only for served workers)
runner, scorer, reflector, proposer, evaluator

# namespaces
artifacts, optimizers, lm, agent, sandbox, workspace,
layouts, output, cases, setup, assets, runs, gepa, trust, x
```

Note what is NOT here: `Scorer` is a **type alias for annotations only**,
not a constructor (scorers are plain functions). No `Evaluate` (scoring
is the `score` slot — one scorer or a list). No `Rollout.fn` /
`Scorer.fn` constructors. No top-level `sampling` / `frontier` /
`compare` / `validation` (those are GEPA policy under `lv.gepa.*`).

Forbidden in top-level `lv.__all__` (engine/wire/adapter nouns):

```text
OutputRecord, Visibility
EvaluationJob, EvaluationItem, Granularity, Purpose
EvidenceEnvelope, EvidencePublic, EvidencePrivate
AssessmentWrite, Replayability
ProposalBatch, ProposalEffect
ReflectRequest, ProposeRequest, JudgeRequest, ReflectExample, ReflectionResult
ReflectiveCase, ReflectiveRun, Attachment, TraceRef
StageSourceRef, StageRole, RegisteredStage
RunContext, StageContext, EvalContext
RunCase, ScoreCase, CandidateHandle, WorkspaceHandle, WorkspaceLifetime,
WorkspaceSurface
QueryReceipt, CallReceipt, WriteReceipt
```

These nouns exist — they are part of the engine and the wire — but they
do not live at the top level. Authors who need them import explicitly
from `lv.adapters` or `lv.wire`.

A test in `leaven-py` enforces both directions: every name in the
allow-list is exported; no name in the forbidden list appears in
`lv.__all__`.

The two earlier sub-rules carry over:

1. **Every public module ships an `__all__`** listing exactly the names
   it intends to export. Modules without `__all__` are implementation
   detail even if they live in the public package tree.
2. **No backdoor exports.** A public-tier function or class must not
   require importing from a private module to use. If a user needs
   `_InternalThing` to compose `lv.public_thing`, that's a design
   failure — either promote `_InternalThing` to public or hide it
   behind a public-tier shim.

Linting: ruff's `RUF022` (sorted `__all__`) on by default. Ring
discipline is enforced by review against the explicit allow-list / deny
list above, plus the surface tests.

Deprecation policy for V1: any removal or signature change to a public
symbol passes through one minor release with a `DeprecationWarning`
before the breaking change lands. `environment` → `runtime`,
`optimize` → `evolve`, and the eviction of engine nouns from top-level
are the V1 → V1.x transition deprecations; each ships with a working
alias for at least one minor release.

## Constraints on implementation

The implementation must honor:

- **Don't repeat Rust UX failures.** The user surfaced specific
  friction with Rust Leaven setup; every Python surface must be
  readable as "this is what the user writes" without requiring
  cross-file inference or implicit conventions.
- **Don't expose engine internals.** `RunContext`, `RunGraph`, internal
  optimizer state, internal store handles, internal cache keys are not
  product Python concepts. Product types mirror seam types only;
  advanced authoring imports typed contexts from `lv.adapters`.
- **Don't add hidden APIs.** Anything public in the Python sense is in
  the spec. No private functions doing public work. No `_internal`
  modules that users discover and depend on.
- **Don't proliferate ways to do one thing.** One canonical way.
  `Stages` with four fields, not a freeform DAG. Decorators as sugar
  over the same constructors. Typed records, not unstructured maps.
- **Don't ship without typing.** Pydantic v2 models for wire-shaped
  records; dataclasses for internal config; full type hints.
  `RolloutResult[Out]` and `Evolved[A]` are generic; IDE autocomplete
  works without guessing.
- **Don't bundle benchmark catalogs.** `lv.cases.from_jsonl /
  from_parquet` are generic loaders. Paper-specific catalogs live in
  separate `leaven_benchmarks_<name>` packages users opt in to.
- **Don't depend on the upstream ACP SDK.** Path B per the
  conversation; reasons in the wire section.
- **Don't language-lock the wire.** Any decision that would make a TS
  or Go SDK awkward later is a failure.
- **Don't put `ContextVar` magic on `cx`.** `cx` is passed
  explicitly to every stage function, never implicitly via thread
  binding. This is a deliberate divergence from marvin/pydantic-ai
  (validated by parallel adversarial review in
  `docs/specs/leaven_py/docs/agent-context/patterns/SYNTHESIS-2026-05-24.md`).
- **Don't put fields on `RunContext`/`StageContext`/`EvalContext` that
  are optional across execution boundaries.** Either every context type
  has the field, or it is not a context field at all. (Per pydantic-ai
  adversarial-review finding.)

## What governs disagreement

This spec governs Leaven Python. The locked public seam V1 at
`docs/specs/public-seam-v1/` governs the wire. If this spec and the
locked seam disagree about wire behavior, the seam wins; this spec is
revised to match. If this spec and any implementation artifact (plan,
working-memory note, code, test) disagree about what the product is,
the spec wins; the artifact is revised.

The 2026-05-25 revision (this revision) replaced the six-decorator
authoring model with the four-stage transform model after sustained
design pressure surfaced that the decorator model was leaking engine
and wire nouns into product authoring code. The revision was
deliberate; the spec is not implementation documentation, and
implementation that requires further spec change pauses and proposes
the change.

## What this spec does not say

Per the discipline that EXPLICITLY_UNMODELED is more complete than
silence: this spec deliberately does not specify:

- **How `leaven-acp` is implemented.** Module layout, async dispatch
  shape, cancellation propagation pattern, bounded-queue mechanics.
- **Distribution mechanics.** Wheel layout, per-platform binary
  bundling, CI matrix, PyPI publication policy.
- **Implementation tranche ordering or timeline.** Implementation
  orders its own work.
- **Test names.** Implementation chooses test names. The spec names
  the gates (acceptance gate, conformance matrix rows, ring discipline
  surface tests) that tests must satisfy.
- **Windows support.** Not in V1; revisited when V1 ships.
- **TypeScript or Go SDKs.** Future spec slices when those land.
- **codex_kit hooks/MCP enforcement details.** Hooks/MCP/tool-policy are
  first-class mutable targets gated by the ordinary trust-profile and
  data-class machinery; the *exact* capability rules for hook execution
  and MCP-server mounting are downstream detail, not specified here.
- **Budget accounting for engine-mediated agent sessions.** `budget`
  counts metric/LM calls today; how engine-mediated Codex session cost
  is metered and charged is a known unspecified gap.
- **Exact trajectory normalization shape.** `RolloutResult.trajectory`
  exposes commands / tool calls / messages / output files /
  optionally raw provider events; the precise schema is downstream
  from the locked seam's stage payload definitions.
- **Custom optimizer protocol.** When a second behavior-bearing
  optimizer arrives, the optimizer-policy boundary will be
  re-examined; this spec does not pretend to design it now.

## Provenance

This spec was authored on 2026-05-24 during a long design conversation
that produced the original six-decorator model. It was revised on
2026-05-25 after a follow-on design pass exposed the four-stage
discipline:

- The six-decorator model leaked engine nouns into product code
  (`EvaluationJob`, `Granularity`, `OutputRecord`, `AssessmentWrite`,
  `RegisteredStage`, `ReflectRequest`/`ProposeRequest`/`JudgeRequest`).
- `Evaluate` and `Reflect.gepa()` exposed that the "stages" bag was
  conflating three layers: execution components, measurement policy,
  and optimizer phase configuration.
- `Improve` was tried as a unifier and rejected because it collapsed
  reflect/propose, which the user specifically wanted separable.
- `Objective` + `aggregate` was tried and rejected as a "cardboard box
  labeled the important stuff" that solved a type-shape problem by
  adding a noun without clarifying ownership.
- An intermediate landing kept `Stages` with four fields
  (rollout/evaluate/reflect/propose) and scorers nested under an
  `Evaluate` stage. A second pass (2026-05-25, round 2) dissolved
  `Evaluate`: once scorers are plain functions, `Evaluate.scorers({...})`
  has no content left, so scoring became a `score` dict slot directly.
  Final shape: `Stages` = `{rollout, score, reflect, propose}`.
- `Codex` was clarified as having two distinct roles: engine-mediated
  executor (configured via `lv.agent.codex`, fixed runtime substrate)
  vs Codex-shaped behavior package (configured via
  `lv.artifacts.codex_kit`, the evolving artifact). Mutating the artifact
  does not change which agent the runtime spawns.
- `codex_kit` was named as the flagship off-the-shelf harness
  artifact, with Rust implementation upcoming in
  `crates/leaven-artifact-codex-kit`. Its mutable surface was split into
  a default set (AGENTS.md, skills, dev_instructions) and an opt-in set
  (task_message, hooks, mcp, tool_policy); hooks/MCP/tool-policy are
  first-class optimization targets gated by the ordinary trust/capability
  machinery, not special-cased hazards.
- `optimize` → `evolve` and `environment` → `runtime` were renamed to
  reflect what Leaven actually does (artifact lineage, not parameter
  tuning) and to avoid collision with Verifiers/Harbor/Inspect's
  "environment = task world" convention.
- Engine-noun eviction from top-level was made explicit and enforceable
  via a surface test.

Round-2 (2026-05-25) refinements, after reading the actual scaffold,
the WIP examples, FlashEvolve, vendored GEPA, and the locked seam:

- **Stages are plain async functions**, not constructor objects. A
  scorer is just `async def (run, case, cx) -> Score`; agentic scoring
  is `cx.agent.run(...)` inside it, not a `Scorer.agent` constructor.
  The `.fn` / `.agent` zoo collapsed: a slot takes a function *or* a
  declarative built-in (`Rollout.agent` / `Reflect.agent` /
  `Propose.agent_edit`) for the engine-mediated no-Python case. This is
  the FlashEvolve "stages are functions, compose inside" taste, adapted
  to Leaven's `cx` (FlashEvolve uses typed payloads because it has no
  context object; Leaven has `cx`).
- **The ACP worker model was corrected and stated plainly.** The engine
  is the ACP *client* and drives; Python stage code is the ACP *agent*
  (worker) the engine spawns. ACP exists so Python can drive Leaven
  without pyo3. Codex is reached over the internal `codex-app-server`
  protocol, NOT the public seam — Codex is never a public-seam worker.
  Engine-mediated stages are run by the engine (no Python served); only
  Python-authored functions are served back. `lv.serve` is only for the
  deployment mode where an external engine drives the run.
- **The reflection data-ferrying blocker was resolved** via
  build-once-pass-down (confirmed across GEPA, Leaven's Rust, and the
  locked seam): the optimizer builds the target-safe reflective batch
  once and hands `reflect` a finished `ReflectRequest`; the reflect
  function never assembles its own evidence. Heavy data (transcripts,
  env state) rides by `TraceRef` handle. Customizing what reflection
  sees is the `lv.optimizers.gepa(reflective_dataset=...)` hook, not
  logic inside the reflector. (Leaven's Rust reflection types are ahead
  of the Python scaffold and are the authority for shape.)
- **Splits are user-defined labels**, not a fixed train/val/test enum.
- **claude_code/opencode are reserved scaffold names**, not first-class.
  Codex is the only blessed agent in V1; `lv.agent.command` /
  `lv.agent.config` are the generic escape hatches.
- **Flagged-but-unresolved (not blocking):** budget accounting for
  engine-mediated agent sessions is unspecified; whether `leaven-lm`
  should become a standalone `rig`-wrapper is a Rust implementation
  question that does not change this surface.

The 2026-05-24 patterns synthesis at
`docs/specs/leaven_py/docs/agent-context/patterns/SYNTHESIS-2026-05-24.md`
remains the cross-cutting evidence for keeping
`ContextVar`-magic out, keeping the schema-locked typed-validation
discipline, and the load-bearing context-field rule.

The acceptance gate is unchanged from 2026-05-24: a live P5-shaped GEPA
optimization with an agentic reflector through the Python SDK
end-to-end, producing an `Evolved[Artifact]` with measurable
improvement over the seed.
