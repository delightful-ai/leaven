# Issue Catalog Resolution v0.3

## common.schema.json

Added `Score.output` as required.

Typed fingerprint aliases now use typed prefixes.

Pinned `JsonPath` and `JsonPointer` dialects.

Added `EvaluationAttemptRef`.

Added `MetadataBag` maxProperties and normative byte/depth limit language.

## capability.v1

Added aggregate budgets.

Removed singular/plural purpose ambiguity by using `purposes` in constraints.

Defined revocation policy shape.

Opened audience to registered Leaven strings.

Separated partitions from visibility classes.

Required stage_call_id on evaluation subjects.

Added forbidden input classes generally.

Added jti.

Added subject fingerprint.

Added mint-time validation list and prose.

## plan.v1

Pinned template, JSONPath, and field-path dialects.

Made RequestEvaluationWrite typed.

Typed AgentToolPolicy.

Typed LM sampling, tools, tool_call_id, and output restrictions.

Flattened graph query expression.

Disambiguated graph source kind names.

Added field-list projections.

Added pagination.

Added descendants.

Added predicate operators: contains, matches, exists, is_null.

Added compositional evaluation sets.

Marked explicit case/tag/recent sets as requiring partition resolution.

Added per-read workspace expected data classes.

Added git_status.

Pinned digest algorithm.

Added sandbox streaming policy.

Documented proposal batch sequence semantics.

Added workspace release call.

Added assessment_exists precondition.

Added ChangeFromAgentSession.

## plan_result.v1

Moved replayability to values and per-assessment results, with plan-level roll-up.

Typed graph_set row shapes.

Split case/workspace value variants.

Added workspace_handle value.

Typed LM message through LmMessage reference.

Typed stdout/stderr/files as BlobRef.

Typed call_kind and write_kind enums.

Added call result_hash.

Linked write receipt preconditions to Precondition.

Added started_at and completed_at.

Added receipt-level error fields.

Closed PlanError codes through common ErrorCode.

Plan errors can reference receipts.

Made final_revision required.

## evidence_envelope.v1

Score.output is canonical on Score.

EvidenceEnvelope keeps public/private evidence and visibility policy.

Closed public channel shape.

Moved prompt/transcript logging out of evidence envelope.

Split source_receipts into read/effect/write.

Require data_classes when target_derived is true.

Added producer identity fields.

Added private payload schema fingerprint.

## evaluation_job.v1

Added evaluator_id and evaluator_fingerprint.

Added stage_call_id.

Removed mixed granularity.

Added deadline_at.

Added parent_job_id.

Added target_egress_policy_ref.

Added case_count and cursor support for large sets.

## stage_payloads.v1

Added ProposeRequestV1.

Added ReflectionResultV1.

Mapped proposer role.

Canonicalized judge role.

Typed preference as output shape, not role.

Added adapter payload schema fingerprints.

Bound ReflectRequest to target_safe_projection.

Bound attempt_index.

## watch.v1

Deferred to v1.1.

v1 uses pull-based finite diffs.

## worker_protocol.v1

Deprecated.

Replaced by the Leaven ACP Profile. (MCP-over-ACP was considered during v0.3 drafting and dropped from v1; all worker callbacks ride ACP extension methods uniformly.)
