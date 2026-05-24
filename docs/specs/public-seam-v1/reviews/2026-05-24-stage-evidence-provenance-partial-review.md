# Public Seam V1 Stage And Evidence Provenance Partial Review

Scope: partial semantic evidence for stage source-ref provenance and evidence
target-derivation visibility in `crates/leaven-public-seam`.

Reviewer: Schrodinger (`019e5897-646b-7032-ac98-99b37b518778`)

Review mode: read-only adversarial semantic inspection. The reviewer was
explicitly instructed not to treat rerunning the same tests as sign-off.

Reviewed sources:

- `docs/specs/public-seam-v1/manifest.json`
- `docs/specs/public-seam-v1/00_architecture_judgment_v0.3.md`
- `docs/specs/public-seam-v1/04_stage_payloads_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/common.schema.json`
- `docs/specs/public-seam-v1/schemas/leaven.stage_payloads.v1.schema.json`
- `docs/specs/public-seam-v1/schemas/leaven.evidence_envelope.v1.schema.json`
- `docs/specs/public-seam-v1/notes/CONFORMANCE_TESTS_v0.3.md`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `crates/leaven-public-seam/src/stage_payload.rs`
- `crates/leaven-public-seam/src/evidence.rs`
- `crates/leaven-public-seam/tests/stage_payloads.rs`
- `crates/leaven-public-seam/tests/evidence_envelope.rs`
- `crates/leaven-public-seam/tests/plan_result_evidence.rs`

## Findings And Resolution

1. Initial review found that the first source-ref coverage implementation
   narrowed locked `InfoRef` values to strings, rejecting schema-valid object
   refs. Resolution: source-ref coverage now compares JCS hashes of the full
   JSON values, and object-form candidate refs are accepted through reflector,
   reflection result, and proposer payloads.

2. Initial review found incomplete negative coverage for hidden target
   derivation. Resolution: evidence-envelope negatives now cover top-level,
   public, private, public trace-ref, and top-level trace-ref `case.target`
   classes with `target_derived=false`; Plan Result evidence also rejects the
   same hidden target-derivation flag when nested in result values.

3. Initial review found AGENTS wording that could imply nested diagnosis source
   refs remained optional. Resolution: the local contract now states that nested
   diagnosis source refs are required.

4. Follow-up review found a non-blocking matrix hygiene issue: object-form
   source-ref preservation had been listed as data-class propagation evidence.
   Resolution: that evidence was moved out of the data-class row and remains
   only on stage/provenance rows.

## Sign-Off

Final follow-up review reported no blocking findings. The tranche is acceptable
as partial pending-row evidence for public-seam validation of stage source-ref
provenance and evidence target-derivation visibility.

This does not prove runtime stage lowering, ACP delivery, provider calls,
proposal graph mutation, or full row closeout. The affected rows remain
`pending`.
