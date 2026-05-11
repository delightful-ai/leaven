# Known Findings Ledger

This file records findings already established before the broader crate audit.
Severity is about product-contract risk, not code size.

## F-001: GEPA Reflection Bypasses Engine Proposer Context

Severity: critical

Evidence:

- `crates/leaven-engine/src/stage/proposer.rs:28` defines the real
  graph-aware `Proposer<P>` trait.
- `crates/leaven-engine/src/context/proposal_context.rs:27` exposes
  `ProposalContext::graph()`.
- `crates/leaven-engine/src/context/run_context.rs:191` wires
  `RunContext::propose(...)`.
- `crates/leaven-gepa/src/proposer.rs:7` defines a separate `SurfaceProposer`
  that only accepts `artifact`, `surface`, and `part`.
- `crates/leaven-gepa/src/optimizer.rs:536` uses the narrow GEPA-local
  proposer path when proposing a candidate.

Why this is bad:

GEPA reflection must be able to inspect evaluation feedback, traces, selected
examples, candidate lineage, and graph context. The current trait cannot even
name those inputs. This makes a real LM or agent reflector impossible without
more ad hoc escape hatches.

Required correction direction:

GEPA reflection needs a request/context shape that carries selected candidate,
selected part, current part text, selected scored trace/evidence, objective or
background, budget, and scoped graph access. This should either be implemented
through the engine `Proposer<P>` seam or through a GEPA reflector trait that is
equally honest and async.

## F-002: `ReflectiveMutation` Is A Fixed-Edit Fixture Named Like Real Reflection

Severity: critical

Evidence:

- `crates/leaven-gepa/src/proposer.rs:21` calls `ReflectiveMutation` a
  deterministic fixture.
- `crates/leaven-gepa/src/proposer.rs:27` stores one edit and always returns
  it.
- `examples/p8_aime_gepa/src/main.rs:91` uses that fixture to replace the
  prompt with a hard-coded optimized prompt.

Why this is bad:

The name implies the GEPA reflective mutation stage exists. The implementation
is a canned edit. This directly enabled the AIME example to look like optimizer
progress while not proving reflection.

Required correction direction:

Rename the fixture to something explicit such as `FixedEditProposer` or move it
to tests/examples. Reserve `ReflectiveMutation` for a real async reflector that
uses trace/evidence and can be backed by an LM or an agent.

## F-003: AIME Example Has A Live Solver Escape Hatch Outside Leaven LM

Severity: high

Evidence:

- `examples/p8_aime_gepa/src/main.rs:236` switches on
  `LEAVEN_AIME_LIVE_OPENAI`.
- `examples/p8_aime_gepa/src/main.rs:253` shells out to
  `scripts/openai_solver.py`.
- `examples/p8_aime_gepa/scripts/openai_solver.py` calls OpenAI Responses API
  directly with `urllib`.

Why this is bad:

The example claims to be the Leaven public API example, but the live provider
path bypasses `leaven-lm`, `leaven-lm-openai`, and `leaven-lm-cache`. This
means the example does not prove the provider-neutral LM crate, the OpenAI
lowering crate, or the response-cache crate.

Required correction direction:

The live solver must use `OpenAiLm` through the Leaven `Lm` trait and cache
policy through a Leaven-owned runtime/cache configuration. No Python provider
escape hatch should be the canonical live path.

## F-004: Public `CachedLm` Wrapper Smells Like Implementation Policy Leaking Outward

Severity: high

Evidence:

- `docs/specs/lm_runtime_and_response_cache.md` presents public user code that
  manually wraps an LM in `CachedLm::read_write(...)`.
- `crates/leaven-lm-cache/src/cached.rs` exposes `CachedLm<M, C>` as the main
  composition type.
- The umbrella crate can re-export `leaven_lm_cache` behind the `lm-cache`
  feature.

Why this is bad:

For ordinary users, caching should be configuration on an LM runtime or run
configuration, not a public wrapper type they must stack manually. The current
shape turns an implementation composition detail into the product API and makes
the user think in wrappers instead of capabilities.

Required correction direction:

Keep cache backends and cache policy swappable, but hide ordinary composition
behind a builder or runtime configuration. Power users can still reach cache
traits/backends, but default examples should not teach `CachedLm` as the core
LM surface.

## F-005: `leaven-run` Runner And Scorer Are Synchronous

Severity: high

Evidence:

- `crates/leaven-run/src/builder.rs:28` defines `Runner<A, C>` as
  `Fn(&A, &C) -> RunOutput`.
- `crates/leaven-run/src/builder.rs:29` defines `Scorer<A, C>` as
  synchronous `Fn(...) -> Score`.
- `crates/leaven-run/src/evaluator.rs:97` calls the runner synchronously per
  case inside an async evaluator.

Why this is bad:

LM programs, agentic runs, remote workspaces, and model judges are naturally
async. The current API pushes examples toward `block_on`, process spawning, or
hidden runtimes. It also blocks proper bounded concurrency for benchmark runs.

Required correction direction:

Add or cut over to async runner/scorer/evaluator closures with explicit
bounded concurrency. Preserve simple sync ergonomics only if they lower into
the async path without splitting semantics.

## F-006: Evidence Payload Access Is Split From Proposer Graph View

Severity: medium-high

Evidence:

- `ProposalContext` exposes graph and render/materialize contexts.
- `RunGraphView` exposes assessment records and evidence refs.
- `RunContext::assessment_evidence(...)` is the current direct evidence payload
  accessor.
- `ProposalContext` does not expose an evidence reader.

Why this matters:

Even if GEPA switched to engine `Proposer<P>`, a proposer can see that evidence
exists but cannot directly retrieve the typed payload from the proposer context.
GEPA can work around this by having the optimizer read evidence before calling
the proposer, but the general optimizer-author story should be explicit.

Required correction direction:

Decide whether proposer-stage code may read evidence payloads directly. If yes,
make that capability explicit and scoped in `ProposalContext`. If no, GEPA must
own the evidence-to-reflection-request lowering inside the optimizer and pass a
complete request to the reflector.

## F-007: OpenAI Constructor Accepts A Default Model It Does Not Use

Severity: medium

Evidence:

- `crates/leaven-lm-openai/src/client.rs:29` documents `from_env(default_model)`.
- `crates/leaven-lm-openai/src/client.rs:34` names the argument
  `_default_model`.
- Requests carry their own explicit model through `LmRequest`.

Why this is bad:

The API suggests that model selection is stored in the provider, but the value
is ignored. That is small but corrosive: users build the wrong mental model,
and fingerprints cannot honestly include the default model because it is not
state.

Required correction direction:

Either make `OpenAiLm` own a default model and provide request helpers that use
it, or remove the argument from `from_env`.

