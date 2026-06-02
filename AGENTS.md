## Boundary
This is the repo-wide operating contract for Leaven. It gives horizon-level guidance for the Rust workspace; more specific `AGENTS.md` files add local deltas and must not silently contradict this file.

Leaven is spec-first library work. The governing product truth starts in `docs/specs/initial_library.md`; narrower implementation slices live beside it when their status lines mark them current. Superseded specs are provenance only.

Product constraints and principles live in `docs/specs/guiding_principles.md`.

NEVER: add compatibility shims, parallel old/new paths, public test holes, or ornamental docs that do not change decisions. Use hard cutovers unless the user explicitly asks otherwise.

## Evidence Ladder
Use the strongest current source for the kind of decision you are making:

- Product intent and public semantics: `docs/specs`, then contract tests and crate docs that deliberately encode the spec.
- Current workspace truth: `Cargo.toml`, crate manifests, live code, and `crates/leaven/tests/topology_contract.rs`; these outrank stale inventory prose.
- Design pressure: `docs/philosophy` and repo-local Leaven skills. They shape choices, but operational rules must land in code, tests, specs, or the nearest `AGENTS.md`.
- Audit findings: `reviews/**`. They are evidence and prioritization input, especially for public lies and maturity gaps; promote durable lessons into owning surfaces instead of treating the audit tree as doctrine.
- Dated execution notes: `docs/plans/**`. Use them for provenance and handoff context, not as product law.

Topology proof is necessary but not sufficient. A crate can be in the right
layer and still expose an immature public name, proxy example, placeholder
feature, or misleading default import. When that happens, fix the public
surface or encode the warning at the owning layer.

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
- `crates/leaven-public-seam`: locked V1 public seam wire-contract owner for external-language workers. This is the API/wire contract the Python SDK and future language SDKs expose and drive: stage dispatch, capability-scoped effects, result receipts, and validation envelopes. It may depend on reusable vocabulary such as `leaven-evidence`, `leaven-lm`, `leaven-agent`, and `leaven-workspace` to project public wire records into owning neutral primitives, but it must not become a runtime, graph mutation layer, provider adapter, process implementation, upstream ACP adapter, or schema-codegen substitute.
- `crates/leaven-seam-runtime`: transport-neutral runtime dispatcher for the locked V1 public seam. It validates incoming Leaven JSON-RPC requests through `leaven-public-seam`, routes every locked `leaven/*` method to an injected service, validates successful service responses before serialization, and maps refusal to JSON-RPC errors. It must not own stdio/HTTP adapters, process spawning, provider execution, graph mutation, optimizer strategy, or schema-codegen proof.
- `crates/leaven-seam-stdio`: line-delimited stdio adapter for `leaven-seam-runtime`. It owns inherited/stdin-stdout reader-writer serving, per-line parse errors, and one-response-per-request transport mechanics. It must not own method semantics, public schemas, service execution, provider adapters, graph mutation, or SDK demo plans.
- `crates/leaven-eval-parquet`: Parquet file adapter for lowering physical rows into `leaven-eval` source-row manifests. It may know Arrow/Parquet format facts, but it must not own split policy, benchmark provenance, paper-specific schemas, scorer/judge behavior, or evaluator execution.
- `crates/leaven-artifact-*`, `crates/leaven-evidence`, `crates/leaven-preference`, `crates/leaven-population`: standard vocabulary and reusable implementations at their respective knowledge boundaries. Placeholder catch-all artifact and render crates were removed; reintroduce those names only with behavior-bearing contracts, tests, topology rows, and local `AGENTS.md`.
- `crates/leaven-std`: shallow curated facade over standard pieces, not an implementation bucket.
- `crates/leaven-lm`: provider-neutral LM request/response vocabulary and `Lm` trait only. `crates/leaven-lm-cache` owns the reusable Leaven response cache wrapper/backends. `crates/leaven-lm-*` provider crates own concrete provider lowering. These stay outside cold and engine crates.
- `crates/leaven-agent*`, `crates/leaven-agentic`, `crates/leaven-agentic-*`: generic agentic stage adapters and shape-specific agentic adapter helpers; these stay outside cold and engine crates. `leaven-agent-codex-app-server` owns Codex app-server protocol/connectors, and `leaven-agent-codex-cli` owns the command-line adapter path. Do not put app-server protocol types in `leaven-agent`, `leaven-agentic`, or a convenience facade.
- `crates/leaven-acp`: legacy-named hot bidirectional process/session transport used by current bridge proofs. The durable public server path is `leaven-seam-runtime` plus `leaven-seam-stdio`; do not route new public SDK server behavior through `leaven-acp`. The current V1 transport is Leaven-owned line-delimited JSON-RPC, not upstream Agent Client Protocol conformance and not an upstream ACP SDK dependency. It may depend on `leaven-public-seam` for profile/method/result validation and may start external worker processes over stdin/stdout JSON-RPC. It must not own provider execution, graph mutation, upstream ACP agent interop, MCP bridges, concrete LM/agent/sandbox runtime behavior, or schema-codegen proof.
- `crates/leaven-gepa`: current optimizer home. Future MIPRO, TextGrad, or trace optimizer crates must return as behavior-bearing crates with local tests and topology rows. Optimizer-specific strategy state lives in optimizer crates, not in the engine.
- `crates/leaven-gepa-agentic-git`: advanced bridge crate for the GEPA reflection + Git-program agentic proposer path. It may compose GEPA reflection requests, generic agentic proposer flow, `GitProgramArtifact`, and Git-program materialization/readback; it must not own GEPA search policy, provider protocols, Git identity law, Firkin mechanics, or frontier admission rules. Its deterministic bridge proof makes the crate behavior-bearing, but not an ordinary prelude/default-feature product route.
- `crates/leaven-gepa-agentic-skill`: bridge crate for the GEPA reflection + skill-bank agentic proposer path. It may compose GEPA reflection requests, generic agentic proposer flow, and skill-bank materialization/readback; it must not own GEPA search policy, provider protocols, or skill artifact validation rules.
- Domain/edge adapters such as future CUDA or Rust-side Python crates should be added only when they carry behavior, tests, topology rows, and local ownership docs. Do not add placeholder adapter crates.
- DSRS interop is not a current workspace crate. Reintroduce a DSRS crate only with topology-contract coverage, workspace membership, behavior-bearing tests, and local ownership docs.
- Derive macros are not a current workspace crate. Reintroduce `leaven-derive`
  only with real codegen, UI pass/fail fixtures, trait-contract tests, topology
  coverage, and public route maturity updates.
