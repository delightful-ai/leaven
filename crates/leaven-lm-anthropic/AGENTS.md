## Boundary
This crate is the future Anthropic provider adapter for the provider-neutral
`leaven-lm` request/response contract.

Current public names are scaffolding. `AnthropicLm` and `AnthropicConfig` do
not yet prove wire lowering, model defaults, continuation semantics, retries,
or usage/cost mapping.

## Local Bait
- Do not copy OpenAI continuation or prompt-cache semantics here. Anthropic
  wire details must lower into neutral `LmRequest` / `LmResponse` without
  changing the neutral trait to fit one provider.
- Do not add live Anthropic calls as the default proof. Mapping behavior should
  be fixture-backed and deterministic first.

## Verification
- `cargo check -p leaven-lm-anthropic` proves only scaffold exports.
- Real provider work needs local request/response/error mapping tests and
  `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
