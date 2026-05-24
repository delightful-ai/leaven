# ps1.visibility.data_class_propagation Review

Reviewer: Bernoulli (`019e5bb9-5872-7cb2-8f54-27258b04a24b`)
Date: 2026-05-24
Verdict: SIGN OFF

## Scope

Reviewed the `ps1.visibility.data_class_propagation` claim against the locked
public-seam V1 specs and current public-seam Rust/package semantic surface.
The review focused on whether data classes propagate monotonically through the
representative execution/projection harness, whether forbidden intersections
deny before host effects, and whether the row's named fake pass is rejected.

## Sign-Off

No blocking findings remain for promoting the row to `proven`.

The negative proof does not require Plan Result `Redaction` wire objects for
call-authority denials: denied calls exit before host effects as structured
`PublicSeamError::CallAuthorityDenied { kind, message, redactions }`, and no
Plan Result envelope is produced on that refusal path.

The propagation proof is credible in the public-seam scope. Plans are schema
validated before authority checks, call shapes require `input_classes`,
dependency classes are collected from binding metadata and typed seam carriers,
and omitted/forbidden classes are denied before host calls. Tests reject the
fake pass across case-read dependencies, literal side channels, nested agent
command blobs, graph assessment rows, workspace listing entries, and incidental
domain JSON.

Plan Result, evidence, and stage-payload coverage is sufficient for this row:
nested score outputs, evidence envelopes, blob refs, trace refs, workspace
listing entries, receipt trace refs, and stage output records are checked
against carrier `data_classes`. The RunContext projection evidence requires
real `CaseDataReadEvidence`, emits query receipt trace classes, and rejects
missing or non-`case.target` receipt trace data classes.

## Residual Limits

This is not ACP/provider runtime sign-off, not live-provider behavior, not
transport-level permission UX, and not a claim that every future engine route
automatically propagates classes without its own row evidence. Query data
classes may ride in typed dependency JSON values rather than the auxiliary
`dependency_data_classes` set; the matrix must not imply that auxiliary set is
the only carrier.
