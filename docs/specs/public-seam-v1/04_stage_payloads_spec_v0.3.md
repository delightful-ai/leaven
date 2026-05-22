# 04 — Stage Payloads Spec v0.3

Stage payloads are role-specific.

Role boundaries are research boundaries.

Reflection and proposal are separate.

`ReflectRequestV1` contains target-safe examples, parent candidate, part information, source refs, and query policy fingerprint.

`ReflectionResultV1` contains summary, failure modes, surface suggestions, constraints, source refs, read receipts, data classes, and confidence.

`ProposeRequestV1` contains parent candidate, reflection result, allowed effects, allowed schemas, source refs, and query policy fingerprint.

A proposer may be LM-backed, agent-backed, deterministic, or hybrid.

A reflector may be LM-backed, agent-backed, deterministic, or hybrid.

They are independently swappable.

`judge` is the canonical role.

`preference` is an output shape, not a stage role.

Runner requests are target-free.

Score contexts may be target-aware only under policy.

Adapter requests require payload schema fingerprints.
