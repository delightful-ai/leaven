# Skill Paper Replication Ledger

Status: active.
Updated: 2026-05-20.

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
  skill validation, metadata, and skill surfaces.
- `leaven-agentic-skill` owns skill-bank materialization, workspace readback,
  diffs, and proposal parsing for agent-authored skill changes.
- `leaven-agentic` has proposer repair policy support.
- `leaven-gepa-agentic-skill` bridges GEPA reflection to skill-bank proposal.
- `examples/p5_evoskill_iteration` proves one live Codex EvoSkill-shaped
  iteration over a tiny treasury-notation fixture.
- `leaven-population` has `ParetoFrontier`, `TopKFrontier`, and deterministic
  `TopKParentSelector` state for best/round-robin parent selection; EvoSkill
  P5 now checkpoints frontier/parent/selector state, but full paper replication
  still needs git-program run-loop integration and dossier resolution of the
  paper/code selector drift.

Current known gaps:

- `crates/leaven-artifact-git` now models normalized paths, immutable object
  IDs, branch/tag refs, typed lineage, content/cache identity, and ref removal
  for discarded candidates. `crates/leaven-workspace-git` can clone and check
  out a local `program/*` branch, capture tracked files and branch/tag refs
  into `GitArtifact`, restore a selected ref, and delete branch/tag refs.
  EvoSkill still needs run-loop integration for checkpointed program/frontier
  state and paper score metadata.
- The P5 EvoSkill path is not OfficeQA or SealQA. It lacks the paper datasets,
  paper splits, full frontier loop, feedback-history schedule, held-out test
  reporting, skill-merge evaluation, paper scorers, and ablations.
- Trace2Skill now has a mechanics-smoke crate for SpreadsheetBench manifest
  lowering only. Memento-Skills, D2Skill, and SkillReducer have no replica
  crates yet.
- No per-paper dossier currently records exact prompts, seeds, model versions,
  splits, metrics, released-code drift, and cost envelope.

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
  over caller-supplied strata. Exact paper split manifests remain; P5 now
  threads sampler state through its private checkpoint and samples explicit
  feedback case IDs from train pools;
- paper-grade scorer/evaluator records with failure-threshold extraction;
- evidence history that can feed proposer prompts without paper-side custom
  plumbing.

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
- `leaven-eval::Case::from_source_row` now covers row-stable case IDs plus
  upstream `source_id` / `source_row_index` metadata for source manifests such
  as Trace2Skill's mixed string/numeric SpreadsheetBench row IDs.
- `examples/trace2skill_spreadsheetbench` now parses the official local
  SpreadsheetBench-Verified 400-row `dataset.json` into Leaven eval cases and
  constructs the paper's `0..200` train/evolving and `200..400` held-out split.
  This is manifest-lowering provenance only, not spreadsheet execution or
  Trace2Skill evolution.
- `examples/trace2skill_spreadsheetbench` now also imports upstream-shaped
  SpreadsheetBench `results.json`, chat logs, and optional analysis reports
  into `AgentTrajectoryCorpusEvidence` for the paper's 200 training/evolving
  tasks. This is artifact import/provenance only, not live Qwen/vLLM execution.
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
  count. The Trace2Skill example can seed pending analyst calls from imported
  training trajectories, but no live analyst/model execution or merge has run.
- `leaven-evidence::AgentPatchMergeTreeEvidence` now preserves checkpointable
  Stage 3 merge provenance: merge levels, input/accepted/discarded patch ids,
  support counts, decisions, prompt/response payloads, parse-failure artifacts,
  output patches, and optional final diff. It is evidence data only, not the
  Trace2Skill merge scheduler, prevalence policy, patch parser, or applier.
- `leaven-agentic-skill::SkillPatchMergeTree` now preserves hierarchical skill
  patch consolidation provenance over validated `SkillPatchPlan`s: leaf ids,
  merge levels, accepted/discarded inputs, output plans, and final plan identity.
- live trajectory generation that writes upstream-shaped results/logs/analysis
  artifacts for the 200 SpreadsheetBench training/evolving tasks;
- live parallel analyst dispatch that executes hundreds of independent
  sub-agent calls against the pending fan-out manifest;
- patch proposal artifact with conflict detection, format validation, support
  counts, and hierarchical merge tree. The first paper-neutral guardrail slice
  now exists as `SkillPatchPlan` in `leaven-agentic-skill`, covering file
  existence, create-overwrite, positive-support, same-file line-range conflict
  validation, and atomic `references/*.md` create/link validation; durable
  merge-tree provenance now exists as `AgentPatchMergeTreeEvidence` and
  `SkillPatchMergeTree`, while live merge execution, patch parsing, and
  skill-directory application remain;
- deterministic skill-directory diff application and rollback;
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

- behavior-aligned skill router training/evaluation substrate;
- skill registry with routing goals, utility table, trigger stats, and
  skill-level failure attribution;
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

- dual-pool skill registry for task and step skills;
- paired rollout evaluator that records baseline-vs-skill deltas;
- utility/EMA/UCB retrieval and pruning state;
- trajectory-level credit assignment tied to retrieved skills;
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

- structured description/body/reference surfaces over `SkillBank`;
- route-equivalence oracle abstraction plus provider-specific real-trigger
  event parser;
- ddmin/minimization primitive over semantic units with restore policy;
- skill token-cost model and tokenizer adapter;
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
3. Skill registry/card layer: derived `SkillCard` plus utility, routing keys,
   trigger stats, retrieved-use evidence, and lifecycle state outside raw
   `SkillBank`.
4. Evidence/trace corpus: trajectories, success/failure labels, feedback
   history, patch/proposal provenance, support counts, and cost records.
5. Agentic batch orchestration: many independent analysts/proposers with
   checkpointed fan-out/fan-in and resumable merge trees.
6. Split/sampler/metric adapters: category-aware without-replacement sampler
   state plus exact stratified split construction exist in `leaven-eval`; P5
   threads sampler state through checkpoints. Official metric wrappers,
   held-out report manifests, and paper-supplied split manifests remain.
7. Skill optimization surfaces: description/body/reference/core/progressive
   disclosure views plus token-cost accounting.
8. Route equivalence and trigger evidence: simulated route oracle plus real
   provider/runtime trigger parser.
9. Utility/retrieval/pruning substrate: embedding keys, similarity thresholds,
   EMA utility, UCB bonus, top-k injection, and eviction.
10. Paired rollout evidence: baseline-vs-skill groups, hindsight credit, and
    per-retrieved-skill utility update inputs.

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
