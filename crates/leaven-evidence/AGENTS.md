## Boundary
This crate owns reusable evidence value shapes: scalar scores, pairwise judgments, paired rollout rewards, casewise outcomes, command/trajectory records, skill-use telemetry, analyst fan-out records, patch merge-tree records, feedback, attribution, and placeholder shapes for diff/json/list/vector/string evidence.

Evidence here is data a stage or evaluator can produce and another component can interpret. It is not a store, scorer, population, preference relation, graph event, or evaluator registry.

## Routing
- Put finite single-objective scores in `src/scalar.rs`; non-finite refusal belongs at construction so preference and population code never decides what `NaN` means.
- Put pairwise outcomes in `src/pairwise.rs`; fitted ability state and tournament updates belong in `leaven-population`.
- Put baseline-versus-treatment rollout group rewards in `src/rollout.rs`;
  skill-credit assignment and utility state updates belong in
  `leaven-population`.
- Put skill-use telemetry events and trajectory-level skill-use evidence in
  `src/skill_use.rs`; transcript parsers, route-key extraction, and utility
  updates belong outside this crate.
- Put per-case evidence containers in `src/casewise.rs`; dataset split policy belongs in `leaven-eval`, and engine case-set resolution belongs in `leaven-engine`.
- Put caller-keyed attribution in `AttributableEvidence`; do not make attribution keys surface-only, path-only, or GEPA-only.
- Store references and persistence capabilities belong in `leaven-store-*`, not in evidence values.

## Current Public-Maturity Split
- Behavior-bearing today: scalar scores, pairwise judgments, paired rollout
  rewards, casewise sparse containers, command/agent trajectory records,
  attribution traits, and `CaseAssessmentEvidence` have local tests.
  `CaseAssessmentEvidence` preserves generated output, scalar score, and
  natural-language feedback; it is reusable evidence vocabulary, not the
  reflective mutation algorithm.
- `AgentTrajectoryEvidence` is the reusable one-session trajectory envelope:
  runtime session id, optional Leaven case id, upstream task id, typed
  success/failure outcome, model id, model configuration fingerprint,
  transcript/blob reference, command records, and parsed/blob-backed analyst
  records. It is not a scheduler, ReAct runner, scorer, or Trace2Skill merge
  policy.
- `SkillTrajectoryUseEvidence` is the reusable one-trajectory skill telemetry
  envelope: upstream task id, trajectory id, finite reward, and ordered
  `SkillUseEvent` records with skill identity, kind, source, confidence, step
  index, and optional supporting output. It is not a router, transcript parser,
  utility updater, or D2Skill pool classifier.
- `AgentTrajectoryCorpusEvidence` is the checkpointable many-trajectory value:
  a caller-declared task manifest plus appended `AgentTrajectoryEvidence`
  records with completed/pending task projection. Persist it through generic
  `EvidenceStore` or caller-owned checkpoints; do not make store backends know
  this schema.
- `AgentAnalystFanoutEvidence` is the checkpointable many-call value for
  independent analyst/sub-agent calls: caller-declared call ids, per-call role,
  source task ids, prompt/response payloads, terminal parse/backend status,
  retry count, and support count. It is not a scheduler, thread pool, model
  client, patch parser, or hierarchical merge policy.
- `AgentPatchMergeTreeEvidence` is the checkpointable merge provenance value
  for agent-authored patches: merge levels, input/accepted/discarded patch ids,
  support counts, merge decisions, prompt/response payloads, parse-failure
  artifacts, per-node output patches, and optional final diff. It is not a
  merge scheduler, prevalence policy, patch parser, or skill-directory applier.
- Public placeholders today: `diff`, `json`, `listwise`, `mixed`,
  `score_vector`, and `string` are root-re-exported names without behavior laws.
  Do not cite them as standard evidence until they carry fields, constructors,
  and tests.
- `CaseAssessmentEvidence` is the reusable scored case-output shape for the
  runner/scorer path: generated output, scalar score, and natural-language
  feedback. It is still evidence data, not the reflective mutation algorithm.

## Local Helper Stack
- Use `ScalarEvidence::new` for any score crossing crate boundaries; downstream
  preference/population code assumes non-finite values were refused already.
- Use `CasewiseEvidence` for sparse per-case data. Missing case IDs mean
  absence, not zero score.
- Use `PairedRolloutEvidence` when a paper compares a baseline group against a
  treatment group for the same upstream task. It records non-empty group sizes,
  finite mean rewards, and the treatment-minus-baseline gap; it does not know
  what changed in the treatment or how credit should update population state.
- Use `SkillTrajectoryUseEvidence` when a runner, parser, or scorer can preserve
  which validated skills were retrieved, injected, or triggered during one
  rewarded trajectory. Unknown telemetry stays absent; do not turn absence into
  a false "not used" event.
- Use `OutputRecord::BlobRef` for large stdout/stderr, transcripts, and parsed
  analyst payloads; `OutputRecord::Inline` is bounded display evidence.
- Use `AgentTrajectoryCorpusEvidence` when a paper or runner must resume over a
  known task manifest. Duplicate manifest task ids are refused at construction,
  unknown task ids are refused at insertion, and repeated trajectories for a
  task are allowed for multi-seed or retry protocols.
- Use `AgentAnalystFanoutEvidence` when a paper or runner must resume over a
  known analyst-call manifest. Duplicate call ids are refused at construction,
  unknown call ids are refused at insertion, pending records remain pending
  until terminal status, and parse failures are durable terminal records rather
  than missing work.
- Use `AgentPatchMergeTreeEvidence` when a paper or runner must preserve the
  shape and result of hierarchical patch consolidation. Node ids are unique,
  the final node id must exist, support is positive, and parse-failed nodes
  remain queryable as evidence.
- Use `AttributableEvidence<K>` when evidence needs to point at surface parts,
  paths, agents, tools, modules, or user keys without making this crate know
  those key domains.

## Local Bait
- `docs/specs/public-seam-v1/schemas/leaven.evidence_envelope.v1.schema.json`
  and the surrounding spec lock the public/private visibility split for
  evidence crossing the worker boundary. The reusable evidence shapes in this
  crate are the durable source those envelopes lower from; visibility itself
  lives in values and receipts per the architecture judgment, not only in
  policy. The wire bridge is not implemented yet.
- Human prose fields such as rationales and notes are debug context. Algorithms should route on typed fields such as `ScalarEvidence::score`, `PairwiseJudgment`, and `CaseOutcome`, not require prose to exist.
- Placeholder modules in `src/lib.rs` are naming reservations, not permission to hide real implementation in `lib.rs`. Move behavior into the named module first.
- The crate doc still says skeleton, and audit docs flag that as stale/ambiguous.
  Fix metadata separately from symbol maturity: some exports are real and some
  are placeholders.

## Proof Anchors
- `cargo nextest run -p leaven-evidence` proves scalar, pairwise, paired
  rollout, casewise, command/trajectory, skill-use telemetry, analyst fan-out,
  patch merge-tree, and attribution behavior. It does not currently prove every
  root-re-exported evidence name.
- `cargo nextest run -p leaven-preference --test scalar` proves scalar preference callers rely on `ScalarEvidence`'s finite-score contract.
- `cargo nextest run -p leaven-population --test tournament` proves pairwise evidence feeds fitted population state outside this crate.
- Before adding an evidence name to `leaven-std`, add a focused test in this
  crate and update the public-maturity/export ledger pressure from
  `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/surface-requirements.md`.
