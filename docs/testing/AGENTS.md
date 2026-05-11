## Boundary
This subtree owns the test contract: proof shape, suite layout, runtime SLA, coverage ratchet, and canonical commands.

Tests in this repo are design constraints. They should kill plausible wrong implementations at the lowest clean layer, not merely exercise code paths.

## Local Rules
- Every new test should name exactly one claim and fit one shape: law, example, scenario, or regression.
- Prefer lower-layer crate tests for algebra, errors, cache keys, path laws, and trait contracts. Use examples only when the claim is a public milestone workflow.
- Milestone examples are workspace packages under `examples/p*/`. `cargo check --workspace --examples` is not a proof command for them.
- `just milestone-p8` is currently a P8 public-builder mechanics proof around fixed-edit reflection. Do not cite it as product proof of GEPA reflection, provider-neutral LM/cache roles, or live AIME improvement until the example is hard-cut to those real paths.
- `just milestone-examples` currently includes the live-gated P5 recipe from the root `Justfile`; do not treat it as a cheap default smoke unless that recipe is made deterministic by default.
- `scripts/coverage-gate.py` runs milestone packages under coverage, but that is execution coverage, not product proof. It runs P5 without the `--live-codex` gate and P8 through the deterministic fixed-edit path.
- Coverage proves executed code stayed covered. It does not answer public maturity; classify examples as product-proof, mechanics-smoke, or proxy-demo before using them as release evidence.
- Keep `just test` under the `<30s` SLA by reducing fixture/setup cost instead of adding a slow lane.
- Coverage floors in the root `Justfile` are ratchets. Raise them when coverage improves; do not lower them to land weaker work.

## Proof Classification
- `product-proof`: the real public contract at the intended user layer, with no substitute implementation for the claimed behavior.
- `mechanics-smoke`: wiring, split handling, reporting, topology, or deterministic loop mechanics over a local fixture.
- `proxy-demo`: a desired flow that knowingly bypasses a missing Leaven surface, such as a provider shell-out or fixed reflective edit.

Use the weakest applicable classification in release notes and closeouts. A run can be valuable and still be a mechanics-smoke.

## Stale Proof Traps
- `cargo check --workspace --examples` misses all milestone packages.
- `just milestone-examples` is not cheap while P5 is live-gated in the root `Justfile`.
- `just coverage` does not tell whether a milestone is product-proof.
- `cargo run -p p8_aime_gepa` does not prove live AIME, provider-neutral LM/cache, or evidence-aware reflection.
- `cargo run -p xtask` currently proves only that the empty automation package builds and exits.

## Verification
- Full suite SLA: `just test`.
- Single nextest selector: `just test-one <selector>`.
- Repeated flake probe: `just test-stress 20 <selector>`.
- Coverage gate: `just coverage`.
- Completion gate: `just check`.
