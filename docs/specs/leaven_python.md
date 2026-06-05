# Leaven Python

Status: governing product spec.
Created: 2026-05-24.

## What this is

Leaven Python is the way you use Leaven. You write Python that configures
and drives a full Leaven optimization run end-to-end — runtime,
optimizer, stages, run, inspection — and the result is a typed, replayable,
auditable optimization with every safety property of the locked public seam
preserved across the wire. Capability tokens are real. Data-class
propagation is enforced. Receipts are audit currency, not log decoration.
Replay is per-assessment honest.

Python users do not learn Rust to use Leaven. The small set of people who
write optimizer crates, workspace backends, agent runtime adapters, and
artifact semantics work in Rust. Everyone else — the people who write
evaluators, runners, scorers, reflectors, proposers, judges, the people who
compose runs, the people who inspect results, the people who reproduce
papers, the people who do the actual research — works in Python. The Python
surface is not an afterthought added because someone wanted bindings. It is
the product. Rust is the substrate.

## What this is not

It is not a stage-authoring kit. The user who can only write a Python
`@lv.evaluator` and has to drop into Rust to compose a run does not have
the product. The user composes the whole thing from Python or this surface
has failed.

It is not a pyo3 wrapper of `leaven-run`. The
`docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md`
named the reasons at line 29 — manylinux wheel matrix, Python ABI
versioning, GIL + Tokio integration, language-locks Leaven forever. Those
reasons still bind. The decision is durable.

It is not a wrapper around `leaven-run`. `leaven-run` is the Rust
product-builder API; Leaven Python is the Python product-builder API.
They are peers that share the same engine, not a stack.

It is not the same as the public seam V1 spec. The public seam V1 spec
locks the wire contract for external-language workers — plan IR, capability
tokens, result receipts, stage payloads, evidence envelopes, the Leaven
worker profile, JSON schemas. This spec uses that wire and adds the Python
projection on top of it. If this spec and `docs/specs/public-seam-v1/`
disagree about the wire, the seam wins.

## Why this exists

In Rust, Leaven is currently hard to set up. Runtime setup bugs.
Topology that needs to be held in working memory. Trait bounds that fail
in non-obvious places. The user surfaced this directly during the design
conversation that produced this spec: *"in Rust, Leaven is not usable.
It's really fucking hard to set up, dude."*

This is the load-bearing motivation. If Rust is hard AND Python is
afterthought, the usability problem isn't solved. The Python surface is
not "let's add bindings" — it is "let's make Leaven actually usable for
the people who do the research." Everything else in this spec serves
that.

The original target sentence is from the archived design pass at
`COMPREHENSIVE_DESIGN_PASS_NOTES.md:15`: *"make the next paper repro 200
lines of glue in whatever language the user wants — Python first, then
TS, then ship a CLI any language can drive — without losing graph truth,
target safety, budget accounting, replay, audit, or remote/multi-tenant
deployment."* That sentence is still operative.

## What the user writes

The high-level Leaven program in Python is `seed × environment × optimizer ×
runtime`. The environment bundles the task (cases with split tags, sandbox
needs), the rollout (how the current artifact runs on a case), and the rubric
(how the result scores); task records stay close to Inspect's `Task` / case
vocabulary:

```python
import leaven as lv

task = lv.Task(
    cases=[
        lv.Case(
            id="arith-001",
            input={"question": "What is 2 + 3?"},
            target={"answer": "5"},
            files={"README.md": "Answer the arithmetic question."},
            setup=lv.setup.bash("mkdir -p output"),
            split="train",
        )
    ],
    sandbox=lv.sandbox.docker(image="python:3.12"),
)

rollout = lv.Rollout.command(
    argv=["uv", "run", "python", "target/current/run.py"],
    layout=lv.layouts.case_workspace(),
    output=lv.output.files(["output/result.json"]),
)

@lv.reward
async def correct(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    result_file = await cx.workspace.read_file(
        cx.rollout_workspace, "output/result.json")
    return 1.0 if result_file.content == case.target["answer"] else 0.0

result = await lv.optimize(
    seed=lv.artifacts.directory("./agent_harness"),
    environment=lv.Environment(
        task=task,
        rollout=rollout,
        rubric=lv.Rubric([correct]),
    ),
    optimizer=lv.optimizers.gepa(
        population_size=8,
        propose=lv.Propose.agent_edit(agent=lv.agent.codex(model="gpt-5-codex")),
    ),
    runtime=lv.runtime.local(budget=lv.budget(usd=20)),
).run()
```

The ownership rule is load-bearing:

