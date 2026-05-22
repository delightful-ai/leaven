# Skill Paper Replication Ledger

Status: active.
Updated: 2026-05-22.

## Authority

Product authority remains in:

- `docs/specs/initial_library.md`
- `docs/specs/agentic_skill_optimization_primitives.md`
- `docs/plans/2026-05-07-milestone-5-skill-paper-reproductions.md`
- `tmp/skill_opt_sources/manifest.json`
- current code/tests and emitted run artifacts

This file is a continuation ledger. It records what Leaven must add to support
paper-faithful replications of the five skill-optimization papers. Verify every
referenced path before claiming a row is complete.

## Replication Bar

The target is 1:1 replication, not an inspired implementation. A paper example
or product crate may contain the paper-specific configuration, prompts, dataset
adapters, and result rendering. Generic behavior discovered while trying to
replicate must move into Leaven primitives first, then the paper replica should
retry against those primitives.

Do not spend major provider, cloud, GPU, or benchmark-access budget without
explicit approval. When a paper depends on private, gated, or very expensive
data/compute, record the exact blocker and run only no-spend setup/provenance
checks until the user approves a live run.

Language boundary: Python is allowed only for scaffolding, read-only
provenance probes, and external upstream inspection when that is the released
reference stack. It is not allowed for Leaven-owned paper replication logic,
runnable replica paths, samplers, scorers, splitters, harnesses, artifact
materialization, repair loops, population/frontier logic, retrieval/utility, or
telemetry primitives. If a paper's upstream release is Python, treat it as
reference evidence or an explicitly invoked external baseline; Leaven-side
replica behavior must be Rust/Leaven primitives with thin paper configuration.

The working loop for each paper:

1. Build a replication dossier from local paper sources and any released code.
2. Attempt the thinnest faithful paper run.
3. When it fails because Leaven lacks a generic primitive, add the primitive in
   the owning crate with tests.
4. Retry the paper run with minimal example-side logic.
5. Record exact deltas, spend, seeds, model/provider versions, and artifacts.

## Current Leaven State

Current useful substrate:

- `leaven-artifact-skill` owns `SkillBank`, `SkillFolder`, `SkillBankChange`,
  `SkillCard`, `SkillRouteRegistry`, skill validation, metadata, and skill
  manifest/body/file/reference surfaces. The current card is the manifest-only
  routing catalog view over names, descriptions, and generic metadata. The
  route registry is an explicit pool/key overlay over a validated bank; learned
  utility/retrieval counters remain outside `SkillBank`. `SkillTokenProfile`
  now records tokenizer-identified token counts for manifest descriptions,
  `SKILL.md` bodies, and direct `references/*.md` modules, while leaving exact
  tokenizer implementations and paper cost curves outside the artifact crate.
- `leaven-agentic-skill` owns skill-bank materialization, workspace readback,
  diffs, and proposal parsing for agent-authored skill changes.
- `leaven-agentic-git` owns Git-program materialization/readback for
  `GitProgramArtifact`: durable bare-store commits are checked out into
  disposable workspaces, dirty or committed workspace children are frozen and
  imported back into the durable store, explicit `output/proposal.patch` and
  `output/proposal.bundle` artifacts can be imported before checkout fallback,
  and the result is a typed `GitProgramChange` for later graph admission.
- `leaven-workspace-firkin` now owns a fake-runtime-tested Firkin product-pod
  workspace adapter contract: allocation carries product-pod/container/root/image
  context, command/file operations are routed through the guest workspace root,
  command output byte limits are forwarded and enforced, cleanup removes the
  container, and unsupported executable-bit operations fail explicitly. The
  `firkin-facade` feature is reserved but intentionally has no external
  Apple/VZ path dependency until a live adapter proves runtime behavior.
- `leaven-agentic` has proposer repair policy support.
- `leaven-gepa-agentic-skill` bridges GEPA reflection to skill-bank proposal.
- 2026-05-20: `leaven-gepa-agentic-skill` is being migrated from bespoke
  `renderer.rs` / `materializer.rs` / `parser.rs` to a
  `SkillBankReflector: ArtifactReflector` impl, running through the generic
  `ReflectionWorkspace` in `leaven-agentic`. Governing spec:
  `docs/specs/typed_signature_adapter_contract.md`.
- `leaven-gepa-agentic-git` bridges GEPA reflection to Git-program agentic
  proposal: `ReflectRequest` is passed down without rebuilding examples, the
  parent `GitProgramArtifact` is materialized into a workspace, provider-neutral
  instructions name checked-out repo paths, readback yields a typed
  `GitProgramChange`, and the deterministic no-spend proof records the batch
  through `RunContext::propose`, applies it with `apply_batch`, and emits tiny
  EvoSkill-shaped frontier/admission events.
- `examples/p5_evoskill_iteration` proves one live Codex EvoSkill-shaped
  iteration over a tiny treasury-notation fixture. It can now run a no-spend
  `--inspect-cases` pass over the materialized OfficeQA `UID0001` sample and
  source text, and over the materialized SealQA seal-0 sample, but those are
  provenance inputs only; both samples are validation-only and do not form
  runnable paper splits.
- `examples/memento_skills_read_write`, `examples/skillreducer_tiny`,
  `examples/d2skill_tiny`, and `examples/trace2skill_tiny_live` are restored
  outside-Cargo paper-specific live harnesses with no-spend preflight modes and
  explicit live Codex opt-ins. They preserve tiny causal loop shapes and
  generated artifacts, but they are not full paper replications.
- `leaven-population` has `ParetoFrontier`, `TopKFrontier`, and deterministic
  `TopKParentSelector` state for best/round-robin parent selection, plus
  `SkillUtilityState` for checkpointable skill utility/use bookkeeping:
  retrieval counts, trigger counts, finite EMA utility updates, explicit rename
  transfer, explicit removal, and `SkillUtilityRanker` for deterministic top-k
  ranking over caller-provided relevance plus utility/UCB exploration,
  `SkillTwoStageRetriever` for D2Skill-style pool-scoped similarity
  threshold/top-m plus utility/UCB top-k retrieval, and `SkillUtilityPruner` for
  D2Skill-style capacity pruning plans over active skill pools. EvoSkill P5 now
  checkpoints frontier/parent/selector state, but full paper replication still
  needs git-program run-loop integration and dossier resolution of the
  paper/code selector drift.

Current known gaps:

- `crates/leaven-artifact-git` now models normalized paths, immutable object
  IDs, branch/tag refs, typed lineage, content/cache identity, and ref removal
  for discarded candidates. `crates/leaven-workspace-git` can clone, project,
  check out, fsck, import child commits, capture tracked files and branch/tag
  refs, restore a selected ref, and delete branch/tag refs.
  `crates/leaven-agentic-git` can materialize multi-repo `GitProgramArtifact`
  values into proposer workspaces and read back imported child commits or
  explicit output patch/bundle proposals as typed artifact changes.
  `crates/leaven-gepa-agentic-git` now proves the generic GEPA reflection bridge
  and tiny frontier admission around that path. EvoSkill still needs
  paper-run integration for checkpointed program/frontier state, paper score
  metadata conventions, and OfficeQA/SealQA run surfaces.
- The P5 EvoSkill path is not OfficeQA or SealQA. It can inspect one real
  OfficeQA sample and one real SealQA sample without live spend, but still lacks
  the paper datasets, paper splits, full frontier loop, feedback-history
  schedule, held-out test reporting, skill-merge evaluation, paper scorers, and
  ablations.
