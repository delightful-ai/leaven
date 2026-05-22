# 02 — Capability Spec v0.3

A capability document is structured authority.

A bearer token is only a handle to authority.

The document contains `jti`.

The document contains `subject_fingerprint`.

The document contains aggregate budgets.

Aggregate budgets cap the sum across grants.

Grant budgets cap individual action classes.

The lower effective limit wins.

Audience strings are open registered Leaven strings.

Grant actions are path strings.

Extension actions are namespaced.

Data-class deny lists override allow lists.

`case.target` read access does not imply `case.target` egress.

`stage_call_id` is required for stage-call subjects.

`evaluation_stage_call` subjects require both `stage_call_id` and `evaluation_request_id`.

Revocation mode is explicit.

Renewal mode is explicit.

Token binding mode is explicit.

Expiry behavior is explicit.

Default expiry behavior is `drain_inflight_no_new_ops`.

A call started before expiry may finish only when expiry behavior permits drain.

No new plan/call/write may start after expiry without renewal.

A graph write after expiry is a new operation.

Workers should renew before long calls.

Mint-time validation enforces role/purpose invariants that JSON Schema cannot express.

A runner cannot receive target fields.

A reflector cannot receive target egress grants under normal GEPA policy.

An evaluator cannot submit assessments outside its evaluation request.
