# Public API Ledger

Status: active findings recorded.

This file records public exports by crate and whether each export is a real
contract, private support leaked through `pub`, or scaffolding presented as
library capability.

## Ledger

### Umbrella `leaven`

- status: too broad for ordinary users
- evidence: `crates/leaven/src/lib.rs:44`,
  `crates/leaven/src/prelude.rs:3-23`, `crates/leaven/Cargo.toml:39`
- issue: the umbrella exposes derivation and engine-author imports through
  default-facing paths. Derives currently expand to compile errors, and the
  ordinary prelude teaches `RunContext`, `RunGraphView`, `TrustPolicy`, and
  stage traits as common imports.
- correction direction: split ordinary and advanced import surfaces. Remove
  default derive exposure until derives are implemented.

### `leaven-run`

- status: real shell, insufficient product contract
- evidence: `crates/leaven-run/src/builder.rs:28-29`,
  `crates/leaven-run/src/evidence.rs:3-54`,
  `crates/leaven-run/src/evaluator.rs:61-116`
- issue: public builder has the desired broad shape but the runner/scorer,
  score context, score output, cache policy, and dataset identity are too thin
  for real LM/agent optimizer use.
- correction direction: hard-cut to async runner/scorer traits, rich score and
  trace feedback, stable case identity, and runtime/cache configuration.

### `leaven-gepa`

- status: mixed real loop plus fake reflection
- evidence: `crates/leaven-gepa/src/optimizer.rs:180-430`,
  `crates/leaven-gepa/src/proposer.rs:21-54`
- issue: there is a real candidate/evaluate/accept loop, but the reflection
  surface is a fixed-edit fixture, strategy slots are underexposed, and
  feedback/trace context is not available to proposal.
- correction direction: reserve production GEPA names for real reflection and
  expose actual swappable slots through the builder.

### `leaven-lm-cache`

- status: useful implementation layer, bad ordinary story if exposed directly
- evidence: `crates/leaven-lm-cache/src/lib.rs:9`,
  `crates/leaven-lm-cache/src/cached.rs:6-16`
- issue: `CachedLm<M, C>` is a wrapper type that power users may need, but
  ordinary users should configure runtime/cache policy rather than manually
  stacking wrappers.
- correction direction: keep cache traits/backends public for advanced users;
  add higher-level runtime role configuration for solver/reflector cache policy.

### `leaven-agent`

- status: public fake runtime should be scoped deliberately
- evidence: `crates/leaven-agent/src/lib.rs:10`,
  `crates/leaven-agent/src/fake.rs:1-155`
- issue: a public fake runtime may be valid for tests/examples, but the import
  surface should make that status explicit so it does not look like provider
  behavior.
- correction direction: keep fake support in test/example modules or a clearly
  named fake feature path.
