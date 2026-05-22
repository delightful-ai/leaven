# Leaven Public Seam V1 — Locked

This package is the locked public seam for Leaven v1.

- `goal-readiness-gate.yaml` is the pre-goal correctness gate. Broad
  implementation goals for this seam must reference this package and code to the
  standards encoded here.
- `schemas/` contains designed JSON Schemas.
- `profiles/` contains the Leaven ACP profile prose.
- `examples/` contains concrete shape examples.
- `notes/` contains review resolution and conformance tests.

V1 incorporates the comprehensive design pass: reflection/proposal structural separation, ACP transport, all worker callbacks as ACP extension methods (no MCP layer in v1), data-class propagation, score output placement, pinned dialects, schema fingerprinting, typed plan results, per-assessment replayability, aggregate budgets, and deferral of watch.v1.

The archived lock draft and any downloaded draft bundles are provenance only.
They are not alternate correctness standards for V1 unless this package and
manifest are deliberately revised in the same change.
