## Boundary
This crate owns reusable adapters that turn agent sessions into Leaven stage
outputs.

It may know engine, core, surface, workspace, render, store, and provider-neutral
agent vocabulary because it is the bridge from materialized workspaces and
runtime sessions into `ProposalBatch` or `Assessment` values. It must not own
provider protocol details or optimizer-specific search rhythm.

## Map
- `AgenticProposer` allocates a workspace, materializes allowed context, renders
  instructions, runs an `AgentRuntime`, parses session/workspace output, and
  returns proposals.
- `AgenticEvaluator` and `AgentCaseEvaluator` adapt agent sessions into
  assessments and case-run records.
- `CaseSuite`, `AgentWorkload`, `CasePartitions`, hidden targets, setup files,
  run policy, retry records, and approval policy are generic agent workload
  vocabulary.
- `ProposalParser`, `EvidenceParser`, and `EvaluationInputBuilder` are the
  import seams from runtime/session facts into typed Leaven outputs.
- `PublicStagePayloadIdentity`, `ReflectRequestPayload`,
  `ReflectionResultPayload`, `ProposeRequestPayload`, and
  `ReflectProposeHandoffPayload` lower generic agentic reflection/proposal
  stages into the locked public-seam stage-payload wire contract without
  making this crate the public-seam validator or graph mutation authority.
- `RunnerRequestPayload`, `ScorerContextPayload` with
  `ScorerContextPayloadFields`, `JudgeContextPayload` with
  `JudgeContextPayloadFields`, `CallbackRequestPayload`, and
  `AdapterRequestPayload` lower generic runner, scorer, judge, callback,
  artifact adapter, and dataset adapter dispatch payloads into the locked
  public-seam role-specific wire contract without making this crate the
  runtime, provider implementation, or schema validator.
- Repairing proposers own bounded re-prompt policy; repair is stage policy, not
  a provider-runtime responsibility.

## Route Away
- Provider SDKs, Codex protocol types, CLI flags, and local-mount connector
  rules belong in `leaven-agent-*` provider leaves.
- Provider-neutral one-session runtime vocabulary belongs in `leaven-agent`.
- Skill-specific materializers, layouts, and skill-bank proposal parsers belong
  in `leaven-agentic-skill`.
- GEPA selectors, population updates, and optimizer rhythm belong in optimizer
  crates such as `leaven-gepa`.
- Generic artifacts/surfaces belong in artifact crates; this crate consumes
  materializers and parsers instead of defining every domain artifact.

## Proof Anchors
- `crates/leaven-agentic/tests/agentic_contract/agentic_adapters.rs` proves
  proposer/evaluator workspace lifecycle, runtime cost flow, parser
  boundaries, graph application, and cleanup failure handling.
- `crates/leaven-agentic/tests/agentic_contract/agentic_workload.rs` proves
  case-suite fingerprinting, hidden targets, run policy, preflight, case
  records, cache policy, and workload evaluator behavior.
- `crates/leaven-agentic/tests/agentic_contract/repairing_proposer.rs` proves
  bounded repair loops, repair metadata, inspection, and exhausted repair
  behavior.
- `crates/leaven-agentic/tests/agentic_contract/public_seam_stage.rs` proves
  agentic-owned reflection/proposal stage lowering crosses the locked
  public-seam owner with separate ReflectRequest, ReflectionResult,
  ProposeRequest, and binding stage receipts.
- `docs/specs/agentic_stage_runtime.md` owns the generic runtime/stage split.
- Run `cargo nextest run -p leaven-agentic` to prove generic agentic adapter
  behavior.

## Public Maturity

Crate-root exports for `PublicStagePayloadIdentity`,
`ReflectRequestPayload`, `ReflectionResultPayload`,
`ProposeRequestPayload`, `ReflectProposeHandoffPayload`,
`RunnerRequestPayload`, `ScorerContextPayload`, `ScorerContextPayloadFields`,
`JudgeContextPayload`, `JudgeContextPayloadFields`, `CallbackRequestPayload`,
and `AdapterRequestPayload` are advanced public seam-lowering contracts for
adapter authors. They are intentionally not in `leaven_agentic::prelude`; they
do not prove ACP transport, graph mutation, provider execution, or concrete
provider runtime behavior.

