# Trace2Skill Full-Denominator Runbook

This runbook is generated from the ARA approval packet and dataset manifest.
It is not permission to launch Qwen/vLLM work.

Normal approval preflight passes: `false`

## Paper Protocol

- Dataset path: `data/spreadsheetbench_verified/spreadsheetbench_verified_400`
- Seeds: `[41, 42, 43]`
- Workers: `128`
- Merge batch size: `32`
- ReAct turn budget: `100`
- Case order SHA-256: `ac05d2035ad776af9d901689423645316e707e6e8426a04d2eae6591929b64e9`

## Approval Blockers

- models.qwen_122b is unresolved
- models.qwen_35b is unresolved
- serving.host is unresolved
- serving.version is unresolved
- serving.tensor_parallel is unresolved
- serving.gpu_type is unresolved
- serving.gpu_count is unresolved
- budget.max_usd is unresolved
- budget.max_wall_clock_hours is unresolved
- budget.max_gpu_hours is unresolved
- credentials.api_key_env is unresolved
- credentials.redaction_policy is unresolved
- credentials.log_retention is unresolved
- artifacts.root is unresolved
- artifacts.retention is unresolved
- approval.approved_by is unresolved
- approval.approved_at is unresolved
- tolerance.approved must be True, got None

## Stages

### G0: No-spend guardrails

- Denominator: `ara-and-approval-preflight`
- Runnable now: `true`
- Approval required: `false`
- Allowed label: `guardrail-ready`
- Forbidden label: `paper reproduction`

Commands:

