# Layer 1 Examples And End-To-End Proof

Status: active findings recorded.

This file audits examples as product proof. An example is a finding when it
claims or implies end-to-end optimizer behavior while bypassing the Leaven
surface that should provide that behavior.

## Findings

### L1-008: The AIME example proves a fixed edit, not GEPA reflection

- severity: blocker
- evidence: `examples/p8_aime_gepa/src/main.rs:75-99`,
  `examples/p8_aime_gepa/src/main.rs:398-442`,
  `crates/leaven-gepa/src/proposer.rs:21-47`,
  `docs/specs/gepa_optimizer_surface.md:322-357`
- promised behavior: the high-level AIME example should demonstrate GEPA
  consuming traces/feedback and producing a reflected mutation through the
  public surface.
- actual behavior: the example wires
  `ReflectiveMutation::new(AimePromptEdit::ReplaceSystem(...))`, a deterministic
  fixed edit. The acceptance test proves score movement from a pre-authored
  replacement prompt.
- why it matters: the example can show numbers going up without proving the
  optimizer can actually reflect, propose, or learn from evidence.
- correction direction: rename/move the fixed fixture out of production-looking
  public GEPA API. The AIME proof must use the same solver and reflector
  surfaces that users would use for real optimizer runs.

### L1-009: Live AIME bypasses the Leaven LM surface

- severity: high
- evidence: `examples/p8_aime_gepa/src/main.rs:271-301`,
  `examples/p8_aime_gepa/scripts/openai_solver.py:37-45`,
  `examples/p8_aime_gepa/Cargo.toml:12`,
  `docs/specs/lm_runtime_and_response_cache.md:21-31`
- promised behavior: swapping from mock to live LM should exercise
  provider-neutral `Lm`, provider adapters, cache policy, and cost accounting.
- actual behavior: live solver calls shell out to a Python script using the
  OpenAI API directly. The example does not depend on `leaven-lm-openai` or
  `leaven-lm-cache`.
- why it matters: live AIME does not prove Leaven's LM trait, OpenAI adapter,
  response cache, retry/error mapping, or budget wiring.
- correction direction: route both solver and reflector through Leaven-owned LM
  runtime/capability APIs. The Python script can be deleted once the Rust path
  exists.

### L1-010: Proxy examples are covered as if they were product proof

- severity: medium
- evidence: `scripts/coverage-gate.py:13-26`,
  `examples/p8_aime_gepa/README.md:33`,
  `examples/p8_aime_gepa/src/main.rs:271`
- promised behavior: examples used as acceptance proof should exercise the
  product surface they claim to prove.
- actual behavior: deterministic or bypass examples can contribute to coverage
  while the README admits they are not live AIME optimizer proof.
- why it matters: coverage can ratify proxy behavior and make the repo look
  healthier than the actual end-user surface.
- correction direction: split proxy/demo coverage from product capability
  gates. Mark fake examples as demos in automation until they exercise the
  real surfaces.
