# Trace2Skill Replication Dossier

Status: partial, blocked before faithful live replication.

## Scope

Paper/source bundle: `tmp/skill_opt_sources/arx_2603.25158`.

Upstream code: `https://github.com/Qwen-Applications/Trace2Skill`.

Local upstream checkouts:

- `tmp/repros/trace2skill-upstream`
- `/Users/darin/vendor/github.com/Qwen-Applications/Trace2Skill`

Remote state checked 2026-05-20:

- `git ls-remote --symref https://github.com/Qwen-Applications/Trace2Skill HEAD refs/heads/main`
  returned `refs/heads/main` at
  `3d0b52a140f002a512930252b613c49048f7d5ac`.
- both local upstream checkouts are at
  `3d0b52a140f002a512930252b613c49048f7d5ac`.

## Paper Anchors

- Pipeline stages are trajectory generation, parallel multi-agent patch
  proposal, and conflict-free hierarchical consolidation:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:19`.
- Analysts are dispatched concurrently with no sequential dependency:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:99`.
- Consolidation identifies prevalent patterns and discards idiosyncratic
  patches: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:129`.
- SpreadsheetBench-Verified uses 400 samples split 200 evolving / 200 held-out,
  OOD WikiTQ converted to spreadsheet format, averaged over seeds 41/42/43:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:149`.
- Qwen3.5-122B-A10B and Qwen3.5-35B-A3B are served through vLLM; Stage 1 uses
  one trajectory per problem, Stage 2 uses 128 sub-agents, merge batch size 32,
  and ReAct turn budget 100:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159`.
- DocVQA uses official validation split 5,349 as first 2,700 evolving and
  remaining 2,649 held-out:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:479`.
- Parallel runtime comparison reports W=128, N≈70 error lessons, ≈7 merge
  layers, and 3 min vs 60/15 min baselines on an 8-GPU A800 node:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:599`.
- Released hierarchy routes low-support observations into 13 supplementary
  reference files rather than main `SKILL.md`:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:844`.

## Upstream Assets

The checked-out upstream release includes:

- SpreadsheetBench-Verified data:
  `tmp/repros/trace2skill-upstream/data/spreadsheetbench_verified/spreadsheetbench_verified_400/dataset.json`.
  A Ruby JSON probe found an array with 400 rows and keys
  `id,instruction,spreadsheet_path,instruction_type,answer_position,answer_sheet,data_position`.
- released paper skills under `released_skills/`, including
  `trace2skill-xlsx-35B-combined`, `xlsx-35B`,
  `trace2skill-xlsx-122B-combined`, and `xlsx-122B`:
  `tmp/repros/trace2skill-upstream/README.md:55`.
- run/evaluation/analysis/evolution entrypoints:
  `tmp/repros/trace2skill-upstream/README.md:68`.
- reproduction variables for spreadsheet runs:
  `DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400`,
  `MODEL=Qwen3.5-122B-A10B`, `WORKERS=128`, `SEED=41`, and Qwen generation
  configs: `tmp/repros/trace2skill-upstream/README.md:109`.
- held-out `cli_only` baseline over rows `200:400`:
  `tmp/repros/trace2skill-upstream/README.md:122`.
- training/evolving split run over rows `0:200`, `max_turns 100`, `workers
  128`, and skill-preloaded agent:
  `tmp/repros/trace2skill-upstream/README.md:148`.
- evolution from parsed training-split error records with `--batch-size 1`,
  `--max-workers "$WORKERS"`, `--save-intermediates`, parse-failure directory,
  and patch pipeline selection:
  `tmp/repros/trace2skill-upstream/README.md:206`.
- training-set validation over `0:200` before held-out evaluation and seed
  selection across three seeds:
  `tmp/repros/trace2skill-upstream/README.md:235`.
- held-out evaluation over `200:400` after selecting the best seed:
  `tmp/repros/trace2skill-upstream/README.md:285`.

