## Boundary
This subtree holds dated implementation plans and execution notes. Plans may contain useful grounding, task lists, and historical decisions, but they are not the current source of truth when specs, code, or tests disagree.

## Local Rules
- Before executing an old plan, re-read the governing specs it names and inspect the current code. Many plans were written against earlier topology or partially completed milestones.
- Treat `For Claude` or skill-preface lines as plan-local execution hints, not repo-wide instructions.
- Do not add durable architecture decisions only here. Promote stable decisions to `docs/specs/`, crate docs, tests, or the nearest owning `AGENTS.md`.
- When a plan is completed or superseded, prefer adding a short status note over rewriting history into a fake current plan.
- Keep plan checklists concrete: file paths, commands, and behavioral claims. Avoid evergreen doctrine that belongs in specs or root AGENTS.
- Raw reports, adversarial reviews, and subagent transcripts are evidence. Preserve them when they explain how a decision was reached, but do not ask future workers to rediscover "what the report meant"; promote the conclusion into the owning `AGENTS.md` or spec.
- If a recent audit under `reviews/` contradicts an older plan's success claim, treat the plan claim as stale until the owning code/spec/test is corrected.
- Do not use a plan closeout as a final proof gate. Closeouts can record commands that passed; the test contract decides what those commands prove.

## Decision Cards
- when: continuing work from a plan after a rebase or audit
  do: restate the current denominator in the plan or nearest coverage matrix, then update durable docs only for stable rules
  preserve: original plan chronology and raw evidence links
  avoid: editing old checklist prose until it reads like current law
  verify: path/command references and any named narrow gates still exist

- when: writing a new goal handoff or execution note
  do: identify public promise, actual proof path, and known proxy risks before the task is launched
  preserve: user intent that could be satisfied by a nearby false positive
  avoid: success criteria that can pass by adding placeholders, shell-outs, or example-only behavior
  verify: include both narrow iteration gates and the completion gate expected by `docs/testing/README.md`

## Verification
- Run the narrow commands named by the plan only after confirming they still exist.
- If a plan touches a milestone example, use the matching `just milestone-pN` command rather than `cargo check --workspace --examples`.
- If a plan touches crate boundaries, run `cargo test -p leaven --test topology_contract`.
