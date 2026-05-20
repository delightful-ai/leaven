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
- `leaven-eval::Case::from_source_row` now owns the generic source-row case
  lowering needed for the 400-row release: Leaven case IDs are row-stable
  `CaseId::from_index(row)`, while upstream ids such as `13-1` and `59902` are
  preserved in metadata as `source_id`, with `source_row_index` also recorded.
  SpreadsheetBench JSON parsing remains paper-specific loader work.
- `examples/trace2skill_spreadsheetbench` now owns the first no-spend
  paper-specific loader pass: it parses the official local
  SpreadsheetBench-Verified 400-row `dataset.json`, lowers rows into
  `leaven-eval::Case<SpreadsheetBenchTask, SpreadsheetBenchAnswerSpec>` with
  source-row metadata, and builds the paper's `0..200` train/evolving and
  `200..400` held-out split through `RowOrderSplitBuilder`. This is a
  mechanics-smoke for manifest provenance, not spreadsheet execution or skill
  evolution.
- This slice adds `leaven-agentic-skill::SkillPatchPlan`, a paper-neutral
  mechanical guardrail for agent-authored skill patch plans. It validates
  existing-file modifications/deletions, create-file overwrite refusal,
  positive support counts, same-file range conflicts, and atomic pairing between
  new `references/*.md` files and `SKILL.md` links. It intentionally does not
  own Trace2Skill merge policy, prevalence thresholds, prompt wording, batch
  sizes, or result selection.

Verification:

- `cargo nextest run -p leaven-agentic-skill -p leaven-eval` passed 37 tests
  on 2026-05-20, including source-row lowering and `skill_patch_plan_*`
  contract tests.
- `cargo test -p leaven --test topology_contract` passed 4 tests on
  2026-05-20.
- `cargo clippy -p leaven-agentic-skill -p leaven-eval --all-targets -- -D
  warnings` passed on 2026-05-20.
- `cargo test -p trace2skill_spreadsheetbench --test manifest` passed on
  2026-05-20 and verified the official local 400-row manifest lowers to 400
  cases with first id `13-1`, last id `59902`, and the paper split.
- `cargo clippy -p trace2skill_spreadsheetbench --all-targets -- -D warnings`
  and `cargo test -p leaven --test topology_contract` passed on 2026-05-20
  after adding the manifest-lowering example as a workspace member.

## Current Blockers

Leaven-owned remaining primitives before faithful Trace2Skill replication:

- checkpointed many-trajectory evidence corpus with success/failure labels,
  full ReAct trace access, task ids, model/config fingerprints, and derived
  analysis records;
- checkpointed parallel analyst fan-out for hundreds of independent agent/LLM
  calls with durable per-call prompts, responses, parse failures, and retry
  state;
- hierarchical merge-tree artifact that records patch batches, merge levels,
  accepted/discarded patches, support counts, prevalence decisions, and final
  applied diff;
- deterministic skill-directory patch application with rollback and validation
  integrated with `SkillBank` materialization/readback;
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

Define the run artifact schema for Trace2Skill trace records and patch-plan
records without launching model/GPU work. The next Leaven-owned primitive is a
checkpointed many-trajectory evidence corpus that can preserve full ReAct
traces, success/failure labels, task ids, model/config fingerprints, and parsed
analysis records.
