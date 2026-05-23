# ps1.plan.pinned_dialects adversarial review

Fresh evidence before final review:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test plan_dialects`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven-public-seam`

Adversarial reviewer:

- Sub-agent `019e5497-1189-72b2-a835-36fcda15ec47`

Initial and follow-up review blockers:

- No sign-off on the first pass. The reviewer found that standalone dialect replay still allowed `validate_plan_document` to treat Plan IR path/template strings as opaque.
- Later passes blocked over- and under-validation in the semantic Plan IR traversal: schema-valid bad JSON Pointer fields were missed in predicate/projection, stratified case-set `by`, typed graph filters, and `schema_valid.value` `ValueExpr` locations.
- Later passes also blocked false positives where arbitrary data slots were treated as Plan IR dialect syntax: extension payloads, literal values, predicate comparison values, `in.values`, LM JSON schemas, metadata, human-review rubrics, proposal causal data, and assessment target/preference/ranking payloads.

Fixes after review:

- Added `PinnedDialectEvaluator` for deterministic RFC 6901 JSON Pointer, Leaven RFC 9535 JSONPath subset, and `leaven.mustache.strict.v1` template replay.
- Added `PlanDocument` semantic dialect traversal that validates known Plan IR dialect-bearing fields through the evaluator and records pinned pointer, JSONPath, and strict template counts.
- Added positive tests that validate a Plan IR document carrying predicate pointers, stratified `by`, JSONPath extraction, and strict template usage through `validate_plan_document`.
- Added negative tests for unpinned pointer syntax, non-subset JSONPath filters/functions/scripts/recursive descent, non-strict template features, invalid predicate pointers, invalid stratified `by`, invalid typed graph-filter pointers, and invalid `schema_valid.value` extraction paths.
- Added regressions proving arbitrary JSON/data slots are not treated as core Plan IR dialect syntax.

Follow-up review result:

- Sign-off granted. The reviewer found no blocking issues after the arbitrary-slot regressions and confirmed that the row can be marked proven after recording implementation, positive test, negative test, and review evidence.

Scope of sign-off:

- `ps1.plan.pinned_dialects` is signed off for semantic-denial proof that the public seam parses/replays the pinned mini-languages and rejects unpinned dialect features at known Plan IR dialect-bearing locations.
- This sign-off does not prove full Plan IR execution, graph query execution, RunContext mutation behavior, authorization, or runtime provider behavior; neighboring Plan IR and runtime rows remain pending.