## Upstream Code Shape

- `skill_evolver/run_parallel_skill_evolution.py` describes the map-reduce
  pipeline: independent error-record batches propose patches against a frozen
  original skill, patches merge hierarchically, and the final patch is applied:
  `tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_skill_evolution.py:5`.
- It derives task ids from the dataset with runner semantics and optional
  start/end/shuffle/sample controls:
  `tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_skill_evolution.py:125`.
- It exposes merge batch size, max workers, max merge levels, intermediate
  artifact saving, parse-failure artifacts, changelog, cumulative patch file,
  JSON/markdown patch pipelines, and semantic patch formatting:
  `tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_skill_evolution.py:288`,
  `:380`, `:408`, `:420`.
- It writes summary fields for map patches, estimated LLM calls, edits applied,
  diffs, cumulative patch, and changelog:
  `tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_skill_evolution.py:672`.
- `skill_evolver/run_parallel_combined_skill_evolution.py` owns combined
  error+success inputs and the same patch/consolidation controls:
  `tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_combined_skill_evolution.py:1`,
  `:96`, `:168`, `:321`.

## Leaven-Side Progress

- `leaven-eval::StratifiedSplitBuilder` already owns exact disjoint split
  construction from caller-supplied strata and role counts. That covers lowered
  representation for Trace2Skill's `0:200` evolving/training and `200:400`
  held-out split once the upstream row-order manifest is lowered.
- `leaven-eval::RowOrderSplitBuilder` now owns the row-order split lowering
  needed for Trace2Skill's SpreadsheetBench release: pass the ordered 400
  upstream case IDs, assign `Train` to `0..200` and `Test` to `200..400`, and
  get a disjoint `DatasetSplits` with the paper/source row-order semantics
  preserved. This does not parse SpreadsheetBench JSON or run the benchmark.
- `leaven-eval::DatasetSplitManifest` now owns the required-role contract for
  paper splits: replicas can require nonempty `Train`, `Validation`, `Test`,
  or custom roles and attach the split-use policy without hard-coded partition
  strings in paper crates. This is split metadata only; it does not execute or
  enforce engine trust policy.
- `leaven-eval::Case::from_source_row` now owns the generic source-row case
  lowering needed for the 400-row release: Leaven case IDs are row-stable
  `CaseId::from_index(row)`, while upstream ids such as `13-1` and `59902` are
  preserved in metadata as `source_id`, with `source_row_index` also recorded.
  SpreadsheetBench JSON parsing remains paper-specific loader work.
- `examples/trace2skill_spreadsheetbench` now owns the first no-spend
  paper-specific loader pass: it parses the official local
  SpreadsheetBench-Verified 400-row `dataset.json`, lowers rows into
  `leaven-eval::Case<SpreadsheetBenchTask, SpreadsheetBenchAnswerSpec>` with
  source-row metadata, and builds a `DatasetSplitManifest` requiring the
  paper's `0..200` train/evolving and `200..400` held-out test split. This is a
  mechanics-smoke for manifest provenance, not spreadsheet execution or skill
  evolution.
- The same example now imports upstream run artifacts into Leaven trajectory
  evidence without launching model work: `results.json` rows from
  `run_spreadsheetbench.py`, chat-history logs named
  `<agent>_<instance_id>.md/jsonl`, and optional `error_analysis_*.md` /
  `success_analysis_*.md` reports lower into
  `AgentTrajectoryCorpusEvidence` for the 200 train/evolving tasks.
- This slice adds `leaven-agentic-skill::SkillPatchPlan`, a paper-neutral
  mechanical guardrail for agent-authored skill patch plans. It validates
  existing-file modifications/deletions, create-file overwrite refusal,
  positive support counts, same-file range conflicts, and atomic pairing between
  new `references/*.md` files and `SKILL.md` links. It intentionally does not
  own Trace2Skill merge policy, prevalence thresholds, prompt wording, batch
  sizes, or result selection.
