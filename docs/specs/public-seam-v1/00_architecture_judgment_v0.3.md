# 00 — Architecture Judgment v0.3

The IR is a value, not a call.

A plan is built by the client and evaluated by the engine.

Composition lives in the IR.

The engine sees the whole shape.

The token is the permission spine.

A bearer token is a secret.

A capability document is audit truth.

`RunContext` is the only graph mutation authority.

Evaluator privilege is request-scoped.

Target access is not target egress.

Data-class labels are the security primitive.

Evidence visibility lives in values and receipts, not only in policy.

Reflection is not proposal.

Reflection produces diagnosis.

Proposal produces graph mutation intent.

ACP handles worker lifecycle and transport.

Leaven extension methods on ACP handle all worker callbacks uniformly: graph queries, case reads, workspace operations, LM dispatch, agent runs, sandbox exec, human review, proposal submission, assessment submission.

Leaven handles domain truth, policy, receipts, costs, redactions, and graph mutation.

Adapters own artifact semantics.

Core owns refs, revisions, capabilities, receipts, costs, and projections.

Watch is deferred from v1.

Finite time-travel and diff queries stay in v1.
