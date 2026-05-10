---
name: leaven-goal-handoff
description: Use after substantial Leaven planning/spec work and before setting or launching a long-running /goal, especially for public APIs, optimizer/eval/agentic surfaces, paper reproduction, crate topology, or any task where user intent could be replaced by a nearby provable proxy.
---

# Leaven Goal Handoff

Use this skill at the node after planning/spec alignment and before starting a
durable implementation goal.

The job is to preserve intent across handoff. Do not turn this into another big
planning phase or a user-authored form. First synthesize the load-bearing
interpretation in plain language, align on the few claims that matter, then
write the handoff artifact as the durable record.

For a checkable artifact after alignment, use
`templates/gepa_optimizer_handoff.yaml` as the filled example. Copy its shape for
new goal packages and update acceptance statuses/evidence as implementation
progresses.

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

1. Synthesize the original intent in one sentence.
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

6. Run the alignment checkpoint.
   - Present only the load-bearing interpretation, not the whole artifact.
   - Ask for corrections to intent, proxy risks, acceptance proof, and
     non-goals.
   - If the user corrects the interpretation, incorporate it before writing the
     artifact.

7. Decide.
   - If the design is wrong or incomplete for the intent: return to planning.
   - If the design preserves intent: draft or set the goal against this handoff
     artifact.

Do not silently fill and commit the artifact before this checkpoint. The agent
should have a strong synthesis by now; the checkpoint exists to catch subtle
intent drift, not to make the user re-plan the work.

## Handoff Artifact

Use a compact YAML artifact with this shape:

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

Keep it short. The artifact is not the implementation spec. It packages the
governing specs, translates them into acceptance claims, and stays live during
implementation and closeout.

The artifact records alignment. It is not the place where alignment happens.

Status values for acceptance items:

- `pending`: no implementation proof yet;
- `in_progress`: implementation or verification is underway;
- `proven`: evidence exists and is linked in the artifact;
- `blocked`: cannot be proven without a decision or missing dependency;
- `dropped`: explicitly removed from scope before the goal was set.

## Goal Guidance

When drafting the `/goal`, do not paste the whole handoff artifact unless the
user explicitly asks. Compress it:

- point to the governing specs;
- name the intended user-facing outcome;
- forbid the most tempting proxy proofs;
- require verification against the proof denominator;
- require honest closeout for explicit non-goals.

If the user asks to activate the goal, use the goal tool only after the artifact
has a clear `Decision: set goal`.

## Closeout Reminder

Before marking a goal complete, compare the final work to the handoff artifact:

- user-facing outcome exists;
- proof denominator actually covered the intended surface;
- misleading proxy proofs were not used as completion evidence;
- non-goals were not claimed as achieved.