- Trace2Skill now has mechanics-smoke coverage for SpreadsheetBench manifest
  lowering, upstream run-artifact import, JSON patch lowering/application,
  merge artifact replay, and saved-output directory loading. The tiny live
  Trace2Skill shell harness is separate and still a proxy live loop, not
  SpreadsheetBench/Qwen/vLLM parity.
- Memento-Skills, D2Skill, and SkillReducer now have tiny shell harnesses and
  READMEs recording source anchors and known deviations. They still lack
  Leaven-owned generic primitives, exact datasets, benchmark splits, official
  model/env profiles, ablation matrices, and paper metric reproduction.
- No per-paper dossier currently records enough exact prompts, seeds, model
  versions, splits, metrics, released-code drift, and cost envelope to claim
  full paper replication.

## Paper Rows

### EvoSkill (`arx_2603.02766`)

Source anchors:

- paper claims OfficeQA +7.3 points, SealQA +12.1 points, BrowseComp transfer
  +5.3 points: `tmp/skill_opt_sources/arx_2603.02766/full_source.md:9`.
- main loop says fixed-capacity frontier, round-robin parent selection,
  without-replacement training batches, failure threshold, proposer feedback
  history, validation admission, and weakest-frontier eviction:
  `tmp/skill_opt_sources/arx_2603.02766/full_source.md:56`,
  `:58`, `:64`.
- OfficeQA is 246 questions over about 89k Treasury Bulletin pages, with 17
  validation examples, train sizes 12/24/36, 1.5 epochs, held-out test, and
  skill-merge reporting: `tmp/skill_opt_sources/arx_2603.02766/full_source.md:84`,
  `:90`.
- SealQA uses seal-0, 111 questions, Claude Code Opus 4.5, 10 percent train,
  held-out remainder, and 1.5 epochs: `tmp/skill_opt_sources/arx_2603.02766/full_source.md:143`.
- scorer uses tolerances `{0.0, 0.01, 0.025, 0.05, 0.10}` and flags weighted
  score below 0.8 as failure: `tmp/skill_opt_sources/arx_2603.02766/full_source.md:790`.
- appendix describes git branches under `program/`, frontier tags under
  `frontier/`, default frontier `K=3`, and branch deletion for discarded
  children: `tmp/skill_opt_sources/arx_2603.02766/full_source.md:794`.

Replication obligations:

- replicate OfficeQA and SealQA dataset acquisition, stratification, train/val/test
  splits, category-aware sampling, exact fuzzy scorer, held-out metrics, and
  skill-merge condition;
- replicate the paper's proposer/builder/autograder prompt surfaces and
  Claude Code skill-folder protocol;
- preserve frozen underlying model semantics and record model/provider version;
- preserve frontier capacity, selector, admission, no-improvement stop,
  feedback history, checkpoint/resume, and candidate lineage;
- reproduce BrowseComp transfer from SealQA skill on the 128-example stratified
  sample when data and provider spend are approved;
- run ablations for skill-only, prompt-only, and paper-reported train splits.

Leaven primitive blockers:

- Git program/frontier state: pure artifact vocabulary plus local clone,
  capture, restore, and delete primitives are present; remaining blocker is
  EvoSkill run-loop integration plus paper score metadata conventions;
- `leaven-population`: explicit top-k frontier/admission and checkpointable
  best/round-robin parent selector state now exist; P5 carries that state in
  private checkpoints and reports actual child admission/best-frontier truth,
  while full EvoSkill still needs git-program paper-run integration and
  selector-policy resolution;
- `leaven-eval`: `CategoryRoundRobinSampler` now provides checkpointable
  category-aware without-replacement train-pool sampling; `StratifiedSplitBuilder`
  now provides exact disjoint train/validation/test or custom role membership
  over caller-supplied strata; `ExplicitSplitBuilder` now preserves
  paper-supplied exact role membership over a known case universe;
  `SourceRowManifest` now fingerprints caller-supplied ordered source-row ids
  and lowers them into row-stable Leaven cases for row-order or explicit split
  construction. `leaven-eval-parquet` now reads Parquet rows into those
  manifests with typed string/list column helpers and file-byte fingerprints.
  Exact EvoSkill OfficeQA source manifests remain absent; P5 now threads sampler
  state through its private checkpoint and samples explicit feedback case IDs
  from train pools;
- paper-grade scorer/evaluator records with failure-threshold extraction;
- evidence history that can feed proposer prompts without paper-side custom
  plumbing.
- EvoSkill OfficeQA scorer replay now has a Rust operator path:
  `just evoskill-paper-score-officeqa <predictions.jsonl>` consumes strict
  slot-keyed prediction JSONL, refuses blocked slots, computes row scores with
  the Rust scorer, writes checked score evidence without target/reference
  fields, and writes the strict score result sidecar for final-report import.
  This is score evidence plumbing, not an agent run or split/source proof.

Notes:

- The paper text itself has a selector drift: the method section says
  round-robin parent selection, while the appendix describes selecting the
  highest-scoring frontier program. The dossier must resolve this against
  released code before any "1:1" claim.

Next attempt:

- Start with EvoSkill because Leaven already has P5 substrate and the source
  bundle contains the richest local anchors. First create an EvoSkill dossier
  and run a no-spend setup attempt for OfficeQA/SealQA data, scorer, splits,
  and git-program frontier. Git artifact/workspace primitives plus top-k
  admission/selector state now exist; the next Leaven-side EvoSkill primitive
  pressure is checkpointed paper-run integration around those pieces.

2026-05-19 no-spend probe:

- `docs/working-memory/evoskill-replication.md` now records the first
  OfficeQA/SealQA setup attempt. Current source/data blockers come before code:
  local EvoSkill/OfficeQA checkouts are stale relative to remote HEAD, the
  paper's OfficeQA category/split manifest is not present in the local CSV, no
  local SealQA `seal-0.csv` or BrowseComp transfer sample was found, and static
  source inspection shows direct upstream `scripts/run_loop.py` imports `Agent`
  from the wrong package in the local checkout. After those provenance blockers
  are resolved, the next Leaven blocker is wiring the tested Git
  artifact/workspace primitives into the EvoSkill run loop with checkpointed
  program/frontier state.

2026-05-20 source-bundle recheck:

- The synchronized `tmp/skill_opt_sources/arx_2603.02766` bundle now provides
  current paper/source anchors for the EvoSkill dossier. The category/split
  blocker remains: local OfficeQA has only `difficulty`, while the paper source
  requires LLM-clustered single categories and exact disjoint
  train/validation/test membership. Bounded source probes found references to
  `solved_dataset.csv`, `pseudo_labels`, author-local result pickles,
  `seal-0.csv`, and BrowseComp pickle/sample artifacts, but not the artifacts
  themselves. The dossier also records a paper-source comparison wrinkle:
  OfficeQA skill-merge exact match is 67.9 in prose and 68.1 in the checked-in
  table.

2026-05-21 full `tmp/` workspace recheck:

- After syncing the full main `tmp/` tree into
  `/Users/darin/src/personal/leaven-trace2skill-ws`, the active paper workspace
  has the full local EvoSkill/OfficeQA substrate (`tmp/repros/evoskill` 670M,
  `tmp/repros/officeqa` 46G, `tmp/replication/evoskill` 28K). Re-running the
  bounded missing-artifact search still did not find EvoSkill's
  `solved_dataset.csv`, `full_run_new_evolved_final_two.pkl`,
  `ablation_run_incorrect.csv`, local `seal-0.csv`, `deep_cc_runs`, or
  BrowseComp sample/result pickle.
