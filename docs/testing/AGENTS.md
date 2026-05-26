## Boundary
This subtree owns the test contract: proof shape, suite layout, runtime target, coverage ratchet, and canonical commands.

Tests in this repo are design constraints. They should kill plausible wrong implementations at the lowest clean layer, not merely exercise code paths.

## Local Rules
- Every new test should name exactly one claim and fit one shape: law, example, scenario, or regression.
- Prefer lower-layer crate tests for algebra, errors, cache keys, path laws, and trait contracts. Use examples only when the claim is a public milestone workflow.
- Milestone examples are workspace packages under `examples/p*/`, but default `just test`, `just lint`, and `just coverage` exclude them. `cargo check --workspace --examples` is not a proof command for them.
- `just milestone-p8` is a P8 public-builder proof for LM-backed GEPA reflection through provider-neutral `leaven-lm`. Do not cite it as proof of concrete provider transport, LM cache behavior, or live AIME improvement.
- `just milestone-examples` currently includes the live-gated P5 recipe from the root `Justfile`; do not treat it as a cheap default smoke unless that recipe is made deterministic by default.
- `scripts/coverage-gate.py` excludes milestone packages from default coverage. Run milestone recipes explicitly before using example behavior as evidence.
- Coverage proves executed code stayed covered. It does not answer public maturity; classify examples as product-proof, mechanics-smoke, or proxy-demo before using them as release evidence.
- Keep `just test` moving toward the `<30s` target by reducing fixture/setup cost instead of adding a slow lane; the hard timeout only protects completion.
- Coverage floors in the root `Justfile` are ratchets. Raise them when coverage improves; do not lower them to land weaker work.

## Proof Classification
- `product-proof`: the real public contract at the intended user layer, with no substitute implementation for the claimed behavior.
- `mechanics-smoke`: wiring, split handling, reporting, topology, or deterministic loop mechanics over a local fixture.
- `proxy-demo`: a desired flow that knowingly bypasses a missing Leaven surface, such as a provider shell-out or fixed reflective edit.

Use the weakest applicable classification in release notes and closeouts. A run can be valuable and still be a mechanics-smoke.

## Stale Proof Traps
- `cargo check --workspace --examples` misses all milestone packages.
- Default `just test`, `just lint`, and `just coverage` intentionally skip milestone packages.
- `just milestone-examples` is not cheap while P5 is live-gated in the root `Justfile`.
- `just coverage` does not tell whether a milestone is product-proof.
- `cargo run -p p8_aime_gepa` does not prove live AIME, concrete provider transport, or LM cache behavior.
- `cargo run -p xtask` proves only that the automation CLI builds and accepts
  an empty invocation. Use `cargo run -p xtask -- git-trust-bench ...` or the
  coverage recipe before citing the Git trust benchmark behavior.

## Verification
- Full suite runtime warning and hard completion timeout: `just test`.
- Single cargo-test selector: `just test-one <cargo test args>`.
- Repeated flake probe: `just test-stress 20 <cargo test args>`.
- Coverage gate: `just coverage`.
- Completion gate: `just check`.
