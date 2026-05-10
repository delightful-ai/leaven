## Boundary
This is the repo-wide operating contract for Leaven. It gives horizon-level guidance for the Rust workspace; more specific `AGENTS.md` files add local deltas and must not silently contradict this file.

Leaven is spec-first library work. The governing product truth starts in `docs/specs/initial_library.md`; narrower implementation slices may live beside it, such as `docs/specs/first_two_subsystems.md`.

Product constraints and principles live in `docs/specs/guiding_principles.md`.

NEVER: add compatibility shims, parallel old/new paths, public test holes, or ornamental docs that do not change decisions. Use hard cutovers unless the user explicitly asks otherwise.

## Map / Routing
- `crates/leaven-kernel`: universal IDs, content IDs, cost values, durable error records, metadata, fingerprints, and time primitives.
- `crates/leaven-core`: cold optimizer algebra only: artifact/problem/evidence marker, proposal, evaluation, and preference vocabulary. No graph, context, stages, workspace, store, engine, runtime, or surface assumptions.
- `crates/leaven-surface`: explicit edit/projected surfaces over artifacts, including parts, selections, paths, addresses, fingerprints, and surface errors.
- `crates/leaven-store`: storage capability traits only. It may know kernel/core storage vocabulary, but it must not know `RunGraph` or concrete backend crates.
- `crates/leaven-store-*`: concrete storage backends. Backend crates depend on the store capability crate, not on graph/engine internals.
- `crates/leaven-workspace`: workspace/sandbox substrate contracts only. It must not know artifacts, surfaces, stores, or engine graph behavior.
- `crates/leaven-workspace-*`: concrete workspace backends.
- `crates/leaven-engine`: run execution, `RunGraph`, `RunContext`, stage traits, budget ledger, trust/read scopes, cache, callbacks, reports, and events. Graph mutation is private to this crate and exposed through `RunContext`.
- `crates/leaven-run`: public product-builder ergonomics and lowering: `optimize(seed)`, train/validation/test inputs, runner/scorer/evaluator helpers, default evidence-store wiring, and result facades. It composes engine/eval/store without owning optimizer strategy state, domain semantics, concrete providers, or concrete workspaces.
- `crates/leaven-artifacts`, `crates/leaven-artifact-*`, `crates/leaven-evidence`, `crates/leaven-preference`, `crates/leaven-population`, `crates/leaven-render`: standard vocabulary and reusable implementations at their respective knowledge boundaries.
- `crates/leaven-std`: shallow curated facade over standard pieces, not an implementation bucket.
- `crates/leaven-lm`: provider-neutral LM request/response vocabulary and `Lm` trait only. `crates/leaven-lm-cache` owns the reusable Leaven response cache wrapper/backends. `crates/leaven-lm-*` provider crates own concrete provider lowering. These stay outside cold and engine crates.
- `crates/leaven-agent*`, `crates/leaven-agentic`, `crates/leaven-agentic-*`: generic agentic stage adapters and shape-specific agentic adapter helpers; these stay outside cold and engine crates. `leaven-agent-codex` is a Codex provider-family facade; `leaven-agent-codex-app-server` owns Codex app-server protocol/connectors. Do not put app-server protocol types in `leaven-agent`, `leaven-agentic`, or the facade.
- `crates/leaven-gepa`, `crates/leaven-mipro`, `crates/leaven-textgrad`, `crates/leaven-trace`: optimizer implementations. Optimizer-specific strategy state lives here, not in the engine.
- `crates/leaven-dsrs`, `crates/leaven-cuda`, `crates/leaven-python`: domain/edge adapters.
- `crates/leaven-derive`: derive macros only; no runtime or adapter dependencies without an explicit derive contract.
- `crates/leaven`: umbrella import experience and re-exports only.
- `docs/specs`: durable product and architecture specs. Read the relevant spec before implementing spec-derived behavior.
- `docs/testing/README.md`: test contract, suite layout, coverage ratchet, and runtime SLA.
- `docs/philosophy`: design pressure and agent skills, not implementation status or subsystem plans.
- `scripts`: repo tooling with real local side effects; scripts must be deterministic and safe to rerun.