- Fetching source refs without checkout showed EvoSkill `origin/main` now at
  `8d503845c8d66a2fa458f06de3988d709774e88a`, while the local checkout remains
  `e881c715dcabb525b30981c4a8e344937e3f944b`. Current EvoSkill `origin/main`
  and tag `v1.2.0` still expose only sample/demo OfficeQA data, not the missing
  paper category/split artifacts.
- OfficeQA is sharper than before: the configured remote is
  `https://github.com/databricks/officeqa.git`, remote `main` now points at
  `49e893d97db5d5d32fb7fbb73d913e9e2829c6d9`, and the local CSV-bearing commit
  `78748e5d669d7f8796bb5ad26556a260d5bb7e03` is not contained in the fetched
  remote branches after the upstream force update. The local CSVs are still
  useful evidence, but using them for replication now requires an explicit
  source-pin/archive decision and still does not provide EvoSkill's paper
  `category` / `pseudo_labels` split manifest.
- `leaven-eval-parquet` now closes the generic SealQA physical-input primitive:
  it can read Parquet rows into `SourceRowManifest` with a file-byte fingerprint
  and typed string/list extraction. SealQA still needs a source-pinned
  train/held-out split manifest and LLM-as-judge scorer before it is a runnable
  paper-faithful replica input.

2026-05-22 EvoSkill target-reporting update:

- P5 schema v7 now carries OfficeQA paper result targets as typed manifest
  data. It records baseline exact match at 60.6 percent and both skill-merge
  exact-match candidates, 67.9 from prose/figure caption and 68.1 from the
  checked-in table. This reports the ambiguity instead of leaving it as an
  `officeqa_reported_result_target` blocker; it does not create Leaven scores
  or resolve source/split membership.
- P5 now also carries typed headline targets for the other EvoSkill reported
  results: SealQA baseline 26.6 percent and optimized 38.7 percent
  LLM-judge accuracy, plus BrowseComp baseline 43.5 percent and SealQA-skill
  transfer 48.8 percent accuracy. These are comparison targets only; they do
  not score SealQA, locate the exact SealQA split, or locate the BrowseComp
  128-example transfer sample.
- P5 schema v8 added typed `source_blockers`, so unresolved source policy,
  missing OfficeQA category/exact split artifacts, missing SealQA exact split
  membership, and absent BrowseComp transfer source are represented as
  source-blocked denominator entries with local path candidates. This is still
  not paper-close scoring evidence.
- P5 schema v9 hard-cuts those source blocker candidates from loose strings to
  checked local path evidence: existence, file/dir shape, byte count, and
  SHA-256 for files. This makes missing exact source artifacts auditable in the
  manifest without treating substitute inputs as paper-close proof.
- P5 schema v10 adds source-backed SealQA judge pinning: the scorer manifest's
  judge template carries the checked paper-source artifact existence, byte
  count, and SHA-256, and the request builder consumes the pinned template
  manifest. This is prompt/request evidence only, not a judge-scored run.
- P5 schema v11 adds optional source pin manifest ingestion at
  `tmp/replication/evoskill/source_pin_manifest.json`. A schema-v1
  `local_checkout_pinned` sidecar must match every known source checkout's
  id/path/head/branch/origin before `source_pin` is removed; mismatches fail the
  manifest build instead of becoming fallback proof.
- `just evoskill-paper-pin-local-sources` is the explicit no-spend operator
  path for writing that sidecar from the current local checkout identities. This
  chooses a local-checkout denominator only; it is not paper-release or
  remote-current source evidence.
- P5 schema v12 adds optional split policy manifest ingestion at
  `tmp/replication/evoskill/split_policy_manifest.json`. A schema-v1
  `accept_documented_paper_close_substitutes` sidecar must match every
  materialized substitute split's dataset id, split id, source-row fingerprint,
  split fingerprint, role names, role row counts, and role source-id
  fingerprints before OfficeQA/SealQA split blockers are removed. The accepted
  splits stay `paper_close_substitute`; stale or partial sidecars fail the
  manifest build instead of becoming exact proof.
- `just evoskill-paper-accept-substitute-splits` is the explicit no-spend
  operator path for writing that sidecar from current local substitute split
  fingerprints. This chooses a paper-close split denominator only; it is not
  paper-exact split membership or score evidence.
- P5 final report schema v6 added typed `paper_close_gates`, separating proven
  no-spend manifest/report/mechanics/proxy-closeout surfaces from
  source-blocked source/split work and approval-blocked live/judge execution.
  Schema v7 added a loop `run_manifest` that binds the mechanics loop to the
  replica manifest/scorer/source/split fingerprints, frontier policy, schedule,
  checkpoint boundary, Git identity mode, fake runtime, and fixed score source.
  Schema v12 extends that binding to the OfficeQA validation role/fingerprint
  used before frontier admission. The final report now materializes sources once
  and runs the loop from that same OfficeQA materialization, with explicit drift
  checks against the embedded manifest. This is closeout audit evidence, not
  paper-close scoring evidence.
- P5 final report schema v8 links each score slot to the paper result target
  ids it would compare against. OfficeQA optimized slots point at both
  skill-merge candidates, SealQA baseline/optimized slots point at their judge
  accuracy targets, and the absent BrowseComp transfer denominator now appears
  as source-blocked held-out score slots for baseline and SealQA-skill transfer
  instead of disappearing from `score_slots`. Scores are still absent.

2026-05-22 product-backend bridge proof:

- `crates/leaven-gepa-agentic-git` now has the first deterministic no-spend
  GitProgram reflection proof: the parent repo is materialized, the rendered
  instructions expose repo context and reflection brief, a workspace edit is
  imported into durable Git storage as a typed `GitProgramChange`, the proposal
  is recorded/applied through `RunContext`, and provenance preserves the
  selected parent candidate.
- The proof also wraps the child in a tiny EvoSkill-shaped top-k frontier:
  seed parent selection, fixed validation score/admission, best-candidate/best-score
  report, and `PopulationUpdated` events are asserted. This closes the generic
  GitProgram product-backend denominator, not OfficeQA/SealQA/BrowseComp
  parity.

### Trace2Skill (`arx_2603.25158`)

Source anchors:

- three stages are trajectory generation, parallel multi-agent patch proposal,
  and conflict-free hierarchical consolidation:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:19`.
- analysts are dispatched concurrently with no sequential dependency:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:99`.
- consolidation identifies prevalent patterns and discards idiosyncratic
  patches: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:129`.
- SpreadsheetBench-Verified split is 200 evolving / 200 held-out from 400, OOD
  WikiTQ is converted to spreadsheet format, and results average seeds
  41/42/43: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:149`.
- models are Qwen3.5-122B-A10B and Qwen3.5-35B-A3B via vLLM; Stage 1 uses one
  trajectory per problem, Stage 2 uses 128 sub-agents, merge batch size 32, and
  ReAct turn budget 100: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159`.
- primary Avg equally weights in-distribution SpreadsheetBench Vrf/Soft/Hard
  and OOD WikiTQ across both model scales:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:401`.
- math uses DAPO-Math-Train-400, DAPO-Math-Test-100, and AIME 2026 avg@8 over
  30 problems: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:471`,
  `:473`.
- DocVQA uses official val 5,349 split first 2,700 evolving / 2,649 held-out,
  reporting ANLS and accuracy: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:479`.

Replication obligations:

- replicate SpreadsheetBench, WikiTQ conversion, DAPO-Math/AIME, DocVQA, seeds,
  official metrics, Anthropic `xlsx` and `xlsx-basic` baselines, model scales,
  vLLM config, and all ablations;
