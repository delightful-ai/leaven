# AGENTS Honesty Audit Follow-up

Status: follow-up after checkpoint `vkukuvvk` and latest `main` fetch.

`jj git fetch` reported no newer `main`; the saturated AGENTS hierarchy was
already based on `main` commit `34615589`.

## Audit Docs Re-read

- `reviews/2026-05-11-fuckery-extermination-today/refinement/public-maturity-gates.md`
- `reviews/2026-05-11-fuckery-extermination-today/refinement/implementation-sequence.md`
- `reviews/2026-05-11-fuckery-extermination-today/refinement/open-design-questions.md`
- `reviews/2026-05-11-fuckery-extermination-today/refinement/surface-requirements.md`
- `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/surface-requirements.md`
- `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/topology-and-crate-graph.md`
- Layer 1 and Layer 2 review docs surfaced through `rg` for blocker/gap/proof
  language.

## What Was Still Fucky

The hierarchy had encoded the headline warning that topology is not maturity,
but it had not yet made that warning operational enough to prevent repeat
mistakes.

Main gaps:

- Public maturity must be classified by **route**, not just by symbol. A type can
  be fine in an advanced crate path and still be a lie in ordinary prelude,
  default features, product-proof examples, or coverage claims.
- The hierarchy did not yet require a generated or manual export-route ledger
  when touching `leaven` defaults, preludes, facade features, examples, or
  coverage gates.
- `leaven-run` guidance called out sync/scalar implementation state, but needed
  sharper warnings for single-task/no-dataset mode, score-vs-assessment
  lowering, absent/failed evidence, and domain environment ownership.
- The propagation failure mode is not only "bad AGENTS wording"; it is default
  facades, feature names, examples, and coverage gates silently promoting
  scaffold names into product truth.

## Encoded Now

- Root `AGENTS.md` now requires route-based public maturity classification for
  facades, features, preludes, examples, and coverage gates.
- Root decision cards now require maturity classification and a proof/export
  gate when changing default features, preludes, facade re-exports, or example
  proof status.
- `crates/leaven/AGENTS.md` now makes manual route classification mandatory
  until a generated export-route ledger exists.
- `crates/leaven/AGENTS.md` now treats optional features as public promises,
  not as implicit scaffold markers.
- `crates/leaven-run/AGENTS.md` now records the Layer 1 traps around score
  facade lowering, absent/failed evidence, single-task/no-dataset mode, and
  domain environment ownership.

## How To Keep It Honest

Near-term manual rule:

- Any change to `crates/leaven/src/lib.rs`, `crates/leaven/src/prelude.rs`,
  `crates/leaven/Cargo.toml` features, `crates/leaven-std/src/lib.rs`, milestone
  examples, or `scripts/coverage-gate.py` must name the route-level maturity
  class in the nearest `AGENTS.md` or in the same review note.

Needed hardening:

- Add a generated export-route ledger from `cargo metadata`, crate-root
  `pub mod` / `pub use`, `leaven` and `leaven-std` facades, prelude exports,
  feature-gated exports, and example imports.
- Join generated rows against reviewed classifications:
  ordinary public contract, advanced public contract, test-support public,
  explicit scaffold feature, private fixture.
- Fail the gate on unknown rows, default-facing scaffold, compile-error derive
  exposure, placeholder provider/backend features, whole-crate prelude globs
  with mixed maturity, and product-proof examples importing proxy fixtures.
- Keep topology tests for dependency direction, but add maturity tests beside
  them instead of overloading topology with product claims.

## Propagation Breakers

- Coverage is execution evidence only. It must never promote P8 fixed-edit
  score movement, live Python shell-outs, fake runtimes, or placeholder crates
  into product proof.
- Optional features are import promises. A feature name like `lm-anthropic`,
  `workspace-docker`, or `store-sqlite` must either expose a behavior-bearing
  adapter with tests or be renamed/hidden as explicit scaffold.
- Default `leaven` imports must remain the strictest route. Anything immature
  there is a product bug even if it is acceptable in an advanced crate path.
- Single-task/no-dataset work must not be faked as singleton train data.
- Score remains Layer 1 vocabulary, but the durable record must preserve
  assessment/evidence/preference truth, including failures and absent scores.
