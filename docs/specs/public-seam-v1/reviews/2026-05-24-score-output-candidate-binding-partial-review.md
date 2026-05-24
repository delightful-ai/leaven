# Score Output Candidate Binding Partial Review

Reviewer: Arendt (`019e5815-f805-7391-8ece-ee53515c57c7`)

Scope: `ps1.evaluator.score_output` after the candidate-bound Plan IR and explicit artifact-output runtime tranche.

Decision: useful partial evidence; row remains pending.

Findings:

- Public-seam `submit_assessments` validation now rejects unbound scalar candidate-labeled dummies and mismatched candidate labels, but a dummy can still move into the new candidate-bound shape. A value such as `{ "candidate": "cand_a", "output": "dummy" }` with matching `evidence.public.summary` is still self-declared document content, not independently verified relation to the candidate output actually assessed.
- The first version over-narrowed locked `OutputRecord` semantics by requiring inline `value` before accepting blob/trace-backed score outputs. Follow-up restored schema compatibility: candidate binding is checked when inline `value` is present; blob/trace-backed outputs still rely on public summary/evidence projection.
- The runtime generic `with_reportable_output(...)` route now rejects explicit `candidate.artifact` records. This closes the old generic declaration path, but `with_reportable_artifact_output(...)` remains a runner declaration rather than independent artifact provenance proof.
- Tests passing is supporting evidence only; rerunning the same tests is not review sign-off.

Accepted partial evidence:

- `crates/leaven-public-seam/tests/plan_document.rs::submit_assessments_rejects_missing_or_placeholder_score_output` rejects unbound scalar candidate-labeled dummies and mismatched pairwise candidate labels.
- `crates/leaven-run/tests/scoring_evaluator.rs::scoring_evaluator_rejects_generic_candidate_artifact_declaration` rejects generic explicit `candidate.artifact` declarations for independent scoring.
- `crates/leaven-run/tests/scoring_evaluator.rs::judging_evaluator_rejects_generic_candidate_artifact_declarations` rejects generic explicit `candidate.artifact` declarations for pairwise/listwise judging.
- `crates/leaven-run/tests/scoring_evaluator.rs::judging_evaluator_preserves_candidate_artifact_reportable_outputs` proves the explicit artifact-output route still preserves candidate-artifact data through judging when deliberately declared.

Blocking status:

- Do not promote `ps1.evaluator.score_output`.
- Do not claim `Score.output.data_classes`, candidate-bound Plan values, or the explicit artifact-output API independently prove that the output is the actual candidate/artifact output assessed.
- A future closeout still needs a stronger provenance/binding primitive or a deliberate row split between public document semantic validation and runtime-assessed output provenance.