## Global Invariants
- Before planning code placement or implementation shape, apply topology discipline: identify which crate/module is allowed to know each fact, which boundary should refuse the dependency, and which public surface is the durable contract.
- `lib.rs` files are maps only: module declarations, curated re-exports, and optional preludes. Do not put runtime logic, domain logic, helper logic, or test-only behavior in `lib.rs`; name the owning concept and put the code in that module.
- Types, traits, errors, and tests should preserve domain truth instead of smoothing it away.
- Prefer ownership-native Rust APIs over clone-heavy plumbing. Pass ownership when values are consumed, borrow when the caller retains ownership, and clone only at explicit fan-out, persistence, or async/lifetime boundaries where it is clearer than contorting ownership.
- Use `jj` for repository state, diffs, and commit boundaries. Commit coherent progress as work lands, at reasonable task-completion intervals rather than saving everything for final closeout. For multi-step work, finish a coherent slice, verify the narrow gate for that slice, then prefer `jj describe -m "<message>"` followed by `jj new` when the current working-copy commit is ready.
- `RunContext` is the public mutation path into `RunGraph`; do not expose graph internals to satisfy callers or tests.
- Fresh authored artifacts use `ProposalEffect::Create`; changes to existing candidates use `ProposalEffect::Change` with lineage carried by `CausalInputs` / `InfoRef`.
- Cold core crates stay free of adapter, runtime, cloud, database, HTTP, and process dependencies.
- Tests assert public/capability behavior unless the invariant is genuinely private and lives in a crate-local `#[cfg(test)]` module.
- Coverage is a ratchet across lines and branches. Keep overall branch coverage
  at `80%+`; the current enforced floor is `coverage_branch_floor` in the root
  `Justfile`. Raise `coverage_line_floor` and `coverage_branch_floor` when the
  suite improves; do not lower either floor to land weaker work.

## Decision Cards
- when: implementing behavior from a spec
  do: read the governing spec and the narrowest slice doc first, then implement the smallest coherent surface
  preserve: spec vocabulary, durable events/evidence, hard cutover semantics
  avoid: treating philosophy essays or prior plans as stronger than the current spec and code
  verify: run `just check` before claiming completion

- when: adding or changing `leaven-core` public API
  do: add or update the contract test that proves the public promise at the lowest clean layer
  preserve: cold-core dependency boundaries and `RunContext` as graph mutation authority
  avoid: making fields/functions public only for tests or convenience
  verify: run `cargo nextest run -p leaven-core` during iteration, then `just check`

- when: adding a test
  do: name the claim and choose exactly one shape: law, example, scenario, or regression
  preserve: the `<30s` full-suite SLA and the coverage ratchet
  avoid: ceremony tests, broad e2e assertions for facts expressible lower down, and hidden slow lanes
  verify: run `just test`; it enforces the suite SLA

- when: changing crate boundaries or dependencies
  do: run a topology check before editing, then update the crate-boundary contract and nearest ownership docs in the same change
  preserve: cold core below engine/std/workspace/derive and adapter/runtime dependencies outside cold crates
  avoid: dependency shortcuts that make future seams harder to see, logic in `lib.rs`, and public exports that exist only to make tests convenient
  verify: run `cargo test -p leaven --test topology_contract` during iteration, then `just check`

- when: changing scripts or repo tooling
  do: keep the operator path one-command, deterministic, and idempotent where possible
  preserve: explicit side effects and clear failure exits
  avoid: hidden network/cloud/credential assumptions in local defaults
  verify: run the touched script directly plus the smallest command that depends on it

- when: updating documentation
  do: put durable behavior in specs, crate docs, tests, or the nearest owning `AGENTS.md`
  preserve: one truth at the highest layer where it is true, actionable, and stable
  avoid: parking implementation plans, audits, incidents, or release notes in `docs/philosophy`
  verify: check referenced paths and commands still exist

## Design Skills
Repo-local design skills are rollout-scoped context. Read each applicable Leaven design skill at most once per rollout; after it is loaded, rely on it unless the skill file changed.

The skill descriptions own trigger routing. Do not duplicate their full routing table here.

## Verification Policy
- `just test`: canonical full test suite; must finish in `<30s` and includes nextest workspace tests plus doctests.
- `just check`: completion gate; runs formatting, production line-count lint, clippy, SLA-enforced tests, and line/branch coverage.
- Use narrower commands only while iterating. Before claiming behavior is complete, run `just check` unless the user explicitly requested a narrower proof.
- Child `AGENTS.md` files should add verification deltas tied to local change types, not repeat root commands.

## Hazards / Exceptions
- Nearby code is not automatically precedent. Prefer the contract tests, specs, and decision cards over imitation.
- Philosophy docs are decision filters. If a rule must be operational, encode it in code, tests, specs, or the nearest `AGENTS.md`.
- If code and doctrine disagree, resolve the mismatch in the same change: update code toward doctrine or update doctrine because reality changed.
- Do not create child `AGENTS.md` files unless they add a real local boundary, invariant, hazard, task pattern, routing rule, or verification delta.

## Maintenance
Update the nearest relevant `AGENTS.md` in the same change when you alter a boundary, invariant, canonical pattern, hazard, routing rule, verification flow, or exception status.

Promote rules that become true for siblings. Demote rules that are only true locally. Delete duplicated child guidance when parent guidance is sufficient.
