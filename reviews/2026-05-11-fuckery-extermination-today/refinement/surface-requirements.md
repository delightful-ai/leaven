# Surface Requirements After Vision Comparison

Status: integrated refinement pass.

This file states the public and private contracts the refined audit should
protect. It is not an implementation design. It is the bar a fix plan must
meet.

## Layer 1: Ordinary User Contract

Layer 1 should let a user run a real optimizer without learning engine internals.

### Required User Inputs

- seed artifact or program;
- train cases or one unscoped task;
- optional validation cases;
- optional test cases;
- runner/executor when the artifact cannot be scored directly;
- score function or evaluator;
- optimizer value, usually `Gepa`;
- budget;
- optional runtime/cache policy;
- optional store/resume policy.

Evidence: `docs/specs/gepa_public_private_surface.md:51-67`.

### Required Result Facade

The ordinary result must expose enough truth to debug and trust a run without
requiring `RunGraph`:

- best candidate and why it was chosen;
- stop reason;
- train/validation/test or single-task summary;
- case-level report with stable case IDs when cases exist;
- score/evidence refs, not copied hidden payloads;
- cost and cache summary;
- optimizer summary, such as GEPA frontier/admission/rejection state;
- whether validation influenced admission;
- whether test was final-report-only;
- absent/failed evidence distinguished from numeric zero.

### Forbidden Layer 1 Concepts

Layer 1 examples and ordinary prelude must not require:

- `RunGraph`;
- `RunContext`;
- `ReadScope`;
- `TrustPolicy`;
- `EvaluationRequest`;
- `ResolvedEvaluationRequest`;
- split-use policy types;
- population/frontier internals;
- parent/part selectors;
- evidence stores.

Evidence: `docs/specs/gepa_public_private_surface.md:69-83`.

### Score Function Contract

The ordinary user may call it "score" because that is intuitive. Internally it
must lower into assessment/evidence/preference concepts.

The score function must be able to:

- run asynchronously;
- receive candidate, case/task, output, trace/history, run error, and scoped
  budget/runtime handles through accessors;
- return a comparable primary score when the optimizer requires one;
- return natural-language feedback;
- return structured metrics/records;
- attach or reference files, transcripts, logs, and other evidence payloads;
- report metered scorer cost;
- fail with typed errors instead of sentinel values.

Current blocker: the live `Score`/`RunOutput`/`ScoreContext` shapes are too thin
and field-based.

### Runner Contract

The runner must be async and concurrency-safe. It must be able to represent:

- plain function/program execution;
- LM program execution through `leaven-lm`;
- agentic task execution through `leaven-agentic` / `leaven-agent`;
- subprocess/workspace execution through workspace/runtime adapters.

The runner must not force shelling out from examples to use OpenAI or other
providers.

### Dataset / Environment Contract

Layer 1 words are train, validation, test, cases, tasks, runner, and score.

Lowered dataset/split/report data belongs in `leaven-eval`; execution belongs
in `leaven-engine`; agent/workspace semantics belong in agentic/workspace
crates. Evidence: `docs/specs/eval_lowering_detail.md:24-64`.

Required behavior:

- stable case IDs when concrete cases are supplied;
- duplicate case rejection;
- disjoint split default;
- deterministic dataset/split/plan fingerprint;
- no dataset path for single-task or environment-only evaluation;
- hidden validation/test content from reflective proposers by default;
- final-test-only default.

Evidence: `docs/specs/eval_lowering_detail.md:650-721`.

### Runtime / Cache Role Contract

Layer 1 may use multiple model/runtime roles in one optimizer run:

- solver/program runner;
- reflector/proposer;
- scorer/model judge;
- agent runtime.

Each role needs independent provider, cache, and budget policy. `CachedLm` may
remain an advanced wrapper type, but ordinary examples should configure runtime
and cache by role rather than manually stack implementation wrappers.

## Layer 2: GEPA Customizer Contract

Layer 2 users want GEPA, not a new optimizer. They should swap GEPA parts
without losing the GEPA rhythm.

### Required GEPA Slots

- surface;
- parent selector;
- part selector;
- batch sampler;
- reflector/proposer;
- acceptance;
- validation policy;
- population/frontier;
- merge scheduler/proposer;
- stopper/configuration.

