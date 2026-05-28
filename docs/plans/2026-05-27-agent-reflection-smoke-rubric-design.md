# Agent Reflection Smoke Rubric Design

Date: 2026-05-27
Status: design approved for implementation planning
Scope: live Codex-backed smoke checks for GEPA agent reflection stages

## Goal

Define a natural-language smoke rubric for agent reflection that is strong
enough to catch fake environments, wrong rights, target leakage, graph mutation
bypasses, and decorative reports.

The proof denominator is:

```text
11 agent-reflection stages x 5 natural-language smoke checks = 55 scored checks
```

Each stage scenario must spawn a real Codex agent in a real temporary workspace.
The checks are natural-language rubrics, but scoring must be grounded in
observable evidence: workspace files, stage payloads, transcripts, receipts,
graph records, reports, and checkpoints.

## Non-Goals

- Do not claim paper parity from this smoke suite.
- Do not replace deterministic crate tests, public-seam schema validation, or
  `just check`.
- Do not use a fake agent, scripted transcript, or schema-only payload as the
  primary proof.
- Do not let Codex workspace edits become graph truth without typed readback and
  `RunContext::propose` plus `apply_batch`.

## Scoring Model

Each smoke check is scored as:

- `pass`: the run produced concrete evidence satisfying the check.
- `fail`: evidence is absent, contradicts the check, or is only asserted in
  prose.

No partial credit. A stage score is `0..5`. The full suite score is `0..55`.

Hard suite failures override the numeric score:

- no live Codex process/session was spawned for a stage scenario;
- hidden target, rubric oracle, answer key, or held-out case material appeared
  in a reflector-visible workspace or prompt;
- a workspace commit, patch, or file edit was treated as an admitted candidate
  without a typed proposal;
- a report omitted parent, child, proposal, case, score, or admission identity
  for a non-skipped attempt;
- a resume replay reused stale Codex output as if it were a fresh live stage
  without an explicit checkpoint/cache receipt.

## Stage Denominator

The 11 stages are the agent-reflection path through GEPA, narrowed to the
reflect/propose/admit loop:

1. Parent candidate selection
2. Train-minibatch binding
3. Parent evaluation evidence
4. Skip gates and part selection
5. Reflective dataset construction
6. Agent workspace materialization
7. Reflection instruction and output contract rendering
8. Live Codex reflection session
9. Workspace readback into a typed change
10. Proposal recording and graph application
11. Child screening, validation, report, and checkpoint

Each stage gets a live Codex scenario. The stage scenario may ask Codex to
inspect or act on the stage environment, but the scorer judges only observable
artifacts and receipts.

## 1. Parent Candidate Selection

1. The Codex-visible stage packet names the selected parent candidate id and
   GEPA candidate index, and those values match the parent-selection event.
2. The environment includes enough validation-frontier evidence for Codex to
   explain why this parent was selectable, without exposing hidden validation
   targets.
3. A decoy train-best candidate that is not validation-frontier selectable is
   present in the harness evidence, and Codex does not select or cite it as the
   reflection parent.
4. The final stage evidence cites validation-frontier membership or dominance
   facts, not only a scalar score or "best so far" statement.
5. The report preserves the parent-selection reason so a later scorer can
   reconstruct the choice without reading transient workspace files.

## 2. Train-Minibatch Binding

1. The Codex-visible manifest names the exact train case ids sampled for this
   attempt.
2. Parent evaluation, child screening, and proposal-attempt records all use the
   same minibatch case ids.
3. Validation and test cases are absent from the reflector workspace except as
   opaque ids or aggregate public report facts.
4. A decoy train case outside the minibatch exists in the fixture, and Codex
   does not evaluate, edit for, or cite it as part of this attempt.
5. The stage writes or emits a stable minibatch fingerprint that can be compared
   against downstream parent/child evidence.

## 3. Parent Evaluation Evidence

1. Codex can inspect the parent artifact's actual run outputs for every
   minibatch case, not only candidate metadata.
2. Each failure diagnosis in Codex output cites a concrete case id plus parent
   output, trace, feedback, or score evidence.
3. Hidden answers and raw rubric oracles are not present in the reflector input,
   workspace files, or transcript.
4. Parent evaluation receipts name the parent candidate, evaluation purpose,
   case ids, and assessment rows that fed reflection.
5. A missing or failed parent run remains visible as failure evidence; it is not
   silently dropped from the reflective dataset.

## 4. Skip Gates And Part Selection

1. The stage evidence states whether reflection was allowed, skipped for no
   examples, or skipped for all-perfect parent evidence.
2. If reflection is allowed, the selected part or component is named in the
   Codex-visible packet and matches the part-selection event.
3. If reflection is skipped, no proposal is recorded and the skip reason is
   still visible in the report.
4. A decoy mutable part is present in the artifact, and Codex does not edit it
   unless it was the selected part or an explicitly allowed coupled edit.
5. The gate decision is bound to parent evidence and policy, not to Codex
   preference or an instruction that can override GEPA.

## 5. Reflective Dataset Construction

1. The reflective dataset contains target-safe case input, parent output, score,
   feedback, side information, and trace refs for the sampled cases.