- `leaven-agentic-skill::SkillPatchMergeTree` now owns paper-neutral
  hierarchical consolidation provenance over validated patch plans: leaf plan
  ids, merge levels, accepted/discarded inputs, output plans, and selected final
  plan identity. It refuses malformed merge graphs but deliberately leaves
  prevalence thresholds, merge prompts, batch sizing, and final metric selection
  to the paper runner.
- `leaven-agentic-skill::SkillPatchApplication` now owns paper-neutral
  application of validated patch plans to `SkillBank` artifacts: it applies the
  concrete `SkillBankChange` atomically, reports the resulting skill/file delta,
  and returns rollback evidence carrying the parent bank, plan, attempted
  change, and error when application fails. It does not parse Trace2Skill patch
  JSON, schedule merges, or choose prevalence thresholds.
- `leaven-agentic-skill::SkillParsedPatchDocument` now owns the paper-neutral
  bridge from already-parsed patch operations to a validated `SkillPatchPlan`
  plus concrete `SkillBankChange` values. Trace2Skill still must translate its
  Stage 2/3 JSON, semantic markdown, or unified diff artifacts into those
  full-file write/delete operations in the paper example; the generic adapter
  now owns the mechanical validation/lowering boundary after translation.
- `examples/trace2skill_spreadsheetbench` now owns the paper-specific bridge
  from upstream `PATCH_FORMAT=json` artifacts into Leaven skill primitives:
  fenced or bare JSON patches lower into `SkillParsedPatchDocument` operations,
  then into a validated `SkillPatchPlan` plus concrete `SkillBankChange` values
  before applying through `SkillPatchApplication`. The bridge preserves the
  upstream operations (`append_to_section`, `replace_in_section`,
  `insert_after`, `insert_before`, `add_section`, `delete_section`, `create`,
  `delete_file`) but requires translated exact text targets rather than silently
  claiming a skipped edit was applied.
- `examples/trace2skill_spreadsheetbench` now replays saved/live Stage 2/3 JSON
  patch merge artifacts through Leaven primitives: leaf and merge output patches
  lower against the frozen parent skill bank, upstream accepted/discarded input
  decisions populate `SkillPatchMergeTree`, and the selected final patch applies
  atomically to emit an evolved `SkillBank` plus `SkillBankChangeReport`. This
  is still no-spend artifact replay; it does not run the model-backed analyst
  or merge scheduler.
- `examples/trace2skill_spreadsheetbench` now also loads upstream
  `--save-intermediates` JSON output directories directly: `map_patches`,
  numeric `merge_level_N` directories, `final_patch.json`, and optional
  `translated_final_patch.json`. The loader reconstructs deterministic merge
  batches from saved filename order plus `--merge-batch-size`, records all
  reconstructed inputs as accepted because upstream saved directories do not
  preserve explicit accepted/discarded rationale, and applies
  `translated_final_patch.json` when present because upstream uses that artifact
  after translation. It is strict: fuzzy pre-translation patches still fail with
  the artifact path rather than being silently counted as applied.
- `examples/trace2skill_spreadsheetbench` now owns a one-case no-spend
  inspection/rendering preflight for the materialized SpreadsheetBench-Verified
  case `13-1`. The library reports the exact case metadata, init workbook,
  golden workbook, prompt, upstream system prompt, released combined `xlsx`
  skill, deterministic output path, and exact answer-range comparison for a
  supplied candidate workbook; the binary exposes this as `--inspect-one-case`
  JSON, `--render-one-case-prompt` markdown, and `--compare-one-case-answer`
  JSON. Defaults target the synchronized main-Leaven `tmp/` artifacts:
  `tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json`,
  `tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1`,
  `tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt`,
  and
  `tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md`.
  The scorer compares `LISTS!A3:D32` against `1_13-1_golden.xlsx` and reports
  matched/total cells, score, pass/fail, and mismatch cells. This creates a
  repo-owned input/scoring surface for the next one-sample live attempt; it
  does not solve the workbook, invoke a model, or generate Trace2Skill
  trajectories.
