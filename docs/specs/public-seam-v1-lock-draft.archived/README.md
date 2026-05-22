# Leaven Public Seam V1 Lock Draft

This package contains the candidate public seam specification for Leaven.

The main document is [`leaven_public_seam_v1_lock_spec.md`](./leaven_public_seam_v1_lock_spec.md).

The older short draft is retained as [`leaven_public_seam_v1_spec.md`](./leaven_public_seam_v1_spec.md) for comparison.

## Schemas

- [`common.schema.json`](./schemas/common.schema.json)
- [`leaven.plan.v1.schema.json`](./schemas/leaven.plan.v1.schema.json)
- [`leaven.plan_result.v1.schema.json`](./schemas/leaven.plan_result.v1.schema.json)
- [`leaven.capability.v1.schema.json`](./schemas/leaven.capability.v1.schema.json)
- [`leaven.stage_payloads.v1.schema.json`](./schemas/leaven.stage_payloads.v1.schema.json)
- [`leaven.evaluation_job.v1.schema.json`](./schemas/leaven.evaluation_job.v1.schema.json)
- [`leaven.evidence_envelope.v1.schema.json`](./schemas/leaven.evidence_envelope.v1.schema.json)
- [`leaven.watch.v1.schema.json`](./schemas/leaven.watch.v1.schema.json)
- [`leaven.worker_protocol.v1.schema.json`](./schemas/leaven.worker_protocol.v1.schema.json)

## Examples

- [`reflector_plan.example.json`](./examples/reflector_plan.example.json)
- [`evaluator_capability.example.json`](./examples/evaluator_capability.example.json)
- [`evaluator_dspy_codex.py`](./examples/evaluator_dspy_codex.py)

## Reading order

Read the lock spec first.

Read `leaven.plan.v1.schema.json` and `leaven.capability.v1.schema.json` second.

Read the evaluator example third.

Use the examples as executable design intent, not as final SDK syntax.
