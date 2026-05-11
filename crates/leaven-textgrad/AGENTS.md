## Boundary
This crate is the future TextGrad optimizer home: text-gradient feedback,
aggregation, updating, and loop configuration.

Current public names are scaffolding. Do not cite them as proof that Leaven has
LM-backed textual gradients or evidence-aware update behavior.

## Local Bait
- Provider-specific LM payloads belong in `leaven-lm-*`; this crate consumes
  provider-neutral LM or agent capabilities only after the slot contract is
  real.
- Shared feedback/evidence containers belong in `leaven-evidence`; TextGrad
  owns strategy interpretation, not the universal evidence model.

## Verification
- `cargo check -p leaven-textgrad` proves only scaffold exports.
- Real behavior needs deterministic updater/feedback tests and a caller proof
  that selected evidence reaches the updater without leaking validation/test
  feedback.