```bash
uv run python scripts/build_trace2skill_dataset_manifest.py
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench --expect-blocked
uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Expected artifacts:
- `docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json`
- `docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json`
- `docs/ara/trace2skill_spreadsheetbench/validation.md`

### G1: Deterministic one-case Leaven seam proof

- Denominator: `one-case-13-1-deterministic`
- Runnable now: `true`
- Approval required: `false`
- Allowed label: `deterministic-one-case`
- Forbidden label: `paper reproduction`

Commands:

```bash
cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-run --run-dir tmp/trace2skill-one-case-live
cargo run -p trace2skill_spreadsheetbench -- --run-one-case-acp-worker --run-dir tmp/trace2skill-one-case-live --model-id local-openpyxl-trace2skill-agent
```

Expected artifacts:
- `tmp/trace2skill-one-case-live/13-1_output.xlsx`
- `tmp/trace2skill-one-case-live/acp_result.json`
- `tmp/trace2skill-one-case-live/score_report.json`
- `tmp/trace2skill-one-case-live/trajectory.json`

### G1M: Model-backed one-case upstream gate

- Denominator: `one-case-13-1-model-backed`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `model-one-case`
- Forbidden label: `held-out split reproduced`

Commands:

```bash
cd tmp/repros/trace2skill-upstream
DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400
MODEL=${MODEL:?set approved served model id}
WORKERS=128
MERGE_BATCH_SIZE=32
MAX_TURNS=100
SEEDS=(41 42 43)
GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json
THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json
RUN_ROOT=${RUN_ROOT:?set approved artifact root}
SEED=41
ONE_CASE_DIR="$RUN_ROOT/model_one_case_seed_${SEED}"
python run_spreadsheetbench.py --data_path "$DATA_PATH" --model "$MODEL" --agent cli_skill_preloaded --log_dir "$ONE_CASE_DIR/logs" --log_format markdown --working_dir "$ONE_CASE_DIR/work" --output_dir "$ONE_CASE_DIR/outputs" --max_turns "$MAX_TURNS" --workers 1 --skills_dir spreadsheet_agent/skills --seeds "$SEED" --generation_config "$GENERATION_CONFIG" --start_idx 0 --end_idx 1
python evaluate_with_official.py --data_path "$DATA_PATH" --output_dir "$ONE_CASE_DIR/outputs" --verbose --start_idx 0 --end_idx 1
```

Expected artifacts:
- `model_one_case_seed_41/logs`
- `model_one_case_seed_41/work`
- `model_one_case_seed_41/outputs/eval_official_results.json`

### G2: Small-N held-out subset gate

- Denominator: `paper-subset`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `paper-subset`
- Forbidden label: `held-out split reproduced`

Commands:

```bash
cd tmp/repros/trace2skill-upstream
DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400
MODEL=${MODEL:?set approved served model id}
WORKERS=128
MERGE_BATCH_SIZE=32
MAX_TURNS=100
SEEDS=(41 42 43)
GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json
THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json
RUN_ROOT=${RUN_ROOT:?set approved artifact root}
SEED=41
SUBSET_START=200
SUBSET_END=${SUBSET_END:?set small approved end <= 400}
SUBSET_DIR="$RUN_ROOT/subset_${SUBSET_START}_${SUBSET_END}_seed_${SEED}"
python run_spreadsheetbench.py --data_path "$DATA_PATH" --model "$MODEL" --agent cli_skill_preloaded --log_dir "$SUBSET_DIR/logs" --log_format markdown --working_dir "$SUBSET_DIR/work" --output_dir "$SUBSET_DIR/outputs" --max_turns "$MAX_TURNS" --workers "$WORKERS" --skills_dir spreadsheet_agent/skills --seeds "$SEED" --generation_config "$GENERATION_CONFIG" --start_idx "$SUBSET_START" --end_idx "$SUBSET_END"
python evaluate_with_official.py --data_path "$DATA_PATH" --output_dir "$SUBSET_DIR/outputs" --verbose --start_idx "$SUBSET_START" --end_idx "$SUBSET_END"
```

Expected artifacts:
- `subset_<start>_<end>_seed_41/logs`
- `subset_<start>_<end>_seed_41/work`
- `subset_<start>_<end>_seed_41/outputs/eval_official_results.json`

### G3: Evolving split trajectory and skill evolution

- Denominator: `evolving-split-0..200`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `evolving-split-run`
- Forbidden label: `held-out result`

Commands:

```bash
cd tmp/repros/trace2skill-upstream
DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400
MODEL=${MODEL:?set approved served model id}
WORKERS=128
MERGE_BATCH_SIZE=32
MAX_TURNS=100
SEEDS=(41 42 43)
GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json
THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json
RUN_ROOT=${RUN_ROOT:?set approved artifact root}
for SEED in "${SEEDS[@]}"; do
  BASELINE_DIR="$RUN_ROOT/baseline_seed_${SEED}"
  python run_spreadsheetbench.py --data_path "$DATA_PATH" --model "$MODEL" --agent cli_skill_preloaded --log_dir "$BASELINE_DIR/logs" --log_format markdown --working_dir "$BASELINE_DIR/work" --output_dir "$BASELINE_DIR/outputs" --max_turns "$MAX_TURNS" --workers "$WORKERS" --skills_dir spreadsheet_agent/skills --seeds "$SEED" --generation_config "$GENERATION_CONFIG" --start_idx 0 --end_idx 200
  python evaluate_with_official.py --data_path "$DATA_PATH" --output_dir "$BASELINE_DIR/outputs" --verbose --start_idx 0 --end_idx 200
  python analyze_results.py --eval_results "$BASELINE_DIR/outputs/eval_official_results.json" --log_dir "$BASELINE_DIR/logs"
  python analysis/run_error_analysis.py --data_path "$DATA_PATH" --work_dir "$BASELINE_DIR/work" --logs_dir "$BASELINE_DIR/logs" --output_dir "$BASELINE_DIR/error_analysis" --model "$MODEL" --workers "$WORKERS" --generation_config "$GENERATION_CONFIG" --max_turns "$MAX_TURNS"
  python analysis/run_success_analysis_llm.py --logs_dir "$BASELINE_DIR/logs" --output_dir "$BASELINE_DIR/success_analysis" --model "$MODEL" --max_workers "$WORKERS" --generation_config "$THINK_GENERATION_CONFIG"
  python analysis/parse_error_analysis_outputs.py --input_dir "$BASELINE_DIR/error_analysis" --output "$BASELINE_DIR/error_analysis_parsed.json"
  python analysis/parse_success_analysis_outputs.py --input_dir "$BASELINE_DIR/success_analysis" --output "$BASELINE_DIR/success_analysis_parsed.json"
  EVOLUTION_DIR="$RUN_ROOT/skill_evolution_seed_${SEED}/error_driven_skill_evolution"
  EVOLVED_SKILLS="$EVOLUTION_DIR/skills"
  mkdir -p "$EVOLVED_SKILLS" && cp -r spreadsheet_agent/skills/. "$EVOLVED_SKILLS"
  python -m skill_evolver.run_parallel_skill_evolution --input-json "$BASELINE_DIR/error_analysis_parsed.json" --skill-dir "$EVOLVED_SKILLS/xlsx" --model "$MODEL" --verbose --batch-size 1 --changelog "$EVOLUTION_DIR/change.log" --save-intermediates --intermediates-dir "$EVOLUTION_DIR/intermediates" --max-workers "$WORKERS" --prompt generic --generation-config "$THINK_GENERATION_CONFIG" --parse-failure-dir "$EVOLUTION_DIR/parse_failures" --patch-pipeline json --seed "$SEED"
