# Conformance Tests v0.3

Reject a reflector that reads `case.target`.

Reject a reflector LM call whose input classes include `case.target`.

Reject an evaluator assessment outside its evaluation request.

Reject an evaluator assessment missing `Score.output`.

Reject an evidence envelope with `target_derived=true` and no data classes.

Reject a proposal write that applies proposals from a submit-only token.

Reject a proposal change against an ungranted surface fingerprint.

Reject a call whose data classes intersect forbidden input classes.

Reject a delegated token that widens any action, resource, budget, data class, or schema.

Reject a graph query with unpinned path syntax.

Reject template dialects other than `leaven.mustache.strict.v1`.

Reject extraction dialects outside the Leaven JSONPath subset.

Reject explicit case sets that have not been partition-resolved.

Reject unknown core `kind` values.

Accept `final_revision == base_revision` for read-only plans.

Accept per-assessment mixed replayability and compute plan roll-up summary.

Accept ACP permission denials with typed `PlanError` and redactions.