## Decision Cards
- when: turning an agent session or workspace mutation into a proposal
  do: keep runtime execution in `leaven-agent*`, materialize through workspace/render helpers, then parse typed proposals in this crate or a shape-specific adapter
  preserve: `AgentSession` as runtime fact and `RunContext` as the only graph mutation path
  avoid: treating provider transcript text or workspace diffs as graph records before a parser validates them
  verify: run `cargo nextest run -p leaven-agentic --test agentic_contract agentic_adapters`

- when: adding cache, retry, or repair behavior around agentic evaluation
  do: make the fingerprint inputs explicit and keep bounded repair as stage policy
  preserve: hidden target boundaries, attempt history, cleanup failure reporting, and cost accounting
  avoid: default caching for nondeterministic agent evaluators without a law over runtime/workload/presenter/scorer/candidate identity
  verify: run `cargo nextest run -p leaven-agentic --test agentic_contract agentic_workload repairing_proposer`

- when: adding agentic case or workload policy
  do: distinguish live evaluator behavior from policy vocabulary that is only recorded or preflighted today
  preserve: hidden target boundaries, case-suite fingerprinting, per-case assessment records, retry records, and workspace cleanup accounting
  avoid: documenting `max_parallel_cases`, `max_parallel_workspaces`, `score_on_error`, `fail_on_error`, or checkpoint policy as live scheduler behavior until `AgentCaseEvaluator` actually implements it
  verify: run `cargo nextest run -p leaven-agentic --test agentic_contract agentic_workload` and add an evaluator test, not just a serialization test, when policy becomes operational

- when: adding presenter/scorer helpers
  do: keep case input/workspace requirements in `leaven-agentic` and artifact-specific projection in a shape-specific adapter crate
  preserve: hidden `CaseTarget` values as scorer-visible only and `AgentCasePresentation.materialized_refs` as the explicit candidate-visible footprint
  avoid: moving agentic task/environment semantics into `leaven-eval` or leaking hidden targets through materializers
  verify: run the preflight and evaluator paths in `cargo nextest run -p leaven-agentic --test agentic_contract agentic_workload`

## Local Bait
- `docs/specs/public-seam-v1/` locks the public seam for external-worker
  reflection/proposal/assessment flow: stage payloads, evidence envelopes, and
  the ACP profile that delivers Leaven extension methods to workers. The
  structural split between reflection (diagnosis) and proposal (graph mutation
  intent) is governing judgment; preserve it when shaping adapter parsers. This
  crate now owns generic producer-side stage-payload lowering helpers for
  reflect/propose plus runner, scorer, judge, callback, artifact adapter, and
  dataset adapter payload roles, but that is not ACP transport, provider
  execution, or graph mutation authority.
- Agent workspace mutation is not graph mutation. Only parser-produced
  proposals or assessments enter the graph through `RunContext`.
- Agentic evaluators are nondeterministic by default. Do not make evaluation
  caching the default without a fingerprint/cache-identity law for the runtime,
  workload, presenter, scorer, and candidate content.
- Do not import Codex app-server protocol here even for "better transcripts";
  provider raw events stay provider-leaf facts until normalized through
  `AgentSession`.
- `AgentCaseRunPolicy` is ahead of some live behavior. Treat its retry path as
  implemented; treat parallelism, score-on-error, fail thresholds, and
  checkpoint policy as vocabulary until evaluator tests prove scheduling and
  failure semantics.
- Materialization and rendering return costs, but current engine contexts expose
  budget snapshots rather than mutable budget handles. Agentic stages must keep
  returning accumulated `Cost`; do not hide provider or workspace spend inside
  helpers.
