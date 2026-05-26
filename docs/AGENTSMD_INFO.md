# AGENTS.md Rubric

Status: durable rubric for Leaven's stackable `AGENTS.md` hierarchy.

An `AGENTS.md` file is executable context for a repo slice. It should let a
future contributor route work correctly with only the relevant code plus the
stacked `AGENTS.md` files from root to the working directory.

## Semantic Bar

Each applicable stack should answer:

- what this location owns
- what neighboring location must refuse the work
- which invariants govern this slice
- which local pattern is safe to copy
- which nearby code or doc is stale, bait, scaffold, or historical
- which command proves the local claim
- when this `AGENTS.md` itself must be updated

There is no required universal heading list. Use headings that make the local
decision clear.

## Placement Rule

Put a rule at the highest level where it is simultaneously:

- true for the whole subtree
- actionable at that level
- stable enough not to churn with local implementation details

Duplicate child guidance only when the child adds a real local delta. If the
parent rule is sufficient, delete the child repetition.

## Strong Files

A strong `AGENTS.md` changes decisions. It names:

- boundary: what belongs here
- routing: where related work goes instead
- hazards: what not to imitate
- proof: the narrow command and what it proves
- maintenance: when to update the file

It is weak if it is only a command catalog, only philosophy, or only local
trivia.

## Leaven Defaults

- Root `AGENTS.md` owns repo-wide architecture, evidence ladder, topology map,
  hard-cutover policy, and global verification expectations.
- Subtree `AGENTS.md` files should add local ownership, refusal rules, hazards,
  and proof anchors.
- Leaf `AGENTS.md` files should stay small and tactical; do not create them
  unless they change behavior for that leaf.
- Durable behavior belongs in specs, code, tests, crate docs, or the nearest
  owning `AGENTS.md`. Plans and audits are evidence, not governing law.
- If an audit exposes false proof, encode the warning at the proof site too.

## Checklist For Changes

Before adding or editing an `AGENTS.md` file, check:

- Could the parent say this once for the whole subtree?
- Does this file name a wrong-but-tempting destination?
- Does it point at a current proof anchor, not stale inventory?
- Does it distinguish topology proof from product maturity?
- Does it say when the guidance should change or disappear?