```text
seed        = the mutable artifact being optimized
Environment = task (cases, split tags, hidden targets, sandbox needs)
              + rollout (how the artifact runs) + rubric (how it scores)
Optimizer   = the outer loop: reflect / propose / judge (GEPA-coupled in V1)
Runtime     = workspace/sandbox allocation, effects, receipts, trust, budget
```

`Rollout` is the interpretation of the current artifact version for one
sample. Do not put a universal `entrypoint` on the base artifact concept.
If a harness command or manifest must evolve, it belongs inside the artifact
projection under the stage's mutable root; the rollout remains the stable
contract for executing that projection.

For tiny prompt optimizations, the decorator form remains sugar over the
same roles. The smallest meaningful sketch is still ~20 lines:

```python
import leaven as lv

@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    response = await cx.lm.complete(prompt=prompt.template.format(**case.input))
    return response.text.strip()

@lv.reward
async def correct(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    return 1.0 if output == case.target["answer"] else 0.0

result = await lv.optimize(
    seed=lv.PromptArtifact(template="Answer: {question}\nA:"),
    environment=lv.Environment(
        task=lv.Task(cases=lv.cases.from_jsonl("cases.jsonl").cases),  # cases tagged with split=
        rollout=lv.Rollout.fn(run),
        rubric=lv.Rubric([correct]),
    ),
    optimizer=lv.optimizers.gepa(population_size=8),
    runtime=lv.runtime.local(budget=lv.budget(usd=20)),
).run()

print(result.best.artifact.template)
```

The shape is the same at any scale. An EvoSkill-shaped paper repro is the
same composition with more stages, a richer runtime, and an agentic
reflector. ~70 lines, including stage bodies. The composition glue does
not grow with the paper's complexity; the stage bodies do.

The locked-spec example at
`docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py` is an
archived aspirational evaluator shape, not a current Python SDK promise. It
uses `cx.case.load`, a deferred batch syntax,
`cx.workspace.materialize_candidate`, `cx.sandbox.exec`, `cx.agent.run`,
the deferred DSPy adapter shape, and legacy assessment helpers. Current V1
product claims must be proven through executable examples and the typed seam
service; do not cite that sketch as SDK maturity evidence.

The full surface inventory lives in
`docs/working-memory/leaven-py-research/2026-05-24-python-surface-sketches.md`.
That file is the entry point for implementation; the assumptions section
(§6) and the open questions section (§7) are the unfinished design work
that this spec resolves below.

## What the user does not write

The user does not write the optimizer loop. They configure existing
optimizers from a registry — `lv.optimizers.gepa(...)`,
`lv.optimizers.mipro(...)`, future entries — by passing typed configs to
`lv.optimize(...)`. The optimizer loop runs in Rust. New optimizers
require behavior-bearing Rust crates with local tests and topology rows,
not Python authoring.

This is not a temporary limitation. Optimizer strategy state binds
tightly to engine state — parent selectors mutate frontiers, samplers
consume populations, reflection requests cross trust boundaries with
typed envelopes. Authoring optimizer rhythm from Python would either
require exposing engine internals through the wire (defeating the
seam's separation) or accepting that Python optimizers are second-class
(defeating the equal-power promise).

The user does not write graph mutation. `RunContext` is the only graph
mutation authority and it is engine-side. Python authors emit typed
`AssessmentWrite` and `Proposal` envelopes; the engine applies them
through `RunContext` on the Rust side. This is per the locked seam.

The user does not write capability minting, trust ledgers, or budget
accounting. They declare `trust_profile="managed_sandbox"`, `input_classes=[...]`,
`forbidden_input_classes=[...]`, `budget=lv.budget(usd=200)`, and the
engine enforces. The declarations cross the wire; the enforcement is
engine-side.

The user does not write paper-close bring-up code (refusal cascades,
source pinning, manifest validators, fingerprint binders). That domain
work currently lives in Rust (~9,000 LOC for a paper like EvoSkill) and
will continue to live there in V1. A future V2 may add typed bring-up
primitives to the Python SDK that shrink the bring-up surface; V1 does
not. *Live-run* paper code is ~200 lines; *first-time-paper-bring-up* code
is a separate larger surface.

## The wire

The wire is the locked Leaven worker profile per
`docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`.
Stdio JSON-RPC, kind-discriminated JSON Schema payloads, capability
tokens via env-var bearer + fingerprint, session-bound permission
decisions, bounded progress updates, cancellation-with-receipts.

Leaven owns this wire. The implementation is in-house, in a Leaven crate
(`leaven-acp`). It does not depend on the upstream `agent-client-protocol`
SDK and is not upstream Agent Client Protocol conformance. The reasons are:

- **Schema lock.** Leaven's seam is schema-locked at
  `docs/specs/public-seam-v1/schemas/`; conformance is gated by
  `docs/specs/public-seam-v1/conformance-matrix.yaml`. The MCP-style
  schemaless-drift failure mode is inverted by construction. Inheriting
  an upstream SDK risks inheriting its release cadence and its
  abstractions for things Leaven explicitly does not have (proxy chains,
  protocol-version negotiation, MCP bridge behavior, dynamic handler
  registration).
- **Surface size.** The SDK is ~13,500 LOC; the in-house Leaven version
  is ~1,200 LOC because Leaven omits ~6,500 LOC of features it does not
  consume.
- **Multi-language safety.** Owning the wire keeps the schema-codegen
  pipeline uniform across future TS / Go / shell SDKs.

Upstream ACP remains relevant only as a possible future agent-provider
interop adapter, where Leaven wants to swap agent runtimes. It is not the V1
SDK-driving seam.

The wire is stable across language SDKs. The Python SDK speaks it. A
future TypeScript SDK will speak the same wire. A future Go SDK will too.
Multi-language safety is verified by the wire's properties (snake_case,
no `null` literals, JS-safe integers, opaque cursors) — not by hoping the
Python implementation generalizes.