- record every trajectory, success/error analyst patch, validation result,
  merge tree, support count, discarded patch, and applied diff;
- keep the final skill as one portable skill directory, not a retrieval bank.

Leaven primitive blockers:

- `docs/working-memory/trace2skill-replication.md` now carries the detailed
  Trace2Skill dossier. Upstream code is locally available at
  `tmp/repros/trace2skill-upstream` and remote `main` was checked at
  `3d0b52a140f002a512930252b613c49048f7d5ac` on 2026-05-20.
- `leaven-agentic-skill::SkillPatchPlan` now covers the first paper-neutral
  patch-plan guardrail: existing-file checks, create-overwrite refusal,
  positive support counts, and same-file range conflict detection. This is not
  the hierarchical merge policy itself.
- `leaven-eval::RowOrderSplitBuilder` now covers source/paper manifests that
  define split membership by ordered row ranges, including Trace2Skill's
  SpreadsheetBench `0:200` evolving/training and `200:400` held-out split.
- `leaven-eval::DatasetSplitManifest` now covers paper-required split roles
  and split-use policy attachment. Paper examples should require their actual
  nonempty train/validation/test/search/probe roles here and avoid re-encoding
  those roles as example-local strings.
- `leaven-eval::Case::from_source_row` now covers row-stable case IDs plus
  upstream `source_id` / `source_row_index` metadata for source manifests such
  as Trace2Skill's mixed string/numeric SpreadsheetBench row IDs.
- `examples/trace2skill_spreadsheetbench` now parses the official local
  SpreadsheetBench-Verified 400-row `dataset.json` into Leaven eval cases and
  constructs a required-role split manifest for the paper's `0..200`
  train/evolving and `200..400` held-out test split. This is manifest-lowering
  provenance only, not spreadsheet execution or Trace2Skill evolution.
- `examples/trace2skill_spreadsheetbench` now also imports upstream-shaped
  SpreadsheetBench `results.json`, chat logs, and optional analysis reports
  into `AgentTrajectoryCorpusEvidence` for the paper's 200 training/evolving
  tasks. This is artifact import/provenance only, not live Qwen/vLLM execution.
- `examples/trace2skill_spreadsheetbench` now lowers upstream-shaped fenced or
  bare `PATCH_FORMAT=json` patches into `SkillParsedPatchDocument` operations,
  then into `SkillPatchPlan` plus concrete `SkillBankChange` values, and
  applies them through `SkillPatchApplication`. This is patch-bridge mechanics
  only, not evidence that live analyst or merge calls ran.
- `leaven-evidence::AgentTrajectoryEvidence` now preserves one-session
  trajectory provenance needed by Trace2Skill-style many-trace analysis:
  session id, optional Leaven case id, upstream task id, success/failure label,
  model id, model-config fingerprint, transcript/commands, and analyst records.
- `leaven-evidence::AgentTrajectoryCorpusEvidence` now preserves the
  checkpointable many-trajectory value over a caller-declared task manifest,
  with duplicate-manifest refusal, completed/pending projection, and
  unknown-task refusal. The paper runner still needs to write this corpus while
  generating actual SpreadsheetBench trajectories.
- `leaven-evidence::AgentAnalystFanoutEvidence` now preserves checkpointable
  Stage 2 analyst-call state: call manifest, analyst role, source task ids,
  prompt/response payloads, parse/backend status, retry count, and support
  count. The Trace2Skill example can seed source-template-backed pending
  analyst calls from imported training trajectories, but no live analyst/model
  execution or merge has run.
- `leaven-evidence::AgentPatchMergeTreeEvidence` now preserves checkpointable
  Stage 3 merge provenance: merge levels, input/accepted/discarded patch ids,
  support counts, decisions, prompt/response payloads, parse-failure artifacts,
  output patches, and optional final diff. It is evidence data only, not the
  Trace2Skill merge scheduler, prevalence policy, patch parser, or applier.
- `leaven-agentic-skill::SkillPatchMergeTree` now preserves hierarchical skill
  patch consolidation provenance over validated `SkillPatchPlan`s: leaf ids,
  merge levels, accepted/discarded inputs, output plans, and final plan identity.
- `leaven-agentic-skill::SkillParsedPatchDocument` now lowers already-parsed
  skill patch operations into a validated `SkillPatchPlan` plus concrete
  `SkillBankChange` values. This keeps paper JSON/markdown/diff syntax in thin
  paper examples while centralizing skill-file guardrails and artifact changes.
- `examples/trace2skill_spreadsheetbench` now replays saved/live upstream JSON
  patch merge artifacts through `SkillPatchMergeTree` and
  `SkillPatchApplication`: leaf and merge output JSON patches are lowered
  against the frozen parent skill bank, the upstream accepted/discarded inputs
  become merge-tree provenance, and the selected final patch emits an evolved
  `SkillBank` plus change report. This is artifact replay only; the paper
  runner still owns analyst execution, merge prompts, batch sizing, and
  prevalence policy.
- `examples/trace2skill_spreadsheetbench` now imports saved upstream
  `map_patches/patch_*.json` into pending Stage 2
  `AgentAnalystFanoutEvidence` by upstream `batch_index`, validating each saved
  patch against the parent skill bank and preserving prompt/source metadata. It
  also imports upstream MAP parse-failure markdown from
  `parse_failures_parallel/map/*_batch_000N_*_parse_failed.md` into terminal
  `ParseFailed` call records with artifact refs. Calls without saved parsed
  patches or saved parse failures remain pending; this does not execute
  analyst model calls.
- `examples/trace2skill_spreadsheetbench` now also loads the actual upstream
  `--save-intermediates` JSON directory shape: `map_patches`, numeric
  `merge_level_N` directories, `final_patch.json`, and optional
  `translated_final_patch.json`. It reconstructs merge batches from filename
  order plus `--merge-batch-size` and applies the translated final patch when
  present. This is still no-spend loader proof; no live analyst or merge call
  has run, and the upstream saved directory does not preserve explicit
  accepted/discarded decision rationale.
- `examples/trace2skill_spreadsheetbench` can import that same saved Stage 3
  JSON directory shape into `AgentPatchMergeTreeEvidence`, preserving map,
  merge, final, and translated-final nodes plus optional `applied_diffs.patch`
  after validating each saved JSON patch. It reconstructs accepted inputs from
  deterministic batch order and records the lost discarded-rationale limitation.
- `examples/trace2skill_spreadsheetbench` now has a one-case no-spend
  inspection/rendering surface for exact SpreadsheetBench case `13-1`: it reads
  `tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json`,
  the materialized init/golden workbooks and `prompt.txt`, the upstream
  `cli_skill_preloaded_full_system_v1.txt` system prompt, and the released
  `trace2skill-xlsx-35B-combined/SKILL.md`. This is an input preflight for a
  future one-sample live attempt; it does not execute a spreadsheet agent,
  modify a workbook, or run Qwen/vLLM.
- `examples/trace2skill_spreadsheetbench` now also has a no-spend workbook
  answer-range scorer for exact case `13-1`: `--compare-one-case-answer`
  compares a caller-supplied workbook's `LISTS!A3:D32` cells against
  `1_13-1_golden.xlsx`, reports matched/total cells, score, pass/fail, and
  mismatch cells. It is a scoring seam for a future live output workbook, not
  evidence that a spreadsheet agent has run.