Evidence: `docs/specs/initial_library.md:475-485`,
`docs/specs/initial_library.md:3624-3642`, and
`docs/specs/gepa_optimizer_surface.md:273-284`.

### Required Reflector Input

The reflector/proposer contract must be async-capable and must receive enough
owned context to await safely:

- selected parent candidate;
- selected surface part and part view;
- feedback minibatch identity;
- assessment IDs;
- selected evidence refs or scoped payload/rendered views;
- natural-language feedback and trace excerpts when policy allows;
- attribution evidence for the selected part when available;
- objective/task background;
- budget/runtime/cache handles appropriate for reflection;
- provenance destinations for `informed_by`.

Evidence: `docs/specs/gepa_optimizer_surface.md:320-357` and
`docs/specs/gepa_optimizer_surface.md:447-483`.

### Required GEPA Invariants

- graph mutations go through `RunContext`;
- feedback uses assessment/candidate/external refs, not string metadata;
- validation/test content is hidden from reflective proposers by default;
- every accepted child carries causal lineage;
- every proposal records what it read through `informed_by`;
- rejection does not erase graph truth;
- population events remain optimizer opinions, not graph truth.

Evidence: `docs/specs/gepa_optimizer_surface.md:344-357`.

## Layer 3: Optimizer Author Contract

Layer 3 users build a new optimizer from a paper or idea. GEPA must not be the
only shape.

### Required Engine-Author Surface

An optimizer author must be able to:

- implement `Optimizer<P>`;
- own algorithm rhythm in `step`;
- use `RunContext` to apply proposals, request evaluations, render/materialize
  views, charge budget, and record events;
- read graph views without mutating graph internals;
- interpret assessments/evidence through preference/population state;
- choose final candidates through graph/population state;
- compose proposer/evaluator/renderer/materializer/agent stages without adding
  new core traits.

Evidence: `docs/specs/initial_library.md:487-509`,
`docs/specs/initial_library.md:579-581`, and
`docs/specs/initial_library.md:2740-2760`.

### Raw Context Rule

Raw stage contexts are not public finalization APIs. If they exist for object
safety or tests, they must be private/test-only or named as non-finalizing.
Public stage invocation must route through `RunContext` or an equivalent
finalizer that preserves graph, trust, cache, budget, and events.

### Public Finalizer Rule

Every costful stage path needs a public finalizing entrypoint or must be marked
non-finalizing and internal:

- proposal;
- evaluation;
- rendering;
- materialization;
- agent/runtime execution when routed through engine-controlled stages.

Finalizers must charge budget, emit events, preserve trust/read scope, and
record or link graph truth. Cache hits are not invisible returns; they must
still link the current request to the reused assessment/evidence identity.

### Evidence / Preference / Population Rule

Evidence is not preference. Scores are one evidence shape plus one preference
relation. Population is live optimizer state. A frontier is a kind of
population. The public and customizer surfaces must not collapse those concepts
into scalar averages.

Evidence: `docs/specs/initial_library.md:559-571`,
`docs/specs/initial_library.md:1375-1450`, and
`docs/specs/initial_library.md:2432-2436`.

### Minimum Engine / Eval Contract Before GEPA Can Be Trusted

- `leaven-eval` constructs stable datasets/splits/plans and does not execute;
- product builders lower split intent into engine trust policy;
- explicit case IDs cannot bypass hidden split policy after resolution;
- `RunContext` is the only public graph mutation/finalization path;
- proposer context can either load scoped evidence or receives complete scoped
  evidence views in its request;
- evaluation cache keys include request kind, granularity, purpose, pair/list
  semantics, candidate identities, case identities, and evaluator fingerprint;
- evidence, preference, and population tests prove non-scalar paths, not only
  scalar casewise scoring.

## Cross-Cutting Public Truth Rule

Public names must tell the truth. A name can be:

- a real contract;
- private scaffolding;
- test-support public;
- explicit scaffold feature;
- an explicitly named fixture/demo.

It must not be a production-looking public capability that contains no behavior.

This applies to:

- derive macros;
- provider/backend crates;
- `leaven-std` exports;
- renderers;
- evidence/preference vocabulary;
- GEPA config/merge/reflector names;
- examples and coverage gates.