The CLI consumes this wire too. The same `leaven` binary that the Python
SDK spawns as a child process via `leaven seam serve --stdio` also offers
`leaven query lineage`, `leaven runs list`, `leaven artifact show` as
human-facing subcommands. An agent working inside its own workspace can
introspect the run state by shelling out to `leaven query lineage`
exactly like it would shell out to `git log`. The CLI is not a separate
tool; it is the same binary, talking to the same engine, exposed through
a different I/O shell.

Current SDK status: `sdk/python/examples/10_live_codex_seam.py` is a
live-gated Python proof that drives this Leaven-owned stdio wire through the
private `leaven._seam` client package. It spawns
`leaven seam serve --stdio --config`, sends a locked `leaven/agent.run` Plan IR
request, and verifies workspace materialization, Codex CLI execution, receipts,
and transcript refs. `sdk/python/examples/12_live_optimize_codex_proposer.py`
now proves the same durable seam from the product path:
`lv.optimize(...).run()` dispatches a configured `@lv.proposer`, the proposer
calls `cx.agent.run` against `cx.parent_workspace`, and the resulting Codex
`gpt-5.4-mini` agent receipt is cited in a typed proposal submitted through
`leaven/proposal.submit_batch`. `sdk/python/examples/13_live_optimize_openai_lm.py`
is the numbered wrapper for the corresponding live-gated OpenAI LM product-path
proof; its real behavior lives in the `sdk/python/examples/live_openai_lm/`
project, with its own `pyproject.toml`, dependency-boundary declaration,
console command, and `src/live_openai_lm` package split into environment
config, scenario construction, output validation, and command entrypoint
modules. The runner calls `cx.lm.complete`,
receives text, usage, model, and receipt back through the callback-backed
`LmResponse`, and returns those facts in the reward output. It has deterministic
provider-wiring coverage through `leaven-seam-service` and SDK tests, but the
spend-bearing live proof remains pending until `OPENAI_API_KEY` is available.
These prove the public seam can be the Python/Codex substrate. They are not
proposal application/admission, persisted blob inspection, full live LM-provider
acceptance, or the full GEPA search loop named by the acceptance gate.

Structured output has two Python types. `lv.output.json_schema(Model)` returns
a model-backed `JsonSchemaOutput[Model]`: Rust still validates the provider's
parsed payload against the inline JSON Schema and `schema_fingerprint`, and the
Python SDK then parses the returned bytes into `Model` before exposing
`response.parsed` / `session.parsed`. `lv.output.json_schema(schema)` returns
`JsonSchemaValueOutput`: the wire is still schema-validated by Rust, but Python
exposes the parsed value as explicit `JsonValue` because no Python domain model
was supplied. User examples must rely on the output contract's typed owner, not
`isinstance(..., Model)` fallback checks or string/dict coercion after the
seam has returned.

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
- **Stage payload typing.** Reflector / proposer / runner / scorer / judge
  payloads have semantic owners with cross-stage binding (reflector
  example source refs are carried by the request; proposer cites
  reflector receipt; etc.).
