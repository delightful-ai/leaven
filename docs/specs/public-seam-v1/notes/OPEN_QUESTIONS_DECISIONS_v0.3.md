# Open Questions — v0.3 Decisions

## 4.1 ProposeRequestV1 shape

Decision: reflection and proposal are structurally separate.

`ReflectRequestV1` produces `ReflectionResultV1`.

`ProposeRequestV1` consumes `ReflectionResultV1` and produces proposal write intent.

`ReflectionResultV1` is structured but not overfit: summary, failure modes, surface suggestions, constraints, source refs, read receipts, data classes, confidence.

It contains diagnosis and direction.

It does not contain a candidate change.

The proposer translates diagnosis into typed proposal effects.

This preserves ablations: hold proposer fixed, swap reflector; hold reflector fixed, swap proposer.

## 4.2 Score.output placement

Decision: `Score.output` is required directly on `Score`.

Score is the scored result.

Evidence explains the scored result.

Putting reportable output only in evidence makes omission too easy.

`EvidenceEnvelopeV1.public` may summarize or augment the output, but `Score.output` is canonical.

## 4.3 Schema fingerprint algorithm

Decision: schema fingerprints use RFC 8785 JSON Canonicalization Scheme plus SHA-256.

Prefix is `fp_schema_sha256_`.

Annotation keywords are stripped: `title`, `description`, `default`, `examples`, `deprecated`, `readOnly`, `writeOnly`, `$comment`.

Remote references are forbidden for fingerprinting.

Schemas are fingerprinted after bundle-local `$ref` resolution or with an explicitly declared inline-no-remote-refs descriptor.

Implementations must publish the descriptor used for every schema fingerprint.

## 4.4 Dialects

Decision: all public mini-languages are pinned.

Templates use `leaven.mustache.strict.v1`.

Extraction uses RFC 9535 JSONPath, Leaven subset: root, child, bracket, index, slice, wildcard; no filters, scripts, functions, or implementation-defined extensions.

Field paths and redaction paths use RFC 6901 JSON Pointer.

## 4.5 Aggregate USD budget

Decision: both token-level and grant-level budgets exist.

Token-level aggregate budget is a hard ceiling across all grants.

Grant budgets are per-action ceilings.

The effective limit is whichever denies first.

## 4.6 Token expiry mid-call

Decision: expiry behavior is explicit.

Default is `drain_inflight_no_new_ops`.

A call authorized before expiry may complete if the token policy allows draining.

No new call/write/read may start after expiry without renewal.

Write submission after a long call is a new operation and requires a valid renewed token.

Worker SDKs should renew before long calls when renewal is granted.

## 4.7 Data-class propagation

Decision: propagation is monotonic by default.

Templates union all input labels.

Concatenation unions all input labels.

Extraction inherits input labels unless the extraction is a registered projection with an audited downgrade rule.

Aggregation unions all input labels.

Redaction removes content but not receipt/audit evidence.

Data-class downgrades require a registered projection or scorer/evaluator evidence policy.

Deny labels override allow labels.

## 4.8 Cross-run queries

Decision: addressable now, denied by default.

Refs may contain `run`.

Capabilities may contain `resource.runs` and `resource.run_set` as reserved v1.1 path.

Normal stage tokens cannot delegate cross-run access.

Cross-run writes are denied in v1.

## 4.9 Watch.v1

Decision: defer watch from v1 lock.

Finite diff queries stay in v1 through `consistency.since_revision`.

A real watch spec needs delivery, backpressure, ack, cursor, lifetime, cancellation, and heartbeat semantics.

v0.3 ships only a deferred marker schema.

## 4.10 LM API surface

Decision: v1 core supports text, tools, tool-call IDs, typed sampling, provider hints, final-message output, and JSON-schema output.

Multimodal and streaming LM output are extension or v1.1 concerns.

OpenAI chat shape is not the ontology.

## 4.11 Visibility enum

Decision: one canonical `EvidenceVisibility` enum lives in common.

All projection, reflective example, and evidence envelope policies reference it.

## 4.12 Judge vs preference

Decision: `judge` is the stage role.

`preference` is an output shape.

## 4.13 Query extensibility

Decision: closed core plus registered extension namespace.

Core query kinds are stable.

New research queries use `extension_source` / `ExtensionObject` with schema fingerprint and explicit capability.

A well-known extension may graduate into core only through a schema-versioned release.