- `crates/leaven`: umbrella import experience and re-exports only.
- `docs/specs`: durable product and architecture specs. Read the relevant spec before implementing spec-derived behavior.
- `docs/specs/public-seam-v1`: locked public seam specification for external-language workers: plan IR, capability tokens, result receipts, stage payloads, evaluator/evidence envelopes, the Leaven worker profile, and JSON Schemas. The current V1 public seam is Leaven-owned JSON-RPC over stdio; upstream ACP belongs only in an explicit future agent-provider interop slice. Watch is deferred from v1.
- `docs/working-memory`: active goal ledgers and continuation notes for long-running Leaven work. These files are durable working memory, not product law; use them to resume investigations, then verify against specs/code/tests before implementing or claiming completion.
- `docs/testing/README.md`: test contract, suite layout, coverage ratchet, and runtime SLA.
- `docs/philosophy`: design pressure and agent skills, not implementation status or subsystem plans.
- `examples`: executable milestone packages. They prove specific public workflows, mechanics, or proxy demos as classified by `examples/AGENTS.md`; do not treat coverage over an example as product maturity by default.
- `reviews`: dated audit findings and critique packages. Treat them as evidence and prioritization input, not as stronger product law than specs/code/tests. When an audit exposes a public lie, fix the owning public surface or encode the warning in the nearest `AGENTS.md`; do not bury it in plans.
- `scripts`: repo tooling with real local side effects; scripts must be deterministic and safe to rerun.
- `xtask`: compiled repo automation package. Its current behavior-bearing
  command is `git-trust-bench`; `xtask/AGENTS.md` owns the local side-effect
  and proof limits for repo automation.

## How To Use This Hierarchy
Read stacked `AGENTS.md` files from root to the working directory. Treat higher
files as defaults and lower files as refinements or exceptions; sibling files
are not in scope unless a stacked file explicitly routes you there.

Before editing, run the quick blind checks:
- placement: can the stacked context name the owning crate or docs subtree?
- refusal: does it name the neighboring crate/path that must not own the work?
- imitation: does it point at a canonical proof anchor or example worth copying?
- verification: does it say what the local command proves, not just the command?
- exception: does it mark stale specs, old plans, live-provider paths, or local
  bait that would otherwise look authoritative?

Default `rg` searches skip vendored Python reference repos and archived
public-seam drafts via `.ignore`. Search those trees by explicit path or with
`rg -u` when provenance is the task.

## Global Invariants
- Before planning code placement or implementation shape, apply topology discipline: identify which crate/module is allowed to know each fact, which boundary should refuse the dependency, and which public surface is the durable contract.
- Public maturity is a separate gate from topology. Default-facing exports,
  examples, README claims, and coverage evidence must say whether they prove
  real product behavior, mechanics, or a proxy fixture.
- Public names must be classified by route, not just by symbol. The same type
  can be an advanced public contract in its owning crate and still be forbidden
  from ordinary prelude/default-feature routes. When touching facades, features,
  preludes, examples, or coverage gates, classify each route as ordinary public
  contract, advanced public contract, test-support public, explicit scaffold, or
  private fixture before treating it as proof.
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
  verify: run the owning crate/test target plus any topology or public-surface proof the change affects; reserve `just release-check` for broad/shared/release gates