- `examples/trace2skill_spreadsheetbench` now prepares a durable no-spend
  one-case run directory for exact case `13-1`: `--prepare-one-case-run
  --run-dir <tmp-run-dir>` writes `agent_prompt.md`, `manifest.json`, staged
  `1_13-1_init.xlsx` and `1_13-1_golden.xlsx`, a deterministic
  `13-1_output.xlsx` target, and explicit status
  `blocked_missing_live_spreadsheet_agent` / missing primitive
  `live_spreadsheet_agent_execution`. The agent prompt points only at the
  run-local input/output paths; the golden workbook is held in the manifest for
  scorer use and is not injected into the live-agent prompt.
- `examples/trace2skill_tiny_live` now exists in main Leaven as an
  outside-Cargo tiny live harness for the paper's causal loop: trajectory
  generation with a frozen initial skill, independent error/success analysts,
  consolidation, guarded skill update, and replay of the failed task. Its
  preflight is no-spend and its live mode is an explicit Codex/GPT-5.4-mini
  opt-in. This is a proxy live loop over two CSV tasks, not
  SpreadsheetBench/Qwen/vLLM replication.
- `leaven-evidence::AgentTrajectoryEvidence` now owns the reusable trajectory
  evidence envelope needed before Trace2Skill Stage 1/analysis can be
  faithfully replayed: runtime session id, optional Leaven case id, upstream
  task id, success/failure outcome, model id, model configuration fingerprint,
  transcript/command records, and parsed or blob-backed analyst records. It
  does not execute the ReAct loop or own Trace2Skill scorer/merge policy.
- `leaven-evidence::AgentTrajectoryCorpusEvidence` now owns the checkpointable
  many-trajectory evidence value needed to resume over the 200 train/evolving
  task manifest: caller-declared task ids, appended trajectory records,
  duplicate-manifest refusal, completed/pending task projection, unknown-task
  refusal, and support for repeated trajectories per task for multi-seed/retry
  protocols. Store backends remain generic and do not know this schema.
- `leaven-evidence::AgentAnalystFanoutEvidence` now owns the checkpointable
  many-call evidence value needed for Trace2Skill Stage 2 dispatch bookkeeping:
  caller-declared call ids, per-call analyst role, source task ids,
  prompt/response payloads, terminal parse/backend status, retry count, support
  count, duplicate-manifest refusal, unknown-call refusal, and terminal/pending
  projection. It does not schedule threads, call models, parse patches, or own
  Trace2Skill's hierarchical merge policy.
- `examples/trace2skill_spreadsheetbench` can derive a pending Stage 2 analyst
  fan-out manifest from the imported training/evolving trajectory corpus. This
  is a no-spend checkpoint scaffold for later analyst execution, not evidence
  that the analysts or merge ran.
- `leaven-evidence::AgentPatchMergeTreeEvidence` now owns the checkpointable
  Stage 3 merge provenance value: merge levels, input/accepted/discarded patch
  ids, support counts, merge decisions, prompt/response payloads, parse-failure
  artifacts, per-node output patches, and optional final diff. It records what
  a merge pipeline did; it does not schedule merges, decide prevalence, parse
  patch JSON, or apply edits to a skill directory.

Verification:

- `cargo nextest run -p leaven-agentic-skill -p leaven-eval` passed 37 tests
  on 2026-05-20, including source-row lowering and `skill_patch_plan_*`
  contract tests.
- `cargo nextest run -p leaven-agentic-skill` passed 22 tests on 2026-05-20,
  including `skill_patch_merge_tree_*` provenance and malformed-graph tests.
- `cargo clippy -p leaven-agentic-skill --all-targets -- -D warnings` passed
  on 2026-05-20 after adding `SkillPatchMergeTree`.
