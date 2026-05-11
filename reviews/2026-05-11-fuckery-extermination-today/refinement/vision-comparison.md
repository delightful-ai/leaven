# Vision Comparison

Status: integrated refinement pass.

## Short Answer

The first-pass audit is directionally right, but it still reads too much like a
catalog of local smells. Against the original Leaven vision, the deeper failure
is this:

Leaven is supposed to be a Rust optimizer library where ordinary users run
optimizers, customizers swap optimizer strategy slots, and optimizer authors
build new paper-shaped optimizers over `RunContext`. The current surface often
names those layers, but does not preserve their contracts end to end.

The original vision's final thesis is explicit:

> A Rust optimizer is a configured value that drives a typed run graph by
> proposing changes to artifacts, requesting assessments, interpreting evidence
> through preference relations, and maintaining live populations, while the
> engine provides budgeted, observable, capability-scoped execution.

That is from `docs/specs/initial_library.md:4753-4759`. The current audit
should be judged against that sentence, not only against whether individual
crates compile or examples show numeric movement.

## Original Vision Anchors

### Three User Tiers

The original spec says Tier 1 users call:

```rust
optimize(seed_prompt)
    .train(train_cases)
    .score(|ctx| async move { ... })
    .using(Gepa::default().with_reflection_lm(lm))
    .budget(...)
    .run()
    .await?;
```

Evidence: `docs/specs/initial_library.md:3608-3622`.

Tier 2 users customize GEPA through strategy slots without writing a new
optimizer: surface, proposer, parent selector, part selector, population, batch
sampler, validation, and related policy. Evidence:
`docs/specs/initial_library.md:3624-3642` and
`docs/specs/initial_library.md:475-485`.

Tier 3 users implement new optimizers from papers over `Optimizer` and
`RunContext`. They must be first-class; if TextGrad, MIPRO, pairwise
tournaments, or AlphaEvolve must contort into GEPA, the design failed. Evidence:
`docs/specs/initial_library.md:487-509`.

### Public Story Must Stay User-Intuitive

The GEPA public/private spec rejects implementation-facing public nouns:
evaluation specs, split usage rules, actor scopes, request templates,
visibility policy, and run graph internals. The ordinary public story is:

```text
Give Leaven a candidate, training work, a scoring function, an optimizer, and a budget.
Optionally give it validation/test work and swap GEPA strategies.
```

Evidence: `docs/specs/gepa_public_private_surface.md:20-47`.

### Rust Library Standard

The library standard is precise names, honest types, explicit failure, typed
capability boundaries, typed events, async by default, ergonomic builders, laws,
and examples that show real optimizers. Evidence:
`docs/specs/initial_library.md:511-529`.

The first-pass audit should therefore distinguish two problems:

1. a missing implementation detail;
2. a public name that lets users believe the vision is already implemented.

The second is worse.

### Naming Is Infrastructure

The original spec says "Naming is not polish; it is infrastructure." Evidence:
`docs/specs/initial_library.md:531-544`.

This matters because the current audit found terms like `CandidateSelector`,
`Gate`, `CachedLm`, fixed `ReflectiveMutation`, public derive macros, and
standard placeholder structs. The issue is not only taste. Those names steer
future implementors into the wrong model.

## Where The First Audit Is Strong

### It Caught Proxy Product Proof

The first audit correctly identifies `p8_aime_gepa` as a false-positive proof:
the solver bypasses Leaven LM, and the reflector is a fixed edit. That directly
violates the original implementation plan's prototype progression, where GEPA
parity is supposed to reproduce Python GEPA naturally on top of earlier
`Optimizer + RunContext + RunGraph`, pairwise, and eval substrate work. Evidence:
`docs/specs/initial_library.md:4634-4685`.

### It Caught Layer Leakage

The ordinary prelude exports engine-author names even though Layer 1 users
should not touch `RunGraph`, `TrustPolicy`, `EvaluationRequest`, `Population`,
selectors, or evidence stores. Evidence:
`docs/specs/gepa_public_private_surface.md:49-83`.

### It Caught GEPA Context Loss

The GEPA step contract requires batch sampling, parent evaluation, captured
feedback assessment IDs, reflective mutation, proposal provenance, child
evaluation, acceptance, validation policy, and population observation. Evidence:
`docs/specs/gepa_optimizer_surface.md:320-357`.

The first audit correctly says current reflection cannot see enough context to
be GEPA.

## Where The First Audit Needs Refinement

### R1: It Understates The Three-Tier Contract

The first audit has layer folders, but the integrated narrative should say that
each layer has a different contract:

- Layer 1 is an ergonomic product builder contract.
- Layer 2 is a GEPA strategy-slot contract.
- Layer 3 is a first-class optimizer-author contract.

The current implementation partially violates all three at once. If we only fix
the AIME example, we can still leave the deeper library vision broken.

### R2: It Needs To Separate "Milestone Fixture" From "Public Lie"

The original GEPA milestone plan explicitly allowed a deterministic proposer in
an intermediate milestone. Evidence: `docs/specs/gepa_optimizer_surface.md:692-702`.

That means deterministic proposal is not inherently wrong. It becomes wrong
when it is named `ReflectiveMutation`, exposed as the public reflector, or used
as off-the-shelf proof of GEPA. The refined audit should use that distinction
everywhere:

- private or clearly named fixture: acceptable scaffolding;
- public production-looking capability: blocker.