- **Evidence visibility.** Public / private / trace projection data
  classes preserved through the wire. Target-derived evidence flagged
  honestly. Source receipts present and typed. Python public assessment
  evidence uses the closed `EvidencePublicPayload` owner (`summary`,
  `output`, `feedback`, `metrics`) and must not be re-parsed from an
  arbitrary JSON object by SDK builders or inspection surfaces.
- **Replay determinism.** Receipts plus inputs plus locked seam validation
  reproduce the assessment.

Whatever the Python user declares about input classes, trust profile,
output records, evidence envelopes — the engine enforces. Whatever the
Python user wires for cases, candidates, runs — the engine remembers in
typed receipts. The Python surface is the ergonomic projection of the
seam; it is not an alternative trust path.

## The Python authoring surface

A Leaven program is `seed × environment × optimizer × runtime`. The
environment is the inner loop — how the artifact runs (`rollout`) and how the
result scores (`rubric`). The optimizer is the outer loop — how the search
reflects and proposes. Stages are authored as decorators over async functions,
each receiving a **role-scoped `cx`** that exposes only the capabilities and
case projection its role is allowed:

- `@lv.runner` → `RolloutContext`; the function rollout. `case` is an
  `InputCaseView` (target-free). Wrap with `Rollout.fn(run)`.
- `@lv.reward(weight=...)` → `RubricContext`; a reward. `case` is a
  `ScoringCaseView` (target readable). Collect with `Rubric([...])`.
- `@lv.reflector(stage_id=...)` → `ReflectContext`; diagnosis over the
  target-safe reflective dataset. Receives `(req: ReflectRequest, cx)`.
- `@lv.proposer(stage_id=...)` → `ProposeContext`; emits a typed `ProposalBatch`
  (submit, never apply). Receives `(req: ProposeRequest, cx)`.
- `@lv.judge(stage_id=...)` → `JudgeContext`; pairwise or listwise preference.
  Receives `(req: JudgeRequest, cx)`.
- `@lv.evaluator(...)` → `EvaluatorContext`; the advanced seam escape hatch for
  batched effects and hand-authored assessments. Ordinary scoring is a `Rubric`,
  not an evaluator. Receives `(job: EvaluationJob, cx)`.

Each decorator is sugar over a registration call. `lv.register_stage(role,
spec, func)` is the underlying API. Both are exposed; the decorator is
the recommended form for most code, the function form is the recommended
form for dynamic stage registration (rare).

Function stages run in-process with the optimization (composed into the
environment as `Rollout.fn(run)` / `Rubric([reward])`, or onto the optimizer
as `gepa(reflect=..., propose=...)`). Standalone out-of-process Python worker
serving is not a public SDK entrypoint until it is wired through the current
Leaven-owned worker runtime with capability env vars per the locked worker
profile. The decorator shape and the function signature are intended to stay
identical when that route lands; only the way the engine reaches the stage
will differ.

The role-scoped context objects (`RolloutContext`, `RubricContext`,
`ReflectContext`, `ProposeContext`, `JudgeContext`, `EvaluatorContext`) carry
only the query/effect builders their role is permitted — LM completion, agent
runs, sandbox exec, workspace reads, and (for privileged roles) candidate
materialization, proposal submission, or assessment submission. The capability
boundary is structural: a runner's `cx` has no `proposals`, and its `case` has
no `.target`. These builders construct typed Plan IR ops that the engine
validates against the locked seam before execution.

`ProposalBatch` contains `ProposalEffect` records with direct typed fields for
the effect kind, causal parent, surface fingerprint, change schema, parser,
agent-session receipt, artifact fields, typed artifact value, and `change_value`. It
does not carry an anonymous `payload` object that builders later reparse by
key. Create effects take declared artifact objects such as `PromptArtifact`,
`SkillBank`, or `DirectoryArtifact`; callers must not pass raw JSON objects and
the builder projects the typed artifact to a Plan literal only at the
public-seam boundary. Public skill-bank proposal output uses a typed `SkillBankChange`
discriminated union (`write_file`, `rename_file`, `atomic`, etc.) instead of a
free-form object bag; the builder projects that domain value to a Plan literal
only at the public-seam boundary. GitProgram and future adapter artifact/change
leaves still need concrete Python owners before they can count as fully typed
Python artifact semantics.

The builder geometry:

```python
# Single op, awaited directly
case = await cx.case.load(case_id, include=["input", "target"])

# Deferred: multiple ops batched into one Plan IR document.
# Do not expose `cx.batch()` until the SDK has a real batch accumulator/requester.
```

The batch context manager is the load-bearing ergonomic — it lets the
user write multiple operations as if they were independent calls while
the wire treats them as one transaction with one receipt root. Without
it, every effect is a separate round-trip. With it, the user expresses
intent declaratively and the wire is efficient. It is not a current public
Python SDK surface; a private placeholder module exists only to reserve the
ownership location for the future real implementation.