- `cargo test -p leaven --test topology_contract` passed 4 tests on
  2026-05-20.
- `cargo clippy -p leaven-agentic-skill -p leaven-eval --all-targets -- -D
  warnings` passed on 2026-05-20.
- `cargo test -p trace2skill_spreadsheetbench --test manifest` passed on
  2026-05-20 and verified the official local 400-row manifest lowers to 400
  cases with first id `13-1`, last id `59902`, and the paper split.
- `cargo test -p trace2skill_spreadsheetbench --test run_artifacts` passed on
  2026-05-20 and verified an upstream-shaped `results.json` plus log/analysis
  files lower into the training/evolving `AgentTrajectoryCorpusEvidence`.
- `cargo clippy -p trace2skill_spreadsheetbench --all-targets -- -D warnings`
  and `cargo test -p leaven --test topology_contract` passed on 2026-05-20
  after adding the manifest-lowering example as a workspace member.
- `cargo nextest run -p leaven-evidence`, `cargo clippy -p leaven-evidence
  --all-targets -- -D warnings`, and `cargo run -p p4_meta_harness_lite`
  passed on 2026-05-20 for the hard-cut trajectory evidence envelope, the
  many-trajectory corpus value, and the adapted P4 fixture caller.
- `cargo test -p leaven-evidence --test command analyst_fanout` passed on
  2026-05-20 for durable analyst fan-out call state, parse-failure terminal
  records, support counts, and duplicate/unknown call refusal.
- `cargo test -p trace2skill_spreadsheetbench --test run_artifacts` passed on
  2026-05-20 after deriving a pending Stage 2 analyst fan-out manifest from the
  imported training/evolving corpus.
- `cargo test -p leaven-evidence --test command patch_merge_tree` passed on
  2026-05-20 for merge levels, accepted/discarded patch ids, parse-failure
  nodes, final diff refs, duplicate node refusal, unknown final node refusal,
  and positive support enforcement.
- `cargo test -p leaven-agentic-skill --test skill_agentic
  skill_patch_application` passed on 2026-05-20 for atomic application of a
  validated patch plan, change reporting, and rollback evidence after failed
  atomic changes.
- `cargo test -p trace2skill_spreadsheetbench --test patch_bridge` passed on
  2026-05-20 for upstream-shaped fenced JSON patch lowering, reference
  create/link pairing, exact-section refusal, and atomic application through
  `SkillPatchApplication`.
- `cargo test -p trace2skill_spreadsheetbench --test patch_replay` passed on
  2026-05-20 for saved/live JSON patch merge artifact replay through
  `SkillPatchMergeTree`, selected final-patch application, evolved
  `SkillBank`, and change-report emission.
- `cargo test -p trace2skill_spreadsheetbench --test patch_replay` passed on
  2026-05-20 after adding the saved `--save-intermediates` directory loader,
  including preference for `translated_final_patch.json` as the applied patch
  when present.
- `cargo fmt --check`, `cargo test -p trace2skill_spreadsheetbench`, `cargo
  clippy -p trace2skill_spreadsheetbench --all-targets -- -D warnings`, and
  `cargo test -p leaven --test topology_contract` passed on 2026-05-20 for the
  saved-output loader slice.
- `cargo test -p trace2skill_spreadsheetbench --test one_case` passed on
  2026-05-20 for exact one-case artifact inspection and prompt rendering.
- `cargo test -p trace2skill_spreadsheetbench --test cli` passed on
  2026-05-20 for the binary `--inspect-one-case` JSON and
  `--render-one-case-prompt` markdown paths over test fixtures.
- `rustfmt --check --config skip_children=true
  examples/trace2skill_spreadsheetbench/src/main.rs
  examples/trace2skill_spreadsheetbench/src/lib.rs
  examples/trace2skill_spreadsheetbench/tests/one_case.rs
  examples/trace2skill_spreadsheetbench/tests/cli.rs` passed on 2026-05-20.