### R3: It Needs To Treat Score As A Product Facade, Not Core Truth

The original nomenclature says `Assessment` is the evaluation result and
`Evidence` is the opaque payload; `Score` is not a cold primitive. Evidence:
`docs/specs/initial_library.md:559-564` and
`docs/specs/initial_library.md:1445`.

The public `.score(...)` function can remain user-intuitive, but it must lower
into assessments/evidence/preference without collapsing the library model into
scalar scores.

### R4: It Needs To Preserve Eval / Dataset / Environment Separation

The eval lowering spec defines:

```text
User input        = train/validation/test cases, runner/scoring function/evaluator, optimizer.
Lowered eval data = dataset, splits, split-use plan, request templates, reports.
Execution         = engine evaluator calls, graph mutation, cache, budget.
Environment       = optional workspace/agent/process substrate for an evaluator.
```

Evidence: `docs/specs/eval_lowering_detail.md:24-37`.

The first audit finds pieces of this, but the refinement should make it a
top-level requirement: do not solve score/running/datasets by inventing one
large "eval spec" user concept, and do not move agentic task semantics into
`leaven-eval`.

### R5: It Needs To Include Paper-Optimizer Pressure Tests Beyond GEPA

The original plan intentionally put pairwise tournament before GEPA parity to
stress what Python GEPA does not cover. Evidence:
`docs/specs/initial_library.md:4634-4668`.

The refined audit should warn that "make AIME go up" is necessary but not
sufficient. It proves one GEPA path only if it uses the true surface. It does
not prove optimizer-author generality.

### R6: It Needs To Say GEPA Is One Optimizer, Not The Library

The GEPA refinement report makes this sharper: GEPA is a product-critical
optimizer, but it is not Leaven's whole abstraction. The original design says
GEPA, MIPRO, TextGrad, and future papers should each be optimizer values over
the same substrate. Evidence: `docs/specs/initial_library.md:4753-4759`.

That means the GEPA correction cannot be "put more GEPA hooks into engine" or
"make every optimizer follow GEPA's loop." The correction is:

- `leaven-run` owns ordinary builder ergonomics and train/validation/test
  lowering;
- `leaven-gepa` owns GEPA rhythm and strategy slots;
- `leaven-engine` owns graph, trust, budget, events, cache, and finalizing
  context methods;
- vocabulary crates own reusable evidence/preference/population/rendering
  pieces without knowing GEPA rhythm.

The first-pass audit says this in pieces. The refined audit should make it the
central contract.

### R7: It Needs To Treat Engine/Eval As A Prerequisite, Not A Later Cleanup

The engine/eval refinement report points out that GEPA cannot be honest until
the substrate can preserve evidence, trust, budget, cache, and finalization
semantics. The original actor/capability table says optimizers may use
`RunContext`, proposers receive `ProposalContext`, evaluators receive
`EvaluationContext`, renderers receive `RenderContext`, and materializers
receive `MaterializeContext`; each must be scoped and finalizing behavior must
remain centralized. Evidence: `docs/specs/initial_library.md:2740-2760`.

The refined conclusion:

- raw contexts that bypass finalization are not an implementation detail; they
  violate the optimizer-author contract;
- cache hits must still leave graph-visible request/assessment lineage;
- hidden split policy must be enforced after resolution, not only on the
  syntactic evaluation-set expression;
- score/evidence/preference collapse is a substrate problem, not just a GEPA
  problem.

### R8: It Needs To Separate Boundary-Map Alignment From Product Alignment

The crate-graph refinement report adds a useful distinction: a crate graph can
match the topology while the product surface still lies. A workspace member,
`src/lib.rs`, or feature flag proves only that the boundary exists. It does not
prove the public name is mature enough for ordinary import paths.

The maturity test is default-facing public API:

- Does `leaven::prelude::*` expose only real ordinary contracts?
- Do default features expose no compile-error derives?
- Does `leaven-std` re-export only behavior-bearing standard pieces?
- Do provider/backend features expose usable adapters rather than empty public
  structs?
- Are fixtures visible as fixtures, not as production reflectors?

The first audit's placeholder ledger should therefore become a public-maturity
gate, not only an inventory.

## Refined Root Diagnosis

The root problem is not just missing code, and not just bad naming. The root
problem is contract substitution:

1. scaffolded crate graph substitutes for implemented capability;
2. deterministic fixture substitutes for reflective mutation;
3. shell-out live solver substitutes for provider-neutral LM;
4. scalar `Score` substitutes for assessment/evidence/preference;
5. public raw contexts substitute for `RunContext` finalization;
6. coverage/examples substitute for product proof.

Every correction should remove one substitution and route the behavior through
the concept the original vision already named.

## Agent Report Cross-References

- `agent-reports/layer-1-original-vision.md`: adds single-task/no-dataset,
  score-vs-reward, environment ownership, result facade, and ordinary import
  surface refinements.
- `agent-reports/gepa-original-vision.md`: adds GEPA scope discipline,
  per-slot contract pressure, nomenclature classification, and private-state
  discipline.
- `agent-reports/engine-eval-original-vision.md`: adds engine/eval
  prerequisite status, cache-hit graph semantics, score/evidence/preference
  collapse, and trust-finalization gaps.
- `agent-reports/crate-graph-original-vision.md`: adds public maturity
  categories, default facade risk, and topology-test recurrence prevention.
