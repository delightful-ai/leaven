---
name: leaven-goal-handoff
description: Use after substantial Leaven planning/spec work and before setting or launching a long-running /goal, especially for public APIs, optimizer/eval/agentic surfaces, paper reproduction, crate topology, or any task where user intent could be replaced by a nearby provable proxy.
---

# Leaven Goal Handoff

Use this skill at the node after planning/spec alignment and before starting a
durable implementation goal.

The job is to preserve intent across handoff. Do not turn this into another big
planning phase. Produce a short handoff frame, then either set/draft the goal or
return to planning.

## Core Failure To Prevent

Avoid proxy substitution:

> A nearby provable artifact stands in for the thing the user actually meant.

Common proxies:

- runnable example instead of usable public API;
- internal substrate instead of off-the-shelf product surface;
- paper-shaped demo instead of paper reproduction;
- spec consistency instead of end-user ergonomics;
- local crate DAG instead of full planned workspace topology;
- passing gate whose denominator does not include the intended surface.

## Workflow

1. Restate the original intent in one sentence.
   - Use the user's product question, not the latest internal abstraction.
   - If the intent is unclear, ask one focused question.

2. Restate the designed surface.
   - Name the docs/specs produced.
   - Name the public API or user workflow the design now implies.
   - Name the private/lowered machinery only if it affects the handoff.

3. Check intent preservation.
   - Say whether the designed surface still answers the original intent.
   - If not, stop and return to planning/spec revision.
   - Do not set a goal against a design that no longer preserves intent.

4. Name misleading proxy proofs.
   - List the artifacts that would look like success but would not satisfy the
     original intent.
   - Be concrete: examples, tests, crates, mock-only paths, partial datasets,
     or internal contracts.

5. Define the implementation acceptance path.
   - State what a user should be able to do when the goal is complete.
   - State the proof denominator: which tests, examples, live smokes, coverage,
     docs, or API imports must exercise the intended surface.
   - State explicit non-goals.

6. Decide.
   - If the design is wrong or incomplete for the intent: return to planning.
   - If the design preserves intent: draft or set the goal against this handoff
     frame.

## Handoff Frame

Use this compact shape:

```text
Original intent:
Designed surface:
Intent preservation:
Misleading proxy proofs:
Spec revisions before goal:
Acceptance path:
Proof denominator:
Explicit non-goals:
Decision:
```

Keep it short. The frame is not the implementation spec. It is the translation
from specification to goal.

## Goal Guidance

When drafting the `/goal`, do not paste the whole handoff frame unless the user
explicitly asks. Compress it:

- point to the governing specs;
- name the intended user-facing outcome;
- forbid the most tempting proxy proofs;
- require verification against the proof denominator;
- require honest closeout for explicit non-goals.

If the user asks to activate the goal, use the goal tool only after this frame
has a clear `Decision: set goal`.

## Closeout Reminder

Before marking a goal complete, compare the final work to the handoff frame:

- user-facing outcome exists;
- proof denominator actually covered the intended surface;
- misleading proxy proofs were not used as completion evidence;
- non-goals were not claimed as achieved.