- `cargo test -p trace2skill_spreadsheetbench` passed on 2026-05-20 after the
  one-case preflight slice, including manifest, run-artifact, patch bridge,
  patch replay, one-case, and CLI tests.
- `cargo clippy -p trace2skill_spreadsheetbench --all-targets -- -D warnings`
  passed on 2026-05-20 after the one-case preflight slice.
- `cargo nextest run -p trace2skill_spreadsheetbench` passed on 2026-05-20
  with 12/12 tests.
- `cargo run -p trace2skill_spreadsheetbench -- --inspect-one-case` passed on
  2026-05-20 against the synchronized main-Leaven `tmp/` sample and reported
  case `13-1`, init workbook `12876` bytes, golden workbook `14698` bytes,
  prompt `647` bytes, system prompt `4971` bytes, released skill `22255`
  bytes, and an output path ending in
  `tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/13-1_output.xlsx`.
- `cargo run -p trace2skill_spreadsheetbench -- --render-one-case-prompt`
  passed on 2026-05-20 against the same artifacts and rendered the exact
  one-case prompt markdown without modifying the workbook.
- `cargo test -p trace2skill_spreadsheetbench --test workbook_score` passed on
  2026-05-20 for exact answer-range scoring: golden-vs-golden passes
  `120/120`, and init-vs-golden fails with mismatches in the answer range.
- `rustfmt --check --config skip_children=true
  examples/trace2skill_spreadsheetbench/src/lib.rs
  examples/trace2skill_spreadsheetbench/src/main.rs
  examples/trace2skill_spreadsheetbench/tests/cli.rs
  examples/trace2skill_spreadsheetbench/tests/workbook_score.rs` passed on
  2026-05-20 after the workbook scorer slice.
- `cargo test -p trace2skill_spreadsheetbench` passed on 2026-05-20 after the
  workbook scorer slice, including the new `workbook_score` test.
- `cargo clippy -p trace2skill_spreadsheetbench --all-targets -- -D warnings`
  passed on 2026-05-20 after the workbook scorer slice.
- `cargo nextest run -p trace2skill_spreadsheetbench` passed on 2026-05-20
  with 15/15 tests after the workbook scorer slice.
- `cargo test -p leaven --test topology_contract` passed 4/4 tests on
  2026-05-20 after adding the example-only `calamine` workbook reader
  dependency.
- `cargo run -p trace2skill_spreadsheetbench -- --compare-one-case-answer
  --output-workbook
  tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/1_13-1_golden.xlsx`
  passed on 2026-05-20 and reported `passed=true`, score `1.0`, `120/120`
  matched cells, and zero mismatches.
- `cargo run -p trace2skill_spreadsheetbench -- --compare-one-case-answer
  --output-workbook
  tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/1_13-1_init.xlsx`
  passed on 2026-05-20 and reported `passed=false`, score
  `0.6833333333333333`, `82/120` matched cells, 38 mismatches, with the first
  mismatch at `B15`.
- `bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --preflight`
  passed on 2026-05-20 after restoring the tiny live harness into main Leaven
  and switching its workspace guard away from the old `leaven-papers` checkout;
  it wrote `tmp/trace2skill_tiny_live/20260520T204154Z/preflight.json`.
- `rustfmt --check --config skip_children=true
  examples/trace2skill_spreadsheetbench/src/lib.rs
  examples/trace2skill_spreadsheetbench/src/main.rs
  examples/trace2skill_spreadsheetbench/src/one_case_run.rs
  examples/trace2skill_spreadsheetbench/tests/one_case_run.rs
  examples/trace2skill_spreadsheetbench/tests/cli.rs` passed on 2026-05-20
  after the one-case run-directory slice.
- `cargo test -p trace2skill_spreadsheetbench` passed on 2026-05-20 with
  17/17 tests after the one-case run-directory slice.