Runtime composition takes a single call:

```python
rt = lv.runtime(
    workspace=lv.workspace.local(root=".agents"),
    lm=lv.lm.anthropic(model="claude-opus-4-7"),
    agent=lv.agent.codex(model="gpt-5-codex"),
    sandbox=lv.sandbox.docker(image="python:3.12"),  # optional
    trust_profile="managed_sandbox",
    budget=lv.budget(usd=200, calls=2000),
    cache=lv.cache.sqlite_default(),  # optional; engine default if omitted
)
```

Workspace, LM, agent, sandbox: each is a builder that returns a typed
config the engine knows how to instantiate. Multiple LMs are allowed
(`lm=[anthropic(...), openai(...)]` with role binding). Multiple agents
are allowed (`agent={"executor": codex(...), "judge": claude_code(...)}`).
The trust profile bundles execution policy + capability defaults from a
fixed enum (`trusted_local_operator`, `managed_sandbox`, `package_scorer`,
`remote_untrusted`).

Optimizers come from a registry:

```python
opt = lv.optimizers.gepa(
    population_size=10,
    frontier=lv.frontier.top_k(3),
    parent_selector="round_robin",
    reflection_lm=lv.lm.anthropic(model="claude-opus-4-7"),
    minibatch_size=4,
    objective="instance",   # frontier axis over the rubric's reward vector
    reflect=lv.Reflect.fn(reflect),  # optional; defaults to GEPA's built-in
    propose=lv.Propose.agent_edit(agent=lv.agent.codex(model="gpt-5-codex")),
)
```

The optimizer config is a typed Python record that crosses the wire as
JSON. The engine instantiates the Rust optimizer with the config. The
Python user picks knobs; the engine runs the loop. Named rewards are the
objective dimensions; reward weights feed the aggregate scalar GEPA tracks per
candidate; reward feedback and output are the actionable side info projected
(target-safe) into the reflective dataset.

The `lv.optimize(...).run()` entry point composes everything:

```python
result = await lv.optimize(
    seed=lv.SkillBank.empty(),
    environment=lv.Environment(task=task, rollout=rollout, rubric=rubric),
    optimizer=opt,
    runtime=rt,
).run()
# Train / validation / test splits come from `Case.split` on the task.
```

The result is typed `Optimized[Artifact]`:

```python
result.best                    # Candidate[Artifact]
result.frontier                # list[Candidate[Artifact]]
result.summary                 # RunSummary with cost, replayability, etc.
result.summary.run_dir          # Local run directory when persisted
result.test_assessments()      # Iterable[Assessment]
result.assessment(case_id)     # Assessment
lv.runs.inspect(result.summary.run_dir)  # RunInspection audit projection
await result.replay(case_id)   # ReplayResult, deterministic
```

Inspection of completed runs is the same surface, opened externally:

```python
run = lv.runs.open(result.summary.run_dir)
audit = lv.runs.inspect(result.summary.run_dir)
print(run.best.artifact.summary())
for assessment in run.test_assessments():
    print(assessment.score.value, assessment.case.id)
    print([(r.id, r.value, r.weight) for r in assessment.rewards])
for ancestor in run.lineage(run.best.id):
    print(ancestor.id, ancestor.proposal.summary())
print(audit.receipt_ids(kind="call"))
print([(r.id, r.value, r.weight) for r in audit.evidence[0].rewards])
print(audit.cost_status, audit.unsupported)
```

`lv.runs.inspect(...)` is a flattened read-only projection over the persisted
`Optimized` result. It names the best lineage, visible receipts, public
assessment evidence, per-assessment reward-vector dimensions, cost/usage
status, unsupported dependency facts, and when a local Rust checkpoint exists,
Rust-owned checkpoint/graph readback from `leaven run inspect --run-dir ...` in
one typed `RunInspection`. The current readback proves checkpoint and graph
blob retrieval; it does not yet claim full transcript/artifact blob byte fetch
or that `lv.runs.open(...)` is independent of the Python `optimized.json`
projection.

The same package serves three purposes: compose + configure + run; author
a stage; inspect after the fact. One install, one mental model.

## External adapters

External ecosystem adapters live under `lv.x.*` only after they execute through
typed Leaven seams. DSPy remains a future adapter candidate, not a current SDK
surface: there is no `lv.x.dspy`, `lv.dspy_context`, or `lv.dspy_acall` claim in
this slice. Reintroduce DSPy only with a real `dspy.BaseLM` implementation that
lowers into `leaven-lm` neutral request/response types, routes through the
public seam service, preserves receipts, and has executable example coverage.

