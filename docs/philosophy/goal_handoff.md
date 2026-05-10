# Goal Handoff: Preserving Intent Across Planning, Goals, and Closeout

Long-running Leaven work often begins with a product-shaped question, moves
through research and specification, and then gets handed to a goal-driven
implementation run. The fragile point is not the implementation itself. The
fragile point is the handoff after planning, when the design is concrete enough
to execute but still needs to be checked against the original intent.

This note records the failure pattern we saw in recent optimizer and agentic
planning work and turns it into a reusable pre-goal workflow.

## What We Found

The work was not generally failing because agents stopped early or ignored the
specs. Most runs produced useful code, docs, tests, and verification evidence.
The recurring problem was more subtle:

> A nearby provable artifact stood in for the thing the user actually meant.

Examples:

- "Leaven is an off-the-shelf optimizer library" became "Leaven has optimizer
  substrate and runnable examples."
- "GEPA can be used through Leaven's public surface" became "a GEPA-shaped
  milestone or AIME fixture runs."
- "Paper reproduction" became "a deterministic pressure test captures key paper
  motifs."
- "Full crate graph" became "the foundational crate DAG compiles."
- "Public evaluation UX is intuitive" became "the lowered eval contracts are
  precise."

Those substitutes were not useless. They often represented real progress. They
were wrong only when treated as proof of the original intent.

## Probable Root Cause

The root cause is proxy substitution during handoff.

Planning docs tend to answer:

- What should exist?
- Which crates own which facts?
- What traits, types, errors, and invariants should preserve the design?
- What behavior must implementations satisfy?

Those questions are necessary, but they do not by themselves answer:

- Does the designed surface still preserve the original user intent?
- Which implementation proof would be a misleading proxy?
- What should the user be able to do after the goal completes?
- What claims should still not be made after a successful run?

When that translation step is missing, the goal text becomes overloaded. It
tries to be the spec, the acceptance contract, the proof ledger, and the
closeout standard at once. Long goal text does not fix the problem, because the
failure is not missing words. The failure is losing the distinction between the
designed implementation and the intent it is supposed to serve.

## What Helped

Several interventions were useful:

- Specs reduced implementation ambiguity.
- Crate topology checks caught ownership mistakes.
- Adversarial review caught dependency and boundary errors.
- Runnable examples forced some user-visible behavior into existence.
- Diff review found contract breaks after implementation.

Each helped at the layer it targeted. None fully solved intent preservation.

The missing move is an intent translation review after planning and before
setting the goal.

## The Handoff Node

Use this node after the planning/spec phase has produced enough shape to execute
and before starting a durable `/goal` implementation run.

The point is not to add a large new planning phase. The point is to ask whether
the design still answers the original question. If it does not, go back and
revise the specs before setting a goal.

The handoff should produce a short answer to:

```text
Original intent:
Designed surface:
Does the designed surface preserve the intent?
Likely misleading proxy proofs:
Spec revisions needed before implementation:
Implementation acceptance path:
Explicit non-goals:
Set goal or return to planning:
```

The most important field is "Likely misleading proxy proofs." It names the
nearby artifacts an agent is likely to complete instead of the actual intent.

For the GEPA optimizer work, examples would have been:

- A P3 parity example runs.
- A hand-written optimizer loop works.
- A mocked AIME fixture improves but no live-provider swap path exists.
- Eval lowering types are coherent while public scoring remains awkward.
- A paper-shaped harness exists without the paper's data, metrics, prompts, or
  model setup.

Naming those proxies before implementation makes premature completion harder.

## When To Use This

Use the handoff node for high-semantic-risk work:

- public API or library-surface design;
- "off-the-shelf" product claims;
- paper reproduction or benchmark claims;
- optimizer/eval/agentic workflow design;
- crate topology or "full graph" implementation claims;
- any goal where user intent is broader than a single local bug fix.

Do not force this workflow onto routine implementation where the claim is
already concrete, such as "fix this failing test," "add this field," or "rename
this module."

## Closeout Is Separate

The pre-goal handoff asks:

> Does the thing we are about to implement still preserve the original intent?

Closeout asks:

> Did the implementation satisfy the agreed goal?

Do not wait until closeout to discover that the goal was proving the wrong
thing. Closeout can catch failed execution. The handoff node catches wrong
translation.