- `cargo clippy -p trace2skill_spreadsheetbench --all-targets -- -D warnings`
  passed on 2026-05-20 after renaming the temp-dir fixture fields used by the
  new run-directory tests.
- `cargo nextest run -p trace2skill_spreadsheetbench` passed on 2026-05-20
  with 17/17 tests after the one-case run-directory slice.
- `cargo test -p leaven --test topology_contract` passed 4/4 tests on
  2026-05-20 after adding the example-local run-preparation module.
- `cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-run
  --run-dir
  tmp/paper_exact_lane_runs/trace2skill/one_case_prepare_20260521T003128Z`
  passed on 2026-05-20 and wrote `agent_prompt.md` (`29083` bytes),
  `manifest.json` (`1479` bytes), staged init workbook (`12876` bytes), staged
  golden workbook (`14698` bytes), and output target
  `13-1_output.xlsx`; `manifest.json` reported
  `blocked_missing_live_spreadsheet_agent`, missing primitive
  `live_spreadsheet_agent_execution`, and `score_report=null`.
  `rg` over the generated `agent_prompt.md` found the run-local
  `working_directory`, `spreadsheet_path`, `output_path`, and
  `instruction_type` lines and no golden-workbook path.

## Current Blockers

Leaven-owned remaining primitives before faithful Trace2Skill replication:

- live paper-runner execution that writes the upstream `results.json`, logs,
  and analysis reports for the 200 training/evolving SpreadsheetBench
  trajectories. Leaven can now import those artifacts into
  `AgentTrajectoryCorpusEvidence`; the missing part is generating them through
  the Qwen/vLLM ReAct run or an approved documented substitute.
- live checkpointed parallel analyst execution for hundreds of independent
  agent/LLM calls. Leaven now has the durable fan-out evidence value and the
  Trace2Skill example can seed pending calls, but no scheduler/model client has
  executed those prompts;
- live hierarchical merge execution over analyst patch outputs. Leaven now has
  generic `AgentPatchMergeTreeEvidence` and skill-specific
  `SkillPatchMergeTree` provenance values plus JSON patch replay/application
  and a strict saved-output loader, but no Trace2Skill merge scheduler,
  prevalence policy, or model-backed merge operator has run;
- real upstream saved evolution output has not been generated or imported
  locally yet. The loader is tested against upstream-shaped fixtures; the next
  proof needs either a no-spend/small upstream output directory or live-run
  capture of translated exact map/merge patches plus explicit merge decisions;
- the exact case `13-1` now has repo-owned no-spend prompt/input and workbook
  scoring surfaces plus a durable prepared run directory contract, but no live
  spreadsheet agent has modified `13-1_output.xlsx`, no generated output
  workbook has been scored, and no generated trajectory has been fed into the
  Stage 2 analyst fan-out;
- result matrix/reporting for model scale transfer, OOD WikiTQ, DocVQA,
  DAPO/AIME, ablations, and sequential/retrieval baselines.

External/spend blockers:

- faithful spreadsheet evolution requires Qwen3.5-35B/122B or documented exact
  substitute served through vLLM/OpenAI-compatible endpoints. No large GPU or
  provider spend has been approved.
- DocVQA, DAPO-Math, AIME 2026, and WikiTQ converted spreadsheet assets still
  need exact local acquisition/provenance checks before those paper sections can
  be claimed.

## Next Action

Use the prepared one-case run directory to run the smallest approved live
spreadsheet attempt for case `13-1`, writing `13-1_output.xlsx`, durable
stdout/stderr/logs, and a `--compare-one-case-answer` report against the staged
`1_13-1_golden.xlsx`. After that, generate or import an actual no-spend/small
upstream `--save-intermediates` directory and feed it through the saved-output
loader. If running upstream live, also capture translated exact map/merge
patches plus accepted/discarded merge decisions, because the default saved
directory shape loses that decision provenance.
