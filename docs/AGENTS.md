## Boundary
This subtree holds Leaven's written decision record. It is not one kind of document: specs, plans, testing policy, philosophy, and repo-agent guidance have different authority.

Use the highest document that is true, actionable, and current. If a doc is marked superseded, historical, or planning-only, do not implement from it until you reconcile it with the live specs and code.

## Routing
- `specs/`: durable product, topology, and behavior contracts. Start here for spec-derived code.
- `working-memory/`: active goal ledgers and continuation notes. Treat them as stronger than chat history and weaker than specs/code/tests. Update them when a long-running goal gains new evidence, a live run handle, a concrete blocker, or a verified next action.
- `plans/`: implementation work logs and task breakdowns. Treat them as dated execution notes, not governing truth when specs or code have moved.
- `testing/README.md`: canonical proof model, suite layout, coverage ratchet, and runtime SLA.
- `philosophy/`: design pressure and repo-local skills. It shapes decisions but does not replace specs, tests, or crate ownership docs.
- `AGENTSMD_INFO.md`: rubric for writing stackable AGENTS files in this repo; use it when changing this hierarchy.

## Authority Ladder
- Implemented public contracts live in code plus tests. Specs can demand a stronger future, but do not claim the future is implemented until the proof exists.
- `docs/specs/initial_library.md` is the governing product horizon. More specific implementation specs refine it only for their surface, and their `Status:` line controls whether they are current, planning, pre-implementation, or historical.
- Canonical audit docs under `reviews/` are current negative evidence: they can prove a public path is lying, proxying, or under-proven. They do not replace specs as the target design.
- Dated plans explain why work happened. They are useful for context and command archaeology, but specs/code/tests win once reality moves.
- Philosophy and skills pressure decisions. If a philosophical rule becomes operational, promote it into specs, tests, crate docs, or the nearest owning `AGENTS.md`.

## Local Rules
- Keep durable behavior in `specs/`, crate docs, tests, or the nearest owning `AGENTS.md`. Do not bury operational rules in a dated plan.
- Keep active-goal continuity in `working-memory/` when the work is too long for one turn. Do not use a working-memory note as proof by itself; cite the current spec/code/test/report artifact it points to.
- When updating a plan because reality changed, also update the owning spec or code contract if the change is now durable.
- When adding or changing a status line, make the authority explicit: implemented contract, implementation spec, companion spec, planning note, superseded, or historical.
- Do not move philosophy text into specs by quotation. Convert it into concrete crate boundaries, trait laws, error contracts, or test requirements.
- When a review calls out false proof, encode the warning at the proof site too: examples, testing docs, scripts, public facade docs, or local crate `AGENTS.md`. A plan note alone is too far from the failure.
- Do not use `just check`, coverage, or a milestone binary as product-maturity evidence unless the docs for that surface say what kind of proof it is.

## Decision Cards
- when: using a dated plan to drive implementation
  do: read the named spec, inspect current code/tests, and keep only the still-true task shape
  preserve: plan history as history, not retroactive law
  avoid: copying old crate names, command names, or milestone claims without checking current manifests and `Justfile`
  verify: run the narrow command after confirming it still exists

- when: translating an audit finding into durable guidance
  do: put target behavior in specs or tests, and put local warnings in the nearest owning `AGENTS.md`
  preserve: the audit's distinction between public promise, actual path, and proof gap
  avoid: treating an audit priority map as a compatibility plan
  verify: cited code/spec/review paths still resolve

## Verification
- For doc-only changes, check referenced paths and commands still exist.
- For spec changes that alter crate boundaries, also run `cargo test -p leaven --test topology_contract`.
- For behavior specs, run the narrow milestone or crate test named by the spec, then `just check` before claiming implementation complete.
