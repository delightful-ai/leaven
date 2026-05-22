# 05 — Evaluator Spec v0.3

Evaluators are privileged.

Evaluators are not root.

Evaluator jobs carry request identity, candidate set, case set, base revision, deadline, evaluator ID, and capability fingerprint.

Evaluator jobs do not inline hidden targets by default.

Targets are read through `case_query.load` under evaluator capability.

Evaluator target reads are receipted.

Evaluator target egress is separately granted.

Evaluator workspace reads are data-class labeled.

Evaluator sandbox execs are receipted.

Evaluator agent sessions are receipted.

Evaluator DSPy calls route through Leaven LM adapter by default.

Evaluator writes are limited to `assessment.submit` for the evaluation request.

Every successful score carries `Score.output`.

Every assessment carries `EvidenceEnvelopeV1`.

Every assessment carries per-assessment replayability.

Managed sandbox evaluators must be `boundary_managed` or better.

Package scorers cannot use untracked BYO effects.
