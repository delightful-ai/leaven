# Python Surface Rust-Projection Audit

Status: active cleanup ledger.
Date: 2026-05-24.

## Rule

Python public API is not "whatever Rust exposes as `pub`". Rust has advanced
adapter contracts, proof helpers, and crate-public implementation seams. A
Python public name is allowed only when it is one of:

- **Product public**: ordinary user-facing surface with a stable lowering story.
- **Adapter public**: explicit advanced surface for provider/adapter authors.
- **Wire public**: locked schema-shaped payload, usually behind a curated facade.

Everything else stays private to the engine/transport/runtime implementation.

## Source Stack

1. Locked wire schemas and examples in `docs/specs/public-seam-v1/`.
2. Python product spec at `docs/specs/leaven_python.md`.
3. Rust behavior/proof anchors:
   - `crates/leaven-run/src/evidence.rs`
   - `crates/leaven-run/src/evaluator.rs`
   - `crates/leaven-agentic/src/case_evaluator.rs`
   - `crates/leaven-evidence/src/output.rs`
   - `crates/leaven-evidence/src/feedback.rs`
4. Constraint specs such as `case_visibility_and_target_isolation.md` and
   agentic task/materialization specs.

## Ledger

| Python symbol | Anchor | Current assumption | Verdict | Proposed fix |
| --- | --- | --- | --- | --- |
| `lv.Score` | `leaven-run::Score`; `CaseAssessmentEvidence`; `common.schema#Score` | Product `Score` has `value`, `output`, and a generic `metrics` bag. | Rework | Make product `Score` scalar + feedback + reportable output. Do not expose metrics on ordinary score; the schema's optional public metrics field is a low-level evidence/reporting escape hatch, not the product scoring API. |
| `lv.OutputRecord` | `leaven-evidence::OutputRecord`; `common.schema#OutputRecord` | Hand shape uses `text/blob/structured`, `content`, and private visibility labels not exactly matching schema. | Rework | Align names with public-seam shape while keeping ergonomic constructors. Prefer `value` over `content`; include `json`, `blob_ref`, `agent_session`, `workspace_diff` as reserved kinds. |
| `lv.Case` | `leaven_eval::Case`; `RunCase`; `ScoreCase`; `AgentCase` | One public case can be authoring record and stage projection with target available as an attribute. | Rework | Keep authorable `Case` for tasks, but introduce role-specific projections for stage calls before locking. Runner projection must be structurally target-free. |
| `lv.Task` | Inspect `Task`; `AgentCase` / `CaseSuite`; engine `CaseSet` | Product task owns cases/files/setup/sandbox and lowers implicitly. | Keep with lowering doc | Keep as Python ergonomic authoring bundle. Document lowering to case sets + agentic workload requirements; it is not an engine type. |
| `@lv.runner` / `@lv.scorer` signatures | `ScoringEvaluator`; public-seam `RunnerRequest` / `ScoreContext` | Runner/scorer both receive `lv.Case`; scorer reads `case.target` directly in examples. | Rework | Runner gets input-only case view. Scorer gets output plus `ScoreContext`/case handle and loads target through context. Stage-composition scorers may inspect `cx.rollout_workspace`. |
| `cx.rollout_workspace` | `AgentCaseEvaluator::score(input, workspace)` | Added as public handle for stage-composition scoring. | Keep cautiously | Keep for agentic/command rollout pipelines. It is the runtime-prepared workspace, not a user-materialized workspace. |
| `cx.workspace.materialize_candidate` | `Materializer`; public-seam `workspace_materialize` | Examples make ordinary runners materialize candidates manually. | Rework | Keep for advanced evaluators/proposers. Ordinary rollout pipelines should be engine/runtime materialized through `Rollout.layout`. |
| `AgentInstructions.developer` / `lv.roles.EXECUTOR` | `AgentRunRequest`; artifact materializers; capability roles | Examples imply mutable developer instructions are passed as a role string. | Rework | Treat `developer` as stable prompt/system instruction only. Mutable behavior/instructions belong in artifact/workspace. Rename or soften role helpers before locking. |
| `lv.EvidenceEnvelope` | `leaven.evidence_envelope.v1.schema.json` | Hand dict payload with public/private and no producer/redaction/source receipt structure. | Rework | Make classmethods thin facades over schema concepts: public feedback/summary/metrics, private payload, redaction policy, producer, source receipts. |
| `AssessmentWrite.kind` | Plan/evaluation schemas | Uses `independent_case` while wire uses independent assessment/job vocabulary. | Rework | Align literal names with locked schemas or isolate product builders from wire names explicitly. |
| `EvaluationJob.granularity` | `leaven.evaluation_job.v1.schema.json` | Collapses job kind and assessment granularity into one axis. | Rework | Split assessment shape (`independent/pairwise/listwise`) from granularity (`aggregate/per_case`). |
| `data_class.ARTIFACT_OUTPUT` | `common.schema#DataClass` | Uses `artifact.output`, not locked `candidate.artifact`. | Rework | Rename to `CANDIDATE_ARTIFACT`; keep old name only if intentionally deprecated before publish. |
| `RegisteredStage.func` | ACP stage worker dispatch | Public pydantic field exposes raw callable. | Hide/internalize | Keep callable private in implementation, expose stage id/role/trust only in public handle. |
| `RunContext` builder surface | Engine contexts / public-seam roles | All context flavors expose all builders. | Rework | Split or document role-specific capabilities. Runners should not get proposal/assessment mutation surfaces. |

## Cleanup Slices

1. Score and output cleanup.
2. Agent instruction / mutable artifact behavior cleanup.
3. Case role projection and target isolation cleanup.
4. Context and receipt visibility cleanup.
5. Entry-point naming reconciliation (`evolve`/`optimize`, `runtime`/`environment`).
