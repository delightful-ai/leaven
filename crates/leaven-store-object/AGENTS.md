## Boundary
This crate is the future object-store backend for `leaven-store` capabilities.

Current public names are scaffolding. `ObjectStore` does not yet prove key
layout, consistency, retries, auth, streaming, or reopen behavior.

## Local Bait
- Do not encode engine graph checkpoint schema here. Backend layout stores
  records through `leaven-store` capabilities; graph semantics stay in
  `leaven-engine`.
- Network/cloud credentials must never be required by default tests.

## Verification
- `cargo check -p leaven-store-object` proves only scaffold exports.
- Real behavior needs deterministic fake-object-store tests for key layout and
  retry/error mapping, plus opt-in live cloud tests if a real service path is
  added.
