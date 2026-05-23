# Evidence Visibility Current Blocker Review

Reviewer: Codex active-goal audit

Scope:

- `ps1.evidence.visibility_receipts`
- `ps1.visibility.data_class_propagation`

Decision: both rows remain pending.

Findings:

- Existing validation is useful prerequisite evidence: evidence envelopes preserve public/private data classes and source receipt references, nested Plan Result values must cover evidence and score-output data classes, and evidence source receipts must point at receipts of the expected kind.
- A concrete gap was fixed: `target_derived=true` evidence now requires a `case.target` top-level data class, not only top-level coverage of whatever projection classes were present.
- The broader rows still overclaim runtime behavior. They require monotonic propagation through projections, templates, LM calls, agent runs, writes, receipts, redactions, and evaluator-produced evidence. Current evidence is public-seam validation and fixture-level projection, not full runtime propagation or persisted receipt visibility from evaluator execution.

Current status:

- Keep `ps1.evidence.visibility_receipts` pending.
- Keep `ps1.visibility.data_class_propagation` pending.
- Future promotion needs runtime producer evidence or a deliberate row split separating public-seam validation from runtime propagation.