- when: adding or changing `leaven-core` public API
  do: add or update the contract test that proves the public promise at the lowest clean layer
  preserve: cold-core dependency boundaries and `RunContext` as graph mutation authority
  avoid: making fields/functions public only for tests or convenience
  verify: run `cargo test -p leaven-core` plus the nearest downstream contract touched by the API; use `just release-check` when the change intentionally exercises workspace-wide compatibility

- when: adding a test
  do: name the claim and choose exactly one shape: law, example, scenario, or regression
  preserve: the `<30s` full-suite runtime target, hard completion timeout, and coverage ratchet
  avoid: ceremony tests, broad e2e assertions for facts expressible lower down, and hidden slow lanes
  verify: run the exact new or changed test first; run `just test` when the suite harness, runtime SLA, or broad cross-crate behavior changed

- when: changing crate boundaries or dependencies
  do: run a topology check before editing, then update the crate-boundary contract and nearest ownership docs in the same change
  preserve: cold core below engine/std/workspace/derive and adapter/runtime dependencies outside cold crates
  avoid: dependency shortcuts that make future seams harder to see, logic in `lib.rs`, and public exports that exist only to make tests convenient
  verify: run `cargo test -p leaven --test topology_contract` plus the owning crate tests; escalate to `just release-check` only for broad topology churn or release confidence

- when: changing default features, preludes, facade re-exports, or example proof status
  do: update the public-maturity classification for the affected route and add/adjust the export or proof gate in the same change
  preserve: default-facing imports and product-proof examples as behavior-bearing ordinary contracts only
  avoid: letting scaffold crates, compile-error derives, fixed fixtures, fake runtimes, provider shell-outs, or advanced wrappers propagate through convenient facades
  verify: run `cargo test -p leaven --test topology_contract` plus the owning facade/example test; if no generated ledger exists yet, document the manual classification in the nearest `AGENTS.md`

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
- The workspace pins nightly via `rust-toolchain.toml`. The `dev` profile keeps LLVM as the default backend with line-table debuginfo, enables Cargo incremental compilation for local edit/compile/test loops, and uses many codegen units. `.cargo/config.toml` still enables the parallel rustc frontend (`-Zthreads`); if the incremental canary regresses, remove that frontend flag before disabling incremental.
- `just test`: canonical full test suite; builds workspace libtests with nextest, runs those libtests in parallel plus workspace doctests, warns on the `<30s` runtime target, and enforces the hard completion timeout configured in the root `Justfile`.
- `just check`: default developer gate; runs formatting, production line-count lint, clippy, and the default workspace test lane without coverage.
- `just coverage`: explicit line/branch coverage gate. It is intentionally separate from ordinary local checks because coverage uses instrumented Cargo artifacts and can be much slower than the default dev loop.
- `just release-check`: full workspace/release gate; runs `just check` plus `just coverage`. It is intentionally expensive and should not be the default closeout for a narrow crate, docs, or topology slice.
- Default closeout proof is the strongest focused command set for the touched ownership surface: exact integration tests, owning crate tests, targeted clippy/fmt when Rust changed, touched scripts, and `cargo test -p leaven --test topology_contract` when membership, dependencies, facades, or crate boundaries changed.
- Escalate to `just check` when the change touches shared engine/run/core behavior with broad blast radius, workspace test tooling, default features/preludes/facades, or when the user asks for the default gate. Escalate to `just release-check` for coverage tooling/floors, release/PR readiness, or when the user asks for the full release gate.
- When you do not run `just check` or `just release-check`, say so explicitly in the closeout and list the focused commands that were run.
- Child `AGENTS.md` files should add verification deltas tied to local change types, not repeat root commands.

## Hazards / Exceptions
- Nearby code is not automatically precedent. Prefer the contract tests, specs, and decision cards over imitation.
- The current workspace inventory comes from `Cargo.toml` plus `crates/leaven/tests/topology_contract.rs`, not directory presence or historical spec diagrams.
- Philosophy docs are decision filters. If a rule must be operational, encode it in code, tests, specs, or the nearest `AGENTS.md`.
- Audit docs are not ornamental. When they distinguish topology proof from product maturity, preserve that distinction in examples, test docs, public facades, and local routing guidance.
- If code and doctrine disagree, resolve the mismatch in the same change: update code toward doctrine or update doctrine because reality changed.
- Do not create child `AGENTS.md` files unless they add a real local boundary, invariant, hazard, task pattern, routing rule, or verification delta.

## Maintenance
Update the nearest relevant `AGENTS.md` in the same change when you alter a boundary, invariant, canonical pattern, hazard, routing rule, verification flow, or exception status.

Promote rules that become true for siblings. Demote rules that are only true locally. Delete duplicated child guidance when parent guidance is sufficient.
