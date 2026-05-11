## Boundary
This crate is the future MIPRO optimizer home: bootstrapping, observations,
surrogate modeling, acquisition, and MIPRO loop configuration.

Current public names are scaffolding. Do not treat `Mipro`, `MiproBuilder`, or
the surrogate/acquisition structs as behavior-bearing optimizer proof.

## Local Bait
- Reusable population or preference state belongs in `leaven-population` and
  `leaven-preference`, not here.
- Engine execution, budget, cache, events, and graph mutation remain in
  `leaven-engine`; MIPRO strategy state composes them.
- Public builder names should not move into `leaven-run` until ordinary-user
  product semantics are specified and tested.

## Verification
- `cargo check -p leaven-mipro` proves only scaffold exports.
- Real MIPRO work needs local optimizer contract tests plus
  `cargo test -p leaven --test topology_contract` when dependencies or facade
  exposure change.
