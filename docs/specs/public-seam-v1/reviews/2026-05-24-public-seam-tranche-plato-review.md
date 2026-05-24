# Public Seam V1 Sandbox, Visibility, Stage, And Agent Partial Review

Date: 2026-05-24
Reviewer: Plato (`019e5a42-f123-7540-8648-8084a727c13b`)

Scope:
- `ps1.visibility.data_class_propagation`
- `ps1.stage.payload_receipts`
- `ps1.agent.contract`
- `ps1.sandbox.exec_streaming`

Reviewed tranche:
- Sandbox output file refs must bind safe relative workspace paths, byte counts,
  and SHA-256 values to captured bytes before the seam records file refs.
- Capability-scoped execution denies calls that drop dependency data classes
  before host effects run.
- Dependency data-class collection ignores arbitrary domain payload fields named
  `data_classes`, including domain records with non-seam `kind` discriminators.
- Scorer, judge, and reflective score outputs must declare candidate/artifact
  output classes, and scorer/judge nested blob or trace classes must be covered
  by the enclosing output record.
- Agent session command audit records must carry argv, finite V1 command status,
  receipt binding to the session receipt, and declared-command-policy compliance.

Review method:
- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, matrix fake-pass traps, code, tests, and AGENTS boundary docs.
- The reviewer was explicitly instructed not to treat rerunning the same tests
  as sign-off.

Initial findings:
- Important: scorer and judge stage outputs checked only top-level
  candidate/artifact classes, so nested `blob_ref.data_classes` or
  `trace_refs[].data_classes` could escape monotonic coverage.
- Important: dependency data-class collection recursively treated arbitrary JSON
  fields named `data_classes` as seam authorization metadata.
- Minor: sandbox output-file refs had byte and path negatives, but not a direct
  SHA mismatch negative.
- Minor: `execute_plan_document` remains an unauthenticated representative
  harness route, so capability-before-effect claims must cite
  `execute_plan_document_with_capability`.

Resolution:
- Added scorer and judge nested OutputRecord coverage checks and negatives for
  scorer blob refs, scorer trace refs, and judge trace refs.
- Replaced recursive dependency scanning with a whitelist of locked public-seam
  value/output kinds plus known nested blob/trace/reference fields.
- Added a domain payload fixture whose `kind: application_record` and
  `data_classes: ["external.secret"]` are not interpreted as authorization
  metadata.
- Added a direct sandbox output-file SHA mismatch negative.
- Kept matrix rows pending and recorded only partial evidence.

Follow-up sign-off:
- Critical: none.
- Important: resolved.
- Minor: no new issue found in the final delta.
- The reviewer accepted the tranche as partial pending-row evidence only.

Non-closeout notes:
- No matrix row is promoted by this review.
- This does not prove full runtime/provider execution, ACP delivery, production
  sandbox capture, end-to-end data-class propagation through all runtimes, or
  full agent/sandbox row closeout.