The pattern generalizes: future adapters (`x.skill_bank.*`, `x.git_program.*`,
`x.inspect.*`) follow the same rule: typed integration with an external
ecosystem, lowered into core Leaven types, lifted back for the user, with no
public import-only or `NotImplementedError` scaffolds.

## The acceptance gate

A P5-shaped GEPA optimization with an agentic reflector runs through
the Python SDK end-to-end against a live LM provider, producing a
measurable `Optimized[Artifact]` with improvement over the seed. The
demo:

- Python `@lv.reflector` function materializing a workspace, running an
  agent, parsing typed output
- GEPA loop driving multiple iterations (parent selection, minibatch
  sampling, reflect, propose, evaluate, admit, accept)
- Real LM provider calls through `cx.lm.complete`
- Real agent calls through `cx.agent.run`
- Frontier admission, validation pareto state, capability tokens, data
  classes, receipts all preserved end-to-end
- Output queryable via `lv.runs.open(...)` with receipts, lineage,
  evidence intact

This is the only gate. A smaller demo (single Python evaluator, no
agentic reflector, no real optimizer loop) does not satisfy it. The
gate is unfakeable: it requires every load-bearing piece to be real.

## What success is not

These shapes look like success but are not the product. If implementation
produces any of them, the spec has been compromised:

- A pyo3 wrapper of `leaven-run` shipped as `leaven`. Bypasses the
  wire; recreates the rejected manylinux/ABI/language-lock failure mode.
- Worker-transport conformance rows promoted to `proven` from in-process Rust tests
  labeled as public-seam transport. The matrix explicitly names this as
  `fake_pass_rejected`.
- A working `@lv.evaluator` decorator with no `lv.optimize()` /
  `lv.runtime()` / `lv.optimizers.gepa()`. The stage-only framing
  this spec rejects.
- Typed Python records that don't validate against the same JSON Schemas
  the Rust engine uses. Typed lipstick on schemaless drift; recreates
  the MCP failure mode.
- Python SDK demoed against a mock engine only. Doesn't prove the wire
  carries capability / receipts / data classes across a real process
  boundary.
- A toy Python optimization without an agentic stage. Doesn't exercise
  the load-bearing complexity that the acceptance gate names.
- Implementation begun before this spec is locked. Re-creates the
  Rust-mistakes-repeated-in-Python failure that is the entire reason the
  spec exists.

## Public API discipline

The Python surface has two tiers and one rule: a user reading the source
must be able to tell which is which at a glance.

**Public.** Anything intended for ordinary user code. Accessed as `lv.Name`
or `lv.namespace.Name` (no leading underscore anywhere in the path). Listed
in the owning module's `__all__`. Documented in this spec. Survives across
versions per a stated deprecation policy. Pydantic models in the public
tier are frozen and `extra="forbid"`.

**Private.** Everything else. Convention: leading underscore on file name
(`_handles.py`, `_receipts.py`) or symbol name (`_RuntimeBuilder`,
`_CacheNamespace`). Not listed in any `__all__`. Not documented here.
May change between versions without notice.

The rule: **if it isn't in `__all__` and named without a leading underscore
in a public module, it does not exist.** Users importing through
`from leaven import *` see only the public tier; IDE-autocomplete tooling
that respects `__all__` matches. Reaching into private internals is a
self-inflicted wound; the implementation is free to break it.

Three sub-rules carry the weight:

1. **Every public module ships an `__all__`** listing exactly the names it
   intends to export. Modules without `__all__` are implementation detail
   even if they live in the public package tree.
2. **Submodules that exist as namespaces** (`lv.optimizers`, `lv.lm`,
   `lv.agent`, `lv.workspace`, `lv.sandbox`, `lv.cases`, `lv.frontier`,
   `lv.output`, `lv.scoring`, `lv.trust`, `lv.runs`, `lv.x`,
   `lv.data_class`, `lv.artifacts`, `lv.layouts`, `lv.setup`) are listed
   in the top-level `__all__`. Submodules that
   leak into `dir(leaven)` because of import machinery (`leaven.case`,
   `leaven.assessment`, etc. — files behind public types) are NOT in
   `__all__` and users should access the type via `lv.Case`, `lv.Assessment`
   directly. The file path is private; the name on the package is public.
3. **No backdoor exports.** A public-tier function or class must not require
   importing from a private module to use. If a user needs `_InternalThing`
   to compose `lv.public_thing`, that's a design failure — either promote
   `_InternalThing` to public or hide it behind a public-tier shim.

