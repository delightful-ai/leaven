# Public Seam V1 Workspace Query Op-Specific Partial Review

Scope: `ps1.workspace.handles_lifecycle` partial workspace-query validation
tranche in `crates/leaven-public-seam`.

Reviewer: Hypatia (`019e586c-3072-7281-9726-86ba57edf77f`)

Review mode: read-only adversarial semantic inspection. The reviewer was
explicitly instructed not to treat rerunning the same tests as sign-off.

Reviewed sources:

- `docs/specs/public-seam-v1/manifest.json`
- `docs/specs/public-seam-v1/01_plan_ir_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.plan.v1.schema.json`
- `docs/specs/public-seam-v1/schemas/leaven.plan_result.v1.schema.json`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `crates/leaven-public-seam/src/plan_execution/queries.rs`
- `crates/leaven-public-seam/tests/plan_document.rs`
- `crates/leaven-public-seam/AGENTS.md`

## Findings And Resolution

1. Digest results are not bound to the requested path. The locked Plan IR
   schema requires `digest.path`, but the locked Plan Result
   `workspace_snapshot` value has only `workspace` and `digest`. Resolution:
   do not invent a result path echo or change locked schema semantics. The seam
   now rejects unsafe digest request paths and still validates digest algorithm
   and workspace id, but digest path-level backend truth remains pending and is
   called out in `crates/leaven-public-seam/AGENTS.md`.

2. `git_log`, `git_diff`, and `git_status` share the broad
   `workspace_diff` result family. The locked result schema has no op
   discriminator, workspace echo, `against`, `porcelain`, or parsed log-entry
   structure. Resolution: do not overclaim op-specific result preimage binding
   for these fields. The seam requires `text` or `blob_ref` for all three and
   adds the missing `git_log` missing-body negative, while leaving full
   diff-family backend truth pending.

3. Workspace path containment used string prefixes over unconstrained schema
   strings. Resolution: the public seam now rejects absolute paths, home-path
   shorthands, backslash paths, empty segments, `.` segments except root, and
   `..` traversal before host results are accepted or containment checks run.

## Sign-Off

The tranche is acceptable as partial pending-row evidence for seam-level denial
of schema-valid workspace-query substitutions. It does not close
`ps1.workspace.handles_lifecycle`, does not prove concrete workspace backend
execution, and must remain `pending`.
