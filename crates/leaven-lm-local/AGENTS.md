## Boundary
This crate is the future local-LM provider adapter for the provider-neutral
`leaven-lm` contract.

Current public names are scaffolding. `LocalLm` and `LocalLmConfig` do not yet
prove local server protocol, model identity, streaming, continuation, or
usage/cost behavior.

## Local Bait
- Keep server-specific URL/auth/transport details here; cache identity and
  provider-neutral request semantics stay in `leaven-lm` and
  `leaven-lm-cache`.
- Local model process management is not workspace management. Do not route
  sandbox or command lifecycle behavior here.

## Verification
- `cargo check -p leaven-lm-local` proves only scaffold exports.
- Real behavior needs fixture-backed protocol tests and explicit opt-in tests
  for any live local server path.