Linting policy: ruff's `RUF022` (sorted `__all__`) is on by default in the
`leaven` package's ruff config. There is no current ruff rule for
"public module must declare `__all__`"; this discipline is enforced by
review against this spec section and by the convention rule in
`sdk/python/AGENTS.md`. If a useful lint surfaces (e.g.,
`__all__` completeness check vs `dir(module)` minus underscored), turn it
on in the same change that proposes it.

Deprecation policy for V1: any removal or signature change to a public
symbol passes through one minor release with a `DeprecationWarning` before
the breaking change lands. Private symbols may change at any time without
warning.

## Constraints on implementation

The implementation must honor:

- **Don't repeat Rust UX failures.** The user surfaced specific friction
  with Rust Leaven setup; the rust-leaven-usability sibling note
  captures these as they're enumerated. Every Python surface must be
  readable as "this is what the user writes" without requiring
  cross-file inference or implicit conventions. EXPLICITLY_UNMODELED is
  more complete than silence.
- **Don't expose engine internals.** `RunContext`, `RunGraph`, internal
  optimizer state, internal store handles, internal cache keys are not
  Python concepts. Python types mirror seam types only.
- **Don't add hidden APIs.** Anything `pub` in the Python sense is in
  the spec. No private functions doing public work. No `_internal`
  modules that users discover and depend on.
- **Don't proliferate ways to do one thing.** Per the user's stated
  preference: one canonical way. Builders, not free-form construction.
  Explicit stage objects for composition; decorators only as function-stage
  authoring sugar. Typed records, not unstructured maps.
- **Don't ship without typing.** Pydantic models or dataclasses with
  full type hints. `Optimized[A]` is generic; IDE autocomplete works
  on `result.best.summary()` without guessing.
- **Don't hide Python dependencies.** `sdk/python/pyproject.toml` is the owning
  declaration for public runtime dependencies, public optional extras, private
  dev tooling, and private runtime dependencies. Live provider examples use the
  Rust public seam service for provider execution; adding a Python provider
  dependency requires moving it into the declared dependency boundary in the
  same change.
- **Don't bundle benchmark catalogs.** `lv.cases.from_jsonl`,
  `lv.cases.from_iterable`, and `lv.cases.from_csv` are generic loaders.
  Paper-specific catalogs
  (OfficeQA, SealQA, BrowseComp) live in separate
  `leaven_benchmarks_<name>` packages users opt in to.
- **Don't depend on the upstream ACP SDK for the V1 SDK seam.** Path B per the
  conversation; reasons in the wire section above. Upstream ACP can return only
  as an explicit agent-provider interoperability adapter.
- **Don't language-lock the wire.** Any decision that would make a TS or
  Go SDK awkward later is a failure. The wire's properties
  (snake_case, no `null` literals, JS-safe integers, opaque cursors,
  kind-discriminated unions) are tested by the multi-language audit
  at `docs/working-memory/leaven-py-research/2026-05-24-multi-language-future-proofing.md`.
- **Don't add a ContextVar-based implicit current-context lookup.**
  Pydantic-ai and BAML both expose `get_current_run_context()` via
  `contextvars.ContextVar` so library code can reach the context without
  explicit threading. We pass `cx` as an explicit parameter to every
  stage function and library helper. The cost of implicit context isn't
  worth it for an audit-first system: explicit `cx` makes capability
  authorization, receipt threading, and replay determinism traceable
  by reading the call site. Per the synthesis of vendored references
  (`docs/specs/leaven_py/docs/agent-context/patterns/SYNTHESIS-2026-05-24.md`),
  marvin's pattern agent surfaced this directly: *"implicit ContextVar
  thread binding conflicts with Leaven's RunContext capability-token
  model."*
- **Don't add a Python sandbox at the stage level.** Temporal's sandbox
  (RestrictedPython + builtins patching) has a large attack surface
  with limited security benefit; per the vendored-references synthesis,
  temporal's pattern agent recommended *"Current Leaven design (trusted
  Python, no sandbox) avoids the risk entirely — keep it."* Trust
  profiles are policy declarations the engine enforces at capability
  boundaries (LM, agent, sandbox, workspace); the engine sandboxes
  effects, not arbitrary Python code at the stage body.