2. No dataset record contains `case.target`, answer keys, reference solutions,
   or scorer-private rubric material.
3. Every dataset record carries source refs that resolve to parent assessment or
   evidence records.
4. The reflective dataset fingerprint is identical for the LM-backed and
   agent-backed presentation of the same request.
5. Codex's reflection uses the dataset evidence to identify a failure mode or
   edit direction; it does not invent an unrelated task.

## 6. Agent Workspace Materialization

1. The workspace contains the actual parent artifact body under the expected
   current-artifact root, not only metadata or summaries.
2. The workspace contains only target-safe reflection context and approved tool
   inputs; hidden targets and held-out fixtures are absent.
3. Codex can read and, when the stage grants it, edit the materialized artifact
   through normal workspace tools.
4. The workspace manifest names run id, attempt id, parent candidate, selected
   part, artifact fingerprint, and output contract.
5. Mutating a workspace file alone does not create a graph candidate until the
   readback/proposal stages succeed.

## 7. Reflection Instructions And Output Contract

1. Codex receives instructions that name the selected parent, selected part,
   reflective dataset location, allowed edit surface, and output location.
2. The requested output format is concrete enough for an automated parser to
   reject missing, malformed, or stale output.
3. Tool, file-write, network, budget, and approval rights are stated in the
   prompt or manifest and match the runtime configuration.
4. An attempted out-of-contract output or write outside the allowed root is
   scored as failure, not recovered by a permissive parser.
5. The instructions do not leak hidden target material through examples,
   comments, environment variables, filenames, or "helpful" rubric text.

## 8. Live Codex Reflection Session

1. The run records a real Codex process or app-server session id, model/runtime
   identity, start/end timestamps, and exit status.
2. Codex inspects the materialized artifact and reflective evidence before
   producing its final output.
3. Codex performs a meaningful reflection action tied to the observed failures:
   diagnosis, edit, patch, bundle, or explicit no-change decision.
4. When tool use is expected, the transcript contains command/tool evidence
   tied to the workspace, not only a final assistant message.
5. The final Codex output cites concrete files, cases, or receipts that the
   scorer can verify after the process exits.

## 9. Workspace Readback Into A Typed Change

1. The parser reads the final workspace, patch, bundle, or declared output and
   lowers it into the owning artifact's typed change.
2. Invalid artifact edits produce readback diagnostics and no proposal, rather
   than a best-effort malformed candidate.
3. A no-change or irrelevant-change run is represented as no proposal or a
   rejected attempt, not as a fake successful child.
4. Equivalent patch, bundle, or final-workspace forms produce the same typed
   change fingerprint when they describe the same edit.
5. The readback result cites the Codex session, workspace manifest, and changed
   artifact paths used to construct the typed change.

## 10. Proposal Recording And Graph Application

1. The proposal effect is `Change` against the selected parent and carries the
   typed artifact change from readback.
2. Proposal provenance includes the reflective dataset source refs and the
   stage/session receipts that produced the change.
3. `RunContext::propose` followed by `apply_batch` is the only path by which the
   child candidate enters graph truth.
4. A workspace commit, file diff, or provider transcript without a parsed
   proposal does not create a child candidate.
5. The graph/report records proposal id, parent id, child id when present,
   attempt index, effect fingerprint, and application outcome.

## 11. Child Screening, Validation, Report, And Checkpoint

1. Child screening evaluates the child on the same train minibatch used for the
   parent attempt.
2. Accepted children run the configured validation policy before frontier
   admission.
3. The report names parent, child, proposal, cases, parent scores, child scores,
   validation outcome, and admission decision.
4. Rejected, skipped, invalid, and failed attempts remain visible with typed
   reasons and do not disappear from attempt counts.
5. Checkpoint/resume restores candidate/frontier state and stage evidence
   without replaying stale Codex output as a fresh live session.

## Implementation Shape

The intended implementation is a live-gated smoke harness:

```text
LEAVEN_CODEX_LIVE=1 cargo test -p <owning-crate> --features live-codex-tests -- --ignored
```

The harness should run one live Codex scenario per stage. Each scenario may
bundle the five natural-language checks into one prompt and output file, but the
scorer must score the five checks independently.

Suggested output:

```json
{
  "stage": "reflective_dataset_construction",
  "codex_session": "...",
  "checks": [
    {
      "id": "reflective_dataset_construction.target_safe_records",
      "status": "pass",
      "evidence_refs": ["workspace:reflection/examples.json", "receipt:assess_..."],
      "notes": "..."
    }
  ]
}
```

The JSON shape is implementation scaffolding only. The contract is the
natural-language check plus evidence-backed pass/fail scoring.

## Verification Plan

Design-only verification:

```text
test -f docs/plans/2026-05-27-agent-reflection-smoke-rubric-design.md
```

Implementation verification will be planned separately. It should include:

- the narrow owner crate tests for the smoke harness;
- the relevant Codex provider live gate;
- `cargo test -p leaven-gepa-agentic-skill` or
  `cargo test -p leaven-gepa-agentic-git`, depending on the first artifact;
- `cargo test -p leaven --test topology_contract` if crate boundaries or live
  feature wiring change;
- `just check` before claiming behavior complete.
