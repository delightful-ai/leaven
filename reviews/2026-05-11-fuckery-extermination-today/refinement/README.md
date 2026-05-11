# Refinement Pass

Status: integrated refinement pass.

This pass compares the first audit against the original Leaven vision instead
of only collecting local implementation smells.

The key question is:

Does the audit make it obvious how current Leaven diverges from the intended
optimizer library shape, and what must change before a user can trust the
public surface?

## Inputs

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/gepa_public_private_surface.md`
- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/eval_lowering_detail.md`
- `docs/specs/lm_runtime_and_response_cache.md`
- the first-pass audit files in this review tree

## Outputs

- `vision-comparison.md`: integrated comparison against the original vision.
- `surface-requirements.md`: refined public/private contract requirements.
- `gepa-slot-contract.md`: GEPA slot-by-slot contract and nomenclature map.
- `public-maturity-gates.md`: public scaffolding and topology maturity gates.
- `implementation-sequence.md`: ordered correction plan.
- `open-design-questions.md`: remaining questions that should not be hidden in
  implementation.
- `agent-reports/`: first-party reports from independent refinement agents.

## Agent Reports

- `agent-reports/layer-1-original-vision.md`
- `agent-reports/gepa-original-vision.md`
- `agent-reports/engine-eval-original-vision.md`
- `agent-reports/crate-graph-original-vision.md`