- `examples/trace2skill_spreadsheetbench` now prepares a durable no-spend
  one-case run directory for case `13-1`: `--prepare-one-case-run --run-dir
  <tmp-run-dir>` stages the exact init/golden workbooks, writes an agent prompt
  with run-local input/output paths, writes `manifest.json`, and names
  `live_spreadsheet_agent_execution` as the first missing primitive. This is a
  live-attempt contract, not a solver or trajectory-generation proof.
- `examples/trace2skill_spreadsheetbench` now scores a prepared one-case run
  directory after a candidate output workbook and transcript exist:
  `--score-one-case-run` writes `score_report.json`, updates `manifest.json`,
  and emits `trajectory.json` as `AgentTrajectoryEvidence`. It is post-run
  scorer/evidence plumbing; the live spreadsheet agent remains the missing
  primitive.
- `examples/trace2skill_spreadsheetbench` now derives a source-anchored pending
  one-case Stage 2 analyst fan-out from a scored `trajectory.json`:
  `--prepare-one-case-analyst-fanout` writes `stage2_analyst_prompt.md` and
  `stage2_fanout.json`, embedding upstream `skill_evolver/prompts` template
  files and naming the upstream MAP prompt builders. It is fan-out staging only;
  no analyst model call, patch parsing, or merge has run.
- `examples/trace2skill_spreadsheetbench` now also uses that upstream
  `skill_evolver/prompts` material for corpus-wide pending Stage 2 fan-out from
  imported training trajectories instead of the earlier generic placeholder
  prompt scaffold. This keeps the 200-row path aligned with the paper's MAP
  prompt sources, but still stops before analyst execution, response parsing,
  or merge.
- `examples/trace2skill_tiny_live` now restores the tiny live
  trajectory-to-skill harness into main Leaven. Its preflight writes a no-spend
  proof contract, and its live mode runs Codex/GPT-5.4-mini over two CSV
  editing trajectories, independent analysts, consolidation, guarded skill
  update, and failed-task replay. This is a causal-loop proxy with documented
  deviations, not SpreadsheetBench/WikiTQ/DAPO/DocVQA replication.
- live trajectory generation that writes upstream-shaped results/logs/analysis
  artifacts for the 200 SpreadsheetBench training/evolving tasks;
- live parallel analyst dispatch that executes hundreds of independent
  sub-agent calls against the pending fan-out manifest;
- live or no-spend captured upstream MAP output directories that contain real
  parsed patches and parse-failure artifacts. The example can now import both
  into fan-out evidence, but the current proof is fixture-backed and does not
  show a real Trace2Skill analyst run completed locally;
- patch proposal artifact with conflict detection, format validation, support
  counts, and hierarchical merge tree. The first paper-neutral guardrail slice
  now exists as `SkillPatchPlan` in `leaven-agentic-skill`, covering file
  existence, create-overwrite, positive-support, same-file line-range conflict
  validation, and atomic `references/*.md` create/link validation; durable
  merge-tree provenance now exists as `AgentPatchMergeTreeEvidence` and
  `SkillPatchMergeTree`, while live merge execution remains;
  `SkillPatchApplication` now covers atomic application/reporting/rollback once
  a parsed plan and concrete `SkillBankChange` are available, and
  `SkillParsedPatchDocument` now covers the generic parsed-operation to
  plan/change lowering step; the Trace2Skill example now translates upstream
  JSON patches into those operations;
- live Trace2Skill merge-run integration that executes the model-backed merge
  scheduler and writes or imports real saved/live Stage 2/3 JSON artifacts
  consumed by the replay seam, ideally with translated exact map/merge patches
  and explicit merge decisions rather than only the default upstream saved
  directory shape. The saved-directory evidence importer now exists, but it can
  only reconstruct accepted inputs from output order when the upstream run did
  not save discard rationale;
- result matrix/reporting for cross-model and OOD transfer.

Spend/data risks:

- faithful vLLM replication needs large Qwen3.5 MoE serving and likely GPU
  spend. No live run until approved.

2026-05-20 no-spend probe:

- `docs/working-memory/trace2skill-replication.md` records the first
  Trace2Skill setup probe. The official repo
  `Qwen-Applications/Trace2Skill` is reachable at
  `3d0b52a140f002a512930252b613c49048f7d5ac` and was shallow-cloned under
  ignored `tmp/repros/trace2skill-upstream`. The release includes the
  SpreadsheetBench-Verified 400-row dataset, released spreadsheet skills,
  reproduction commands, Qwen generation configs, and the Python map-reduce
  evolver for spreadsheet runs. It does not locally solve WikiTQ conversion,
  DAPO/AIME, DocVQA, or Leaven-owned trajectory/merge-tree provenance.
- First Leaven primitive from this probe: `SkillPatchPlan` in
  `leaven-agentic-skill` validates the generic mechanical guardrails from
  Trace2Skill Stage 3 without importing paper-specific merge policy.
- The first paper-specific no-spend loader now exists as
  `examples/trace2skill_spreadsheetbench`; its manifest test verified 400 rows,
  first source id `13-1`, last source id `59902`, and the train/test row
  boundary.
- Its run-artifact test also verifies upstream-shaped run outputs can be
  lowered into the Leaven trajectory corpus with transcript and analysis blob
  refs.
- `leaven-agentic-skill::SkillPatchMergeTree` now covers the durable
  merge-provenance value needed by Trace2Skill Stage 2/3; it is still
  paper-neutral and does not encode Trace2Skill worker counts, thresholds, or
  prompts.
- The Trace2Skill example now covers upstream JSON patch lowering and atomic
  application from fenced LLM-style responses, including reference create/link
  pairing and exact-section refusal.
- The Trace2Skill example now covers upstream-shaped saved-intermediate
  directory replay, including deterministic map/merge filename ordering,
  `--merge-batch-size` reconstruction, `final_patch.json` merge selection, and
  `translated_final_patch.json` application.
- The Trace2Skill example now covers exact one-case input preflight for
  SpreadsheetBench case `13-1` through `--inspect-one-case` and
  `--render-one-case-prompt`, using the synchronized `tmp/` sample, upstream
  system prompt, and released combined `xlsx` skill.
- The Trace2Skill example now covers exact one-case workbook scoring for
  `13-1` through `--compare-one-case-answer`: golden-vs-golden scores
  `120/120`, while init-vs-golden scores `82/120` with 38 mismatches over
  `LISTS!A3:D32`.
- The Trace2Skill example now covers exact one-case run-directory preparation
  through `--prepare-one-case-run`, with durable files under
  `tmp/paper_exact_lane_runs/trace2skill/one_case_prepare_20260521T003128Z`
  and manifest status `blocked_missing_live_spreadsheet_agent`.
- The Trace2Skill example now covers prepared-run score/resume wiring through
  `--score-one-case-run`, with a no-spend golden-copy self-check under
  `tmp/paper_exact_lane_runs/trace2skill/one_case_score_selfcheck_20260521T005103Z`
  that wrote `score_report.json`, `trajectory.json`, and an updated manifest.
  The transcript explicitly records that this self-check is not a live
  spreadsheet-agent run.
- The Trace2Skill example now covers one-case pending Stage 2 fan-out staging
  through `--prepare-one-case-analyst-fanout` over that scored self-check run,
  writing `stage2_analyst_prompt.md`, `stage2_fanout.json`, and
  `stage2_fanout_report.json` with expected/pending call id `success-13-1-1`.
  The prompt records upstream prompt-template sources and explicitly says no
  analyst model call executed.
- The Trace2Skill example now covers source-anchored corpus pending Stage 2
  fan-out prompts through
  `stage2_corpus_fanout_embeds_upstream_prompt_sources`, proving success/error
  imported trajectories include upstream MAP prompt-template files rather than
  the previous placeholder scaffold.