- **Don't put context fields that are optional across roles.** If a context
  carries a field present for some roles and absent for others (e.g.
  pydantic-ai's `tool_manager`, present in `RunContext` but absent in
  `TemporalRunContext`), user code that reads it silently fails on the role
  that omits it. This is exactly why the role contexts (`RolloutContext`,
  `RubricContext`, `ReflectContext`, `ProposeContext`, `JudgeContext`,
  `EvaluatorContext`) are separate types: a capability either exists on a
  role's context or it does not — surface
  it through a builder method that raises explicitly when unavailable
  in the current execution mode.

The opaque-Rust-refs-with-typed-payloads design of our receipts
(`CallReceipt`, `QueryReceipt`, `WriteReceipt`) was independently
validated as the right shape by parallax across BAML, temporal, and
jupyter-client pattern observations. Temporal's pattern agent
characterized it: *"PyO3 module boundary is request/reply, not shared
state."* Python never holds Rust internals — only opaque handles plus
serialized payloads. Forging a receipt is not possible; threading one
through evidence and assessments is the only path.

## What governs disagreement

This spec governs Leaven Python. The locked public seam V1 at
`docs/specs/public-seam-v1/` governs the wire. If this spec and the
locked seam disagree about wire behavior, the seam wins; this spec is
revised to match. If this spec and any implementation artifact (plan,
working-memory note, code, test) disagree about what the product is, the
spec wins; the artifact is revised.

Updates to this spec require explicit revision with rationale. The spec
is not implementation documentation; it does not change because
implementation found a shortcut. Implementation that requires a spec
change pauses and proposes the change.

## What this spec does not say

Per the discipline that EXPLICITLY_UNMODELED is more complete than
silence: this spec deliberately does not specify:

- **How `leaven-acp` is implemented.** Module layout, async dispatch
  shape, cancellation propagation pattern, bounded-queue mechanics.
  These are downstream from the wire contract; implementation is free.
- **Distribution mechanics.** Wheel layout, per-platform binary
  bundling, CI matrix, PyPI publication policy. These are real concerns
  but they are downstream from the product shape; if the Python user
  can `pip install leaven` and write the code above, distribution
  succeeded.
- **Implementation tranche ordering or timeline.** The conversation
  that produced this spec produced day-count estimates (6-8 days for
  `leaven-acp` async rewrite; 1-2 days for `leaven-types` codegen) but
  those are estimates for downstream planning, not product claims.
- **Test names.** Implementation chooses test names. The spec names
  the gates (acceptance gate above, conformance matrix rows in the
  locked seam) that tests must satisfy.
- **Phase ordering.** There is no Phase 0 / Phase 1 split. The product
  is the product; implementation orders its own work.
- **Windows support.** Not in V1; revisited when V1 ships.
- **TypeScript or Go SDKs.** Future spec slices when those land.
- **Bring-up code reduction.** V1 targets live-run paper code; bring-up
  is a separate larger surface a future spec may address with typed
  bring-up primitives.

## Provenance

This spec was authored on 2026-05-24 during a long design conversation
that walked through:

- The public seam V1 maturity snapshot at authorship time (32/39 conformance
  rows proven; 3 blocked on the then-named worker transport; 4 pending on runtime
  row work). The transport terminology is now Leaven worker profile.
- The archived `COMPREHENSIVE_DESIGN_PASS_NOTES.md` from the pre-lock
  seam draft, which named pyo3 as rejected at line 29, evaluator
  interior as host-language at line 33, the 200-line target at line 21,
  and the deferred DSPy drop-in shape at line 735. Most decisions in this spec
  are restatements of design positions reached during the seam lock.
- Four research files produced by parallel research agents on EvoSkill
  glue, worker-transport/ACP SDK code inventory, multi-language future-proofing, and
  external-worker prior art. Findings are captured in
  `docs/working-memory/leaven-py-research/`.
- Five concrete Python sketches at
  `docs/working-memory/leaven-py-research/2026-05-24-python-surface-sketches.md`,
  which surface the open questions this spec resolves.
- A goal-handoff alignment checkpoint with the user that confirmed the
  intent and acceptance gate.
- The user's correction that the "200-line Python evaluator/composition"
  reframing was too conservative — live-run paper code IS ~200 lines,
  one-time bring-up is a separate surface.
- The user's correction that the acceptance gate should be a P5 GEPA
  with agentic reflector, not "one Python evaluator runs against live
  LM."
- The user's correction that this artifact is a spec, not a plan — what
  is wanted, declared clearly, drawn from the lived experience of the
  conversation.

The goal handoff artifact at
`docs/working-memory/leaven-py-and-acp-transport-handoff.yaml` is the
operational package against this spec; it names the acceptance rows
implementation must prove. The sibling note at
`docs/working-memory/rust-leaven-usability.md` captures Rust-side
friction that this spec must not repeat in Python.