done
```

Expected artifacts:
- `baseline_seed_{seed}/logs`
- `baseline_seed_{seed}/work`
- `baseline_seed_{seed}/outputs/eval_official_results.json`
- `baseline_seed_{seed}/error_analysis_parsed.json`
- `baseline_seed_{seed}/success_analysis_parsed.json`
- `skill_evolution_seed_{seed}/error_driven_skill_evolution/change.log`
- `skill_evolution_seed_{seed}/error_driven_skill_evolution/intermediates`
- `skill_evolution_seed_{seed}/error_driven_skill_evolution/skills`

### G3V: Training-set validation and best-seed selection

- Denominator: `training-validation-0..200`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `training-validation-candidate`
- Forbidden label: `held-out result`

Commands:

```bash
cd tmp/repros/trace2skill-upstream
DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400
MODEL=${MODEL:?set approved served model id}
WORKERS=128
MERGE_BATCH_SIZE=32
MAX_TURNS=100
SEEDS=(41 42 43)
GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json
THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json
RUN_ROOT=${RUN_ROOT:?set approved artifact root}
for SEED in "${SEEDS[@]}"; do
  EVOLVED_SKILLS="$RUN_ROOT/skill_evolution_seed_${SEED}/error_driven_skill_evolution/skills"
  VALIDATION_DIR="$RUN_ROOT/validation_train_seed_${SEED}"
  python run_spreadsheetbench.py --data_path "$DATA_PATH" --model "$MODEL" --log_dir "$VALIDATION_DIR/baseline_logs" --working_dir "$VALIDATION_DIR/baseline_work" --output_dir "$VALIDATION_DIR/baseline_outputs" --max_turns "$MAX_TURNS" --workers "$WORKERS" --skills_dir spreadsheet_agent/skills --seeds "$SEED" --generation_config "$GENERATION_CONFIG" --start_idx 0 --end_idx 200
  python evaluate_with_official.py --data_path "$DATA_PATH" --output_dir "$VALIDATION_DIR/baseline_outputs" --start_idx 0 --end_idx 200
  python run_spreadsheetbench.py --data_path "$DATA_PATH" --model "$MODEL" --log_dir "$VALIDATION_DIR/evolved_logs" --working_dir "$VALIDATION_DIR/evolved_work" --output_dir "$VALIDATION_DIR/evolved_outputs" --max_turns "$MAX_TURNS" --workers "$WORKERS" --skills_dir "$EVOLVED_SKILLS" --seeds "$SEED" --generation_config "$GENERATION_CONFIG" --start_idx 0 --end_idx 200
  python evaluate_with_official.py --data_path "$DATA_PATH" --output_dir "$VALIDATION_DIR/evolved_outputs" --start_idx 0 --end_idx 200
done
# Select BEST_SEED from training-set validation only; do not inspect held-out outputs.
```

Expected artifacts:
- `validation_train_seed_{seed}/baseline_outputs/eval_official_results.json`
- `validation_train_seed_{seed}/evolved_outputs/eval_official_results.json`
- `best_seed_selection_note.md`

### G4: Held-out split evaluation

- Denominator: `held-out-200..400`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `held-out-single-seed-candidate`
- Forbidden label: `paper aggregate`

Commands:

```bash
cd tmp/repros/trace2skill-upstream
DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400
MODEL=${MODEL:?set approved served model id}
WORKERS=128
MERGE_BATCH_SIZE=32
MAX_TURNS=100
SEEDS=(41 42 43)
GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json
THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json
RUN_ROOT=${RUN_ROOT:?set approved artifact root}
BEST_SEED=${BEST_SEED:?select using training-set validation only}
EVOLVED_SKILLS="$RUN_ROOT/skill_evolution_seed_${BEST_SEED}/error_driven_skill_evolution/skills"
EVOLVED_RUN_DIR="$RUN_ROOT/heldout_seed_${BEST_SEED}"
python run_spreadsheetbench.py --data_path "$DATA_PATH" --model "$MODEL" --log_dir "$EVOLVED_RUN_DIR/logs" --log_format markdown --working_dir "$EVOLVED_RUN_DIR/work" --output_dir "$EVOLVED_RUN_DIR/outputs" --max_turns "$MAX_TURNS" --workers "$WORKERS" --skills_dir "$EVOLVED_SKILLS" --seeds "$BEST_SEED" --generation_config "$GENERATION_CONFIG" --start_idx 200 --end_idx 400
python evaluate_with_official.py --data_path "$DATA_PATH" --output_dir "$EVOLVED_RUN_DIR/outputs" --start_idx 200 --end_idx 400
```

Expected artifacts:
- `heldout_seed_<best>/logs`
- `heldout_seed_<best>/work`
- `heldout_seed_<best>/outputs/eval_official_results.json`

### G5: Seed aggregate and result rows

- Denominator: `seed-aggregate-41-42-43`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `seed-aggregate-candidate`
- Forbidden label: `cross-model paper reproduction`

Commands:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench
# After all three approved seed runs finish, write denominator-labeled rows to:
# docs/ara/trace2skill_spreadsheetbench/results/<approved-run-id>.jsonl
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench
```

Expected artifacts:
- `docs/ara/trace2skill_spreadsheetbench/results/<approved-run-id>.jsonl`
- `docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png`
- `docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json`

### G6: Cross-model paper rows

- Denominator: `full-paper-denominator`
- Runnable now: `false`
- Approval required: `true`
- Allowed label: `paper-denominator-reproduction`
- Forbidden label: `anything stronger than completed rows`

Commands:

```bash
# Repeat G3-G5 for each approved paper model/condition row being claimed.
# Do not mark paper-denominator-reproduction until closeout_audit.json can prove every claimed row.
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench
uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench
```

Expected artifacts:
- `complete denominator-labeled result JSONL rows`
- `updated closeout_audit.json with overall_complete true only after objective-wide proof`