- The Trace2Skill example now covers saved MAP-patch fan-out import through
  `imports_saved_map_patches_into_analyst_fanout_by_batch_index`, proving
  upstream `batch_index` maps parsed `map_patches/patch_*.json` back into
  pending fan-out calls as succeeded response blobs.
- The Trace2Skill example now covers saved Stage 3 merge-output import through
  `imports_saved_json_patch_merge_outputs_as_merge_tree_evidence`, proving
  upstream saved JSON patch outputs become `AgentPatchMergeTreeEvidence` nodes
  with final translated-patch and applied-diff artifact refs.
- Restored tiny live harness preflights passed in main Leaven on 2026-05-20:
  `tmp/memento_skills_read_write/20260520T204154Z/preflight.json`,
  `tmp/skillreducer_tiny/20260520T204154Z/preflight.json`,
  `tmp/d2skill_tiny/20260520T204154Z/preflight.json`, and
  `tmp/trace2skill_tiny_live/20260520T204154Z/preflight.json`. These are
  no-spend proof contracts only; they do not execute Codex or prove paper
  parity.

### Memento-Skills (`arx_2603.18743`)

Source anchors:

- Read-Write loop is Observe -> Read -> Act -> Feedback -> Write:
  `tmp/skill_opt_sources/arx_2603.18743/paperclip_content.lines:107`.
- Write is skill-level reflective update with failure attribution and file-level
  rewriting, not append-only memory:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:118`.
- behavior-aligned router is trained via single-step offline RL because BM25
  and semantic embeddings are insufficient:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:279`,
  `:366`.
- router data starts from about 8k local skills and about 3k sampled skills;
  public catalog is GitHub stars >500 with deterministic dedupe:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:370`,
  `:446`.
- router evaluation uses Qwen3-Embedding-0.6B, 140 synthetic queries, and
  Recall@1 0.32/0.54/0.60 for BM25/Qwen3/Memento-Qwen:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:456`,
  `:460`.
- GAIA uses 165 validation questions split 100 train / 65 test; HLE uses
  788 train / 342 test across 8 subjects:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:518`,
  `:522`.
- all experiments use Gemini-3.1-Flash:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:526`.
- GAIA allows up to three reflective retries and reports 66.0 test vs 52.3
  ablation; HLE reports 38.7 test vs 17.9 baseline:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:532`,
  `:550`.

Replication obligations:

- replicate skill catalog construction, router training data generation,
  contrastive/offline-RL router training, router metrics, GAIA/HLE splits,
  reflective retry schedule, static Read-Write ablation, and library growth;
- persist read/write decisions, utility, target selection, rewritten files,
  unit-test gates, retries, and rollback evidence.

Leaven primitive blockers:

- `examples/memento_skills_read_write` now restores the tiny live Read-Write
  harness from the paper lane into main Leaven. Its preflight writes a
  no-spend proof contract, and its live mode runs one Codex/GPT-5.4-mini case
  through Observe, Read, Act, Feedback, Write, unit-test gate, and retry. This
  is a causal-loop proxy with documented deviations, not GAIA/HLE replication.
- behavior-aligned skill router training/evaluation substrate;
- skill registry with routing goals, utility table, trigger stats, and
  skill-level failure attribution; `SkillUtilityState` now covers the base
  utility table and trigger/retrieval counters, but not router training,
  target selection, failure attribution, or candidate-visible registry
  materialization;
- write-path that targets one skill, rewrites files, validates, retries, and
  rolls back;
- multi-round retry execution with per-round feedback and learned-library state;
- GAIA/HLE harness adapters and exact split manifests.

Spend/data risks:

- HLE and GAIA access, Gemini-3.1-Flash availability, and router training
  compute need approval before live replication.

### D2Skill (`arx_2603.28716`)

Source anchors:

- dynamic dual-granularity skill bank has task and step skills and pairs
  skill-injected with baseline rollouts:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:29`,
  `:87`.
- each task samples `N` parallel trajectories split into skill and baseline
  groups of `N/2`: `tmp/skill_opt_sources/arx_2603.28716/full_source.md:106`.
- performance gap updates task and step skill utilities with EMA:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:119`,
  `:127`.
- reflection triggers below threshold, sampling one failed trajectory and when
  available one successful trajectory, producing at most one task and one step
  skill per group: `tmp/skill_opt_sources/arx_2603.28716/full_source.md:185`.
- retrieval uses task keys `g`, step keys `(g, o_j)`, similarity threshold,
  normalized similarity plus utility/UCB ranking, and top-k injection:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:194`,
  `:200`, `:202`, `:208`.
- experiments are ALFWorld/WebShop with Qwen2.5-7B-Instruct and
  Qwen3-4B-Instruct-2507; validation fixes the skill bank with no reflection:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:223`,
  `:241`.
- reported training cost is 8xH100: GRPO 20.8h, D2Skill 25.6h, SkillRL 49.2h:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:289`.

Replication obligations:

- replicate ALFWorld/WebShop environments, RL stack, model initializations,
  teacher-SFT setup, skill/baseline paired rollout grouping, GRPO integration,
  validation freeze, ablations, and cost curves;
- preserve task/step skill separation, retrieval keys, utility, retrieval
  counts, pruning, and reflection thresholds.

Leaven primitive blockers:

- `examples/d2skill_tiny` now restores the tiny live paired-rollout harness
  into main Leaven. Its preflight writes a no-spend proof contract, and its
  live mode runs one Codex/GPT-5.4-mini textual task through baseline vs
  skill-injected rollouts, reflection into task/step skills, retrieval, utility
  update, and pruning. This is a causal-loop proxy with documented deviations,
  not ALFWorld/WebShop/GRPO replication.
- dual-pool skill registry for task and step skills; `SkillRouteRegistry` now
  covers explicit pool/key membership over a validated `SkillBank`, while real
  D2Skill key construction from task IDs and observations remains paper-runner
  work;
- paired rollout evaluator that records baseline-vs-skill deltas;
  `PairedRolloutEvidence` now records non-empty baseline/treatment group
  rewards and exposes treatment-minus-baseline as a finite task-level signal.
- utility/EMA/UCB retrieval and pruning state; `SkillUtilityState` now covers
  EMA utility plus retrieval/trigger counters, and `SkillUtilityRanker` now
  covers utility/UCB-aware deterministic top-k ranking over caller-provided
  relevance scores. `SkillPairedRolloutUtilityInput` now maps paired rollout
  task gaps and caller-supplied step credits onto validated skill utility
  updates, and can derive step credits from runner-provided
  `SkillStepTrajectoryOutcome`s or generic `SkillTrajectoryUseEvidence` using
  D2Skill's `Y_i - baseline_mean` equation. `SkillTwoStageRetriever` now covers
  pool-scoped similarity
  threshold/top-m retrieval plus utility/UCB top-k selection over
  caller-computed similarities. `SkillUtilityPruner` now covers utility/UCB
  eviction scoring, capacity planning, and protected-window exclusion over
  caller-supplied active pools. Together with `SkillRouteRegistry`, it now
  covers validated pool/key membership plus utility/retrieval/pruning
  bookkeeping. It still does not cover embedding model execution, route-key
  extraction, transcript/env event extraction, injection formatting, validation
  cadence, paper-provided capacity/protected-window values, or skill-bank
  mutation;
- `SkillTrajectoryUseEvidence` now records retrieved/injected/triggered skill
  events for one rewarded trajectory, and `SkillPairedRolloutUtilityInput` can
  consume it for step credits. The actual ALFWorld/WebShop transcript/env parser
  that emits those events remains paper-runner work;
- RL/environment adapter boundary that can record Leaven evidence without
  turning Leaven into the RL framework.

Spend/data risks:

- full 1:1 replication is HPC-heavy and should sit behind explicit approval.
  This paper should probably follow EvoSkill/Trace2Skill/Memento unless the
  user wants to fund the RL run early.

### SkillReducer (`arx_2603.29919`)

Source anchors:

- skill shape is description, body, references, scripts, with scripts out of
  scope: `tmp/skill_opt_sources/arx_2603.29919/full_source.md:31`.
- empirical corpus is 55,315 Wild skills, 100 SkillHub, and 620 Community:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:47`.
- Stage 1 uses ddmin over semantic clauses with simulated routing oracle, four
  TF-IDF distractors, one adversarial skill, and real Claude Code trigger
  validation with selective restore:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:108`,
  `:110`, `:112`, `:116`, `:118`.
- missing/short descriptions are generated from body signals:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:132`.
- Stage 2 classifies core/background/example/template/redundant content,
  dedupes references, annotates references, runs Gate 1 faithfulness, Gate 2
  task evaluation, and promotes failed items to core up to two iterations:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:149`,
  `:161`, `:170`, `:194`.
- evaluation uses 600 skills plus SkillsBench; models include DeepSeek-V3,
  DeepSeek-R1, Claude Code CLI, Qwen3.5, and `cl100k_base`:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:204`;
  `tmp/skill_opt_sources/arx_2603.29919/all_text_sources.md:678`,
  `:680`.
- results include 48 percent description compression, 39 percent body
  compression, 86.0 percent pass rate, 536/536 routing preservation, and 87/87
  SkillsBench pass:
  `tmp/skill_opt_sources/arx_2603.29919/all_text_sources.md:720`,
  `:768`, `:793`, `:813`, `:816`.

Replication obligations:

- replicate crawl/filter/dedupe, corpus samples, ddmin oracle, real-trigger
  protocol, description generation, body taxonomy, reference module generation,
  gates, feedback loop, baselines, statistical tests, cross-model transfer, and
  SkillsBench evaluation;
- separate compression and evaluation models to prevent leakage;
- emit token accounting and per-skill before/after artifacts.

Leaven primitive blockers:

- `examples/skillreducer_tiny` now restores the tiny live debloating harness
  into main Leaven. Its preflight writes a no-spend proof contract, and its
  live mode runs one Codex/GPT-5.4-mini skill through compressed routing
  description selection, real-trigger validation, body taxonomy, progressive
  disclosure, faithfulness gate, Condition A/C evaluation, and feedback
  promotion. This is a causal-loop proxy with documented deviations, not the
  600-skill/SkillsBench paper evaluation.
- structured description/body/reference surfaces over `SkillBank` now have the
  first Leaven-owned artifact projections: manifest description/frontmatter,
  `SKILL.md` body, and direct `references/*.md` modules. Remaining SkillReducer
  surface blockers are core/background/example/template classification views,
  progressive-disclosure routing metadata, and paper-specific token-cost
  annotations;
- tokenizer-agnostic skill token profiling now exists in
  `leaven-artifact-skill`: descriptions and `SKILL.md` bodies are counted as
  always-loaded context, direct `references/*.md` modules are counted as
  progressive-disclosure context, scripts and non-markdown files are excluded,
  non-UTF-8 markdown references are refused, and before/after comparisons
  require matching tokenizer ids. Remaining SkillReducer token blockers are an
  exact `cl100k_base` tokenizer adapter, paper cost curves, route-trigger
  probabilities, and report/dossier integration;
- route-equivalence oracle abstraction plus provider-specific real-trigger
  event parser;
- ddmin/minimization primitive over semantic units with restore policy;
- exact SkillReducer tokenizer adapter plus token-cost model over paper runs;
- task-generation/evaluation dossier with deterministic verifier support and
  LLM judge separation.

Spend/data risks:

- public skill crawling and multi-model evaluation are moderate but still need
  explicit spend/account approval for provider-backed runs.

## Cross-Paper Primitive Backlog

Start with these generic primitives as failures expose them:

1. Git program/frontier snapshots: pure branch/tag identity and lineage
   vocabulary plus local clone/capture/restore/delete primitives exist;
   checkpointed EvoSkill run integration remains.
2. `leaven-population`: top-k scalar frontier admission and deterministic
   best/round-robin parent selector state exist; P5 checkpoint/resume carries
   that state, while full EvoSkill OfficeQA/SealQA run integration remains.
3. Skill registry/card layer: base derived `SkillCard` now exists in
   `leaven-artifact-skill` as the manifest-only catalog view over validated
   skill folders. `SkillRouteRegistry` now exists as an explicit route
   pool/key overlay over a validated `SkillBank`. Utility, trigger stats,
   retrieved-use evidence, similarity scores, and lifecycle state remain
   outside raw `SkillBank` and the route registry.
4. Evidence/trace corpus: trajectories, success/failure labels, feedback
   history, patch/proposal provenance, support counts, and cost records.
5. Agentic batch orchestration: many independent analysts/proposers with
   checkpointed fan-out/fan-in and resumable merge trees.
6. Split/sampler/metric adapters: category-aware without-replacement sampler
   state, exact stratified split construction, and exact caller-declared split
   manifest lowering exist in `leaven-eval`; P5 threads sampler state through
   checkpoints and now has optional EvoSkill OfficeQA/SealQA source-id split
   manifest ingestion that hard-fails bad membership before it can become a
   proof. Official metric wrappers, held-out report manifests, and the missing
   paper source manifests for EvoSkill remain.
7. Skill optimization surfaces: manifest description/frontmatter, `SKILL.md`
   body, and direct `references/*.md` module surfaces exist in
   `leaven-artifact-skill`; tokenizer-agnostic token profiles now account for
   description/body/direct-reference context under a caller-provided tokenizer.
   Core/background/example/template item views, progressive-disclosure routing
   metadata, exact tokenizer adapters, paper cost curves, and run-level token
   reporting remain.
8. Route equivalence and trigger evidence: simulated route oracle plus real
   provider/runtime trigger parser.
9. Utility/retrieval/pruning substrate: `SkillUtilityState` now covers finite
   EMA utility plus retrieval/trigger counters, `SkillUtilityRanker` covers
   deterministic utility/UCB top-k ranking over caller-provided relevance
   scores, `SkillTwoStageRetriever` covers D2Skill-style pool-scoped
   similarity threshold/top-m plus utility/UCB top-k selection, and
   `SkillPairedRolloutUtilityInput` covers D2Skill-style task-gap/step-credit
   application from paired rollout evidence, including the step trajectory
   `Y_i - baseline_mean` credit equation over `SkillStepTrajectoryOutcome`s or
   `SkillTrajectoryUseEvidence`.
   `SkillUtilityPruner` covers D2Skill-style utility/UCB eviction scoring with
   capacity and protected-window planning. `SkillRouteRegistry` covers explicit
   pool/key membership for routed skill cards. Embedding model execution,
   route-key extraction, transcript/env event extraction, injection formatting,
   paper cadence/threshold values, and skill-bank mutation remain.
10. Paired rollout evidence: `PairedRolloutEvidence` now covers
    baseline-vs-treatment group rewards and finite treatment-minus-baseline
    deltas. Hindsight trajectory parsing and real ALFWorld/WebShop rollout
    capture remain.

## Current Priority

The next coherent work slice is EvoSkill, not because it is simplest, but
because it already has Leaven substrate and immediately pressures the most
foundational missing primitive: git-backed program identity and frontier
lineage. The first retry should stay no-spend:

- create `examples/p5_evoskill_iteration` successor dossier for full
  OfficeQA/SealQA replication;
- verify data availability and exact split manifests;
- reproduce scorer/sampler/frontier semantics with fixtures;
- implement the first generic primitive only when the replica cannot proceed
  without it;
  - keep paper-specific code thin enough that replacing the fixture with the
  paper data does not rewrite Leaven behavior.

## EvoSkill Paper-Close Guardrails

The P5 EvoSkill final report schema v19 carries a no-spend loop `run_manifest`
that ties the mechanics loop to manifest/scorer/source/split fingerprints,
frontier policy, schedule, checkpoint boundary, Git identity mode, the full
OfficeQA validation role/fingerprint used before frontier admission, and the
fake validation-score source. Score slots also carry paper target ids,
including BrowseComp transfer target slots. Reported score slots preserve the
importing sidecar entry as `score_evidence_id`, the checked scoring method as
`score_evidence_kind`, any required judge approval id as
`score_evidence_approval_id`, and the checked evidence artifact path/hash/byte
count as `score_evidence_artifact`; unreported slots keep those fields null.
The report now also carries first-class `exactness_gaps`: local source pins are
`paper_release_unverified`, accepted substitute splits are
`accepted_paper_close_substitute`, and unresolved source artifacts are
`blocked_before_paper_close`, with evidence handles for local heads, split
fingerprints, and role source-id fingerprints. The
materialization path can replace the OfficeQA and SealQA
substitute split blockers either when an exact source-id
split manifest validates against the materialized row universe or when the
split policy sidecar validates and explicitly accepts the current documented
paper-close substitute fingerprints. Accepted substitutes stay
`paper_close_substitute`, not paper-exact. A valid strict 128-row BrowseComp
transfer JSONL sidecar materializes only an unscored held-out transfer
denominator; if absent or malformed, the BrowseComp source blocker stays or the
manifest build fails. The loop must use the same materialized source set as the
embedded report manifest and refuse source/split/role drift. Treat that as
mechanics and denominator evidence only. It is not live provider,
validation-score quality, SealQA judge, transferred-skill execution, or
paper-score evidence.

2026-05-22 update: P5 can now derive that BrowseComp JSONL sidecar from a local
copy of the official simple-evals encrypted BrowseComp CSV with
`just evoskill-paper-browsecomp-public-sample <csv>`. The Rust path decrypts
the public rows, selects a deterministic topic-stratified 128-row substitute,
and keeps all transfer slots unscored. With source pins and substitute split
policy sidecars present, source blockers can reach zero; this is still a
paper-close substitute denominator, not the paper author's exact BrowseComp
sample and not transferred-skill score evidence. The report preserves that
distinction by moving top-level exactness only to `paper_close_candidate` and by
switching denominator-ready ablation lanes to `approval_blocked` rather than
leaving stale absent-source notes attached after the sidecar materializes.

2026-05-22 update: P5 can now ingest an optional strict
`tmp/replication/evoskill/score_result_manifest.json` sidecar. It is score
evidence plumbing only: schema-v5 entries must match the current manifest
fingerprint, scorer fingerprint, slot key, split fingerprint, role source-id
fingerprint, and row count before a score is reported. The checked evidence
artifact is now strict JSONL: each row carries `source_id`, `prediction`, and
`score`; the importer verifies exact role membership, rejects duplicate or
missing rows, requires finite `[0, 1]` row scores, recomputes the aggregate,
replays the OfficeQA Rust scorer against materialized scorer-only targets before
importing OfficeQA scores, and rechecks BrowseComp transfer rows with a
conservative exact-normalized answer scorer against materialized scorer-only
targets. The BrowseComp check rejects fabricated row scores when exact answers
are present, but it is not the official simple-evals judge path. External judge
rows must carry the pinned judge-template fingerprint, so approval metadata
cannot bind scores to an unspecified prompt. Stale result
files, duplicate entries, unresolved slot blockers, non-slot blocker claims,
tampered evidence artifacts, fabricated aggregates, OfficeQA predictions whose
row scores do not match the scorer, and BrowseComp row scores that fail the
exact-answer check fail the report build. Reported scores preserve the sidecar
`evidence_id` and checked artifact in the score slot, so future approved runs
leave an auditable score-evidence handle in the final report without treating
fixture values, stale outputs, tampered files, exact-answer checks, or missing
approval as paper scores. This still does not execute the live provider, SealQA
judge, official BrowseComp judge, transferred BrowseComp skill, or any missing
paper score path.

2026-05-22 update: score result sidecars are now schema-v5. Each entry must
declare `score_evidence_kind`, and the final score slot preserves it. OfficeQA
requires `rust_scorer_replay`; BrowseComp can use the current conservative
`exact_answer_replay` or a future approved `external_judge_run`; SealQA requires
`external_judge_run`. External judge entries require a nonempty
`score_evidence_approval_id`, reported LLM calls covering judged rows, and
row-level `judge_template_fingerprint` values matching the pinned scorer
template for that dataset. This does not run a judge or approve spend; it
prevents opaque row scores from masquerading as deterministic replay evidence or
as outputs from a different judge prompt, and gives approved judge outputs a
typed import lane.

2026-05-22 update: score result sidecars can no longer clear source or split
provenance blockers. The importer rejects entries that claim to resolve
blockers such as `officeqa_category_split_manifest`,
`officeqa_exact_split_membership`, or `sealqa_split_manifest`; those remain the
responsibility of source pin, exact split, or accepted substitute split policy
manifests. Today the only score-resolvable blocker is
`sealqa_judge_scored_run`, and only for approved SealQA `external_judge_run`
evidence with matching judge-template fingerprints. This closes the
hand-authored-sidecar path where a row-score artifact could otherwise stand in
for missing denominator proof.

2026-05-22 update: final report blockers are now evidence-sensitive after score
sidecar import. Partial SealQA judge score imports report only their slots and
leave `sealqa_judge_scored_run` on errors, ablations, and the `paper_scorer`
gate. Complete approved `external_judge_run` evidence for every SealQA score
slot clears that blocker and can prove the scorer gate, while the separate
`live_run_spend_approval` gate remains blocked until an approved bounded live
agent run exists.

2026-05-22 update: SealQA approved judge outputs now have a Rust operator
writer. `just evoskill-paper-score-sealqa <judged_rows.jsonl> <approval_id>`
consumes strict SealQA JSONL rows keyed by dataset/split role/candidate/source
id with prediction, score, and the row-level judge-template fingerprint used by
the run. The writer requires a nonempty approval id, current pinned
judge-template fingerprints, exact role coverage, finite `[0, 1]` scores, and
slots whose only blocker is `sealqa_judge_scored_run`; it writes checked score
evidence JSONL and the schema-v5 score result manifest through the normal
importer. This is an import lane only: it does not run or approve a judge,
resolve source/split blockers, or make partial SealQA rows prove the whole
scorer gate.

2026-05-22 update: score result writers now accumulate disjoint score-slot
batches instead of silently replacing the sidecar. The OfficeQA scorer replay
writer and approved SealQA judge import writer first validate the existing
`score_result_manifest.json`, refuse requested slot keys that are already
reported, then merge new entries and summed costs through schema v5 before the
normal importer runs. This keeps replacement explicit while letting OfficeQA
and SealQA score evidence coexist in the final report.
