# Five Paper Infrastructure Requirements

Status: infrastructure inventory, not replication proof.

This document catalogs what we need to set up before claiming faithful
replication for the five skill-optimization papers currently under
`leaven-papers`.

The core rule for the next phase is:

> Build the paper's actual execution environment and loop first. If the setup is
> faithful, the paper's original prompts should fit naturally. If prompts need to
> be rewritten to fit our toy harness, we have not replicated the paper.

The current `scripts/run_paper_exact_lanes.py` work is therefore only a
materialization/preflight scaffold. It is useful because it pins real examples
and prompt files, but direct Python-to-Codex calls are not the Leaven-throughline
or paper-throughline execution environment.

## Global Infrastructure Contract

Every faithful paper lane needs the same outer substrate, regardless of paper:

1. Isolated workspace per paper run.
   - No mutation of the main Leaven checkout.
   - No mutation of the upstream repro checkout unless the paper requires a
     mutable project repo.
   - All generated work goes under an ignored run root such as
     `tmp/paper_runs/<paper>/<run_id>/`.

2. Immutable source anchors.
   - Pin upstream repo URL and commit.
   - Pin paper source path / arXiv id.
   - Pin dataset source, exact split, and sample ids.
   - Pin model substitution policy. Current approved local substitution is
     Codex with `gpt-5.4-mini`; this is not paper-exact for model identity.

3. First-class VCS state.
   - If the paper uses git branches/tags as part of the algorithm, we must
     create an actual nested git repo for that paper run.
   - If the paper mutates skills/code, each candidate program needs a durable
     version boundary, not an in-memory dict.
   - Leaven should observe, snapshot, and compare those states. It should not
     silently replace them with a toy state machine unless the paper itself did.

4. Exact runtime harness.
   - Use the paper's runner when present.
   - Use the paper's agent CLI / SDK contract when present.
   - Preserve tool availability, working directory, sandbox policy, timeout,
     concurrency, retry policy, and environment variables.

5. Exact prompt assembly.
   - Store prompt fragments and the fully rendered runtime prompt.
   - Keep role separation: executor/proposer/generator/judge/router/etc.
   - Do not "normalize" the prompt into a Leaven house style before proving the
     paper loop.

6. Artifact and trace store.
   - Capture input examples, runtime prompts, model outputs, tool calls,
     generated files, branch diffs, evaluation outputs, and failure analyses.
   - A later Leaven abstraction can be extracted only from these repeated,
     paper-faithful traces.

7. One-real-example e2e gate first.
   - Full dataset split can wait.
   - The first real gate is one to three valid train examples through the actual
     paper loop under `gpt-5.4-mini`, with spendful model calls and real
     environment mutations.
   - No-spend tests are allowed for parser/config smoke only; they are not proof
     of agent optimization behavior.

## Current Local Source Anchors

| Surface | Local path | Remote | Commit |
| --- | --- | --- | --- |
| EvoSkill code | `tmp/repros/evoskill` | `https://github.com/sentient-agi/EvoSkill.git` | `e881c715dcab` |
| Trace2Skill code | `tmp/repros/trace2skill-upstream` | `https://github.com/Qwen-Applications/Trace2Skill.git` | `3d0b52a140f0` |
| D2Skill code | `tmp/repros/d2skill-agenticrl` | `https://github.com/TU2021/D2Skill-AgenticRL.git` | `404ae893ef87` |
| Memento-Skills code | `tmp/repros/memento-skills` | `https://github.com/Memento-Teams/Memento-Skills.git` | `07b530edc737` |
| SkillsBench code/data | `tmp/repros/skillsbench` | `https://github.com/benchflow-ai/skillsbench.git` | `72573ab7d8a9` |
| OfficeQA code/data | `tmp/repros/officeqa` | `https://github.com/databricks/officeqa.git` | `78748e5d669d` |
| ALFWorld code | `tmp/repros/alfworld` | `https://github.com/alfworld/alfworld.git` | `aaba6870f86c` |

Paper sources are under `tmp/skill_opt_sources/`:

| Paper | Paper source path |
| --- | --- |
| EvoSkill | `tmp/skill_opt_sources/arx_2603.02766/full_source_flat.tex` |
| Memento-Skills | `tmp/skill_opt_sources/arx_2603.18743/full_source_flat.tex` |
| Trace2Skill | `tmp/skill_opt_sources/arx_2603.25158/full_source_flat.tex` |
| D2Skill | `tmp/skill_opt_sources/arx_2603.28716/full_source_flat.tex` |
| SkillReducer | `tmp/skill_opt_sources/arx_2603.29919/full_source_flat.tex` |

Current materialized sample manifest:

| Paper | Sample status | Sample |
| --- | --- | --- |
| EvoSkill | materialized | OfficeQA Pro `UID0001`; SealQA `seal_0` first streamed case |
| Trace2Skill | materialized | SpreadsheetBench Verified case `13-1` |
| D2Skill | materialized | ALFWorld `json_2.1.1` train trajectory |
| SkillReducer | materialized | SkillsBench `jax-computing-basics` plus `skill-creator` skill |
| Memento-Skills | blocked | GAIA and HLE are gated under current HF auth |

## Paper 1: EvoSkill

Paper: "EvoSkill: Automated Skill Discovery for Coding Agents"

### What The Paper Actually Optimizes

EvoSkill evolves complete agent programs. A program is not just a prompt. It is
the current system prompt, skill folders, metadata, score, and parent lineage.
The loop:

1. Run executor on train examples.
2. Score outputs.
3. Collect failures below threshold.
4. Ask proposer to diagnose failures and propose a skill or prompt mutation.
5. Ask skill-builder or prompt-generator to materialize the mutation.
6. Evaluate candidate on validation examples.
7. Admit candidate into a bounded frontier if it improves.
8. Persist lineage as git branches/tags.

The paper is explicit that programs are represented with git-backed branches
prefixed `program/` and frontier tags prefixed `frontier/`. This is not incidental
plumbing. It is the data structure for the search.

### Concrete Upstream Setup

Code setup:

```bash
cd tmp/repros/evoskill
uv sync
# or
pip install -e .
```

Agent CLI dependencies are external to the Python package:

```bash
brew install --cask claude-code
brew install opencode
brew install --cask codex
brew install block-goose-cli
```

Common auth:

```bash
export ANTHROPIC_API_KEY=...
export OPENAI_API_KEY=...
export OPENROUTER_API_KEY=...
```

The included OfficeQA example lives at:

```text
tmp/repros/evoskill/examples/officeqa/
```

It includes:

```text
.evoskill/config.toml
.evoskill/config.openrouter.toml
.evoskill/task.md
data/officeqa_sample.csv
data/treasury_bulletins/
setup.sh
demo.sh
```

The nested OfficeQA git repo is not currently initialized. EvoSkill expects it:

```bash
cd tmp/repros/evoskill/examples/officeqa
bash setup.sh
```

That script creates `.git`, writes `.evoskill/state.json`, commits the initial
project, and then `evoskill run` can create `program/*` branches and
`frontier/*` tags. Without this nested git repo, the paper's program lineage
mechanism is absent.

### Runtime Configuration To Preserve

Default OfficeQA config:

```toml
[harness]
name = "claude"
model = "anthropic/claude-sonnet-4-6"
data_dirs = ["data/treasury_bulletins"]
timeout_seconds = 600
max_retries = 2

[evolution]
mode = "skill_only"
iterations = 3
frontier_size = 2
concurrency = 2
no_improvement_limit = 3
failure_samples = 2

[dataset]
path = "data/officeqa_sample.csv"
question_column = "question"
ground_truth_column = "answer"
category_column = "difficulty"
train_ratio = 0.4
val_ratio = 0.2

[scorer]
type = "multi_tolerance"
```

The paper runs OfficeQA with Claude Code / Opus 4.5, training splits of 5%, 10%,
and 15%, 17 validation examples, held-out test on the remainder, and skill-merge
across independent runs. It also runs SealQA `seal-0` with a 10% training split
and applies the learned search skill zero-shot to BrowseComp.

For our first real sample, use:

```text
tmp/paper_exact_samples/evoskill/officeqa/officeqa_pro_first_case.json
tmp/paper_exact_samples/evoskill/officeqa/treasury_bulletin_1941_01.txt
tmp/paper_exact_samples/evoskill/sealqa/seal_0_first_case.json
```

### Prompt Surfaces

Paper appendix placeholders:

```text
tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/proposer_placeholder.md
tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/skill_builder_placeholder.md
tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/auto_grader_placeholder.md
```

Upstream runtime prompts:

```text
tmp/repros/evoskill/src/agent_profiles/proposer/prompt.py
tmp/repros/evoskill/src/agent_profiles/skill_proposer/prompt.py
tmp/repros/evoskill/src/agent_profiles/skill_generator/prompt.py
tmp/repros/evoskill/src/agent_profiles/prompt_proposer/prompt.py
tmp/repros/evoskill/src/agent_profiles/prompt_generator/prompt.py
tmp/repros/evoskill/src/agent_profiles/base_agent/prompt.py
tmp/repros/evoskill/src/agent_profiles/base_agent/prompt.txt
tmp/repros/evoskill/src/agent_profiles/sealqa_agent/prompt.txt
```

Important mismatch to track: the paper appendix describes Claude Code skills,
while the current upstream generator prompt says "one repo-local skill for
OpenCode" and writes `.claude/skills/<skill-name>/SKILL.md`. A faithful lane
must preserve the selected harness's exact skill discovery semantics instead of
papering over them.

### Infrastructure We Need

- A disposable nested git repo per EvoSkill benchmark run.
- A runner that invokes `evoskill run` rather than reimplementing the loop.
- Config generation for `gpt-5.4-mini` through the `codex` harness or through an
  OpenAI-compatible harness, with explicit model-substitution labeling.
- Captured branch graph after each iteration: program branches, frontier tags,
  `.claude/program.yaml`, `.claude/skills/`, `.evoskill/feedback_history.md`,
  `.evoskill/loop_checkpoint.json`.
- Captured fully rendered prompts for executor, proposer, and skill-builder.
- One-real-example mode that still exercises: train failure detection, proposer,
  builder write, validation scoring, and branch/tag mutation.
- Later full mode that restores paper split sizes and skill-merge behavior.

### Current Blockers / Non-Exactness

- `evoskill` is not currently on PATH in this workspace.
- OfficeQA nested `.git` has not been initialized.
- The current Leaven P5 example is not the paper environment.
- The previous live preflight called Codex directly and therefore skipped
  EvoSkill's git-backed program manager.

## Paper 2: Trace2Skill

Paper: "Trace2Skill: Distill Trajectory-Local Lessons into Transferable Agent
Skills"

### What The Paper Actually Optimizes

Trace2Skill distills many agent execution traces into a durable skill directory.
The key loop is:

1. Run a task agent on SpreadsheetBench and collect trajectories/files.
2. Evaluate outputs with the official-compatible scorer.
3. Analyze failures with artifact access and minimal-fix validation.
4. Analyze successes for generalizable patterns.
5. Generate trajectory-local skill patches in parallel.
6. Hierarchically consolidate patches into one non-overlapping skill update.
7. Validate evolved skill on the training split.
8. Select the best seed by train validation.
9. Evaluate selected skill on held-out split.

The paper explicitly contrasts this with sequential per-trajectory editing. The
important infrastructure is parallel trace processing plus conflict-free
many-to-one consolidation.

### Concrete Upstream Setup

Code setup:

```bash
cd tmp/repros/trace2skill-upstream
python -m pip install openai tqdm openpyxl requests diskcache
export OPENAI_API_KEY=...
export OPENAI_BASE_URL=... # optional OpenAI-compatible endpoint
```

The released reproduction data is included:

```text
tmp/repros/trace2skill-upstream/data/spreadsheetbench_verified/spreadsheetbench_verified_400
```

The included reproduction script uses:

```bash
DATA_PATH=data/spreadsheetbench_verified/spreadsheetbench_verified_400
MODEL=Qwen3.5-122B-A10B
WORKERS=128
SEED=41
GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_instruct_reasoning.json
THINK_GENERATION_CONFIG=gen_config/qwen3.5_35B_122B_thinking_reasoning.json
```

Baseline held-out run:

```bash
python run_spreadsheetbench.py \
  --data_path "$DATA_PATH" \
  --model "$MODEL" \
  --agent cli_only \
  --log_dir "$CLI_ONLY_DIR/logs" \
  --log_format markdown \
  --working_dir "$CLI_ONLY_DIR/work" \
  --output_dir "$CLI_ONLY_DIR/outputs" \
  --max_turns 100 \
  --workers "$WORKERS" \
  --seeds "$SEED" \
  --generation_config "$GENERATION_CONFIG" \
  --start_idx 200 \
  --end_idx 400
```

Training split with skill preloaded:

```bash
python run_spreadsheetbench.py \
  --data_path "$DATA_PATH" \
  --model "$MODEL" \
  --agent cli_skill_preloaded \
  --log_dir "$BASELINE_DIR/logs" \
  --log_format markdown \
  --working_dir "$BASELINE_DIR/work" \
  --output_dir "$BASELINE_DIR/outputs" \
  --max_turns 100 \
  --workers "$WORKERS" \
  --skills_dir spreadsheet_agent/skills \
  --seeds "$SEED" \
  --generation_config "$GENERATION_CONFIG" \
  --start_idx 0 \
  --end_idx 200
```

Skill evolution:

```bash
python -m skill_evolver.run_parallel_skill_evolution \
  --input-json "$BASELINE_DIR/error_analysis_parsed.json" \
  --skill-dir "$EVOLVED_SKILLS/xlsx" \
  --model "$MODEL" \
  --verbose \
  --batch-size 1 \
  --changelog "$EVOLUTION_DIR/change.log" \
  --save-intermediates \
  --intermediates-dir "$EVOLUTION_DIR/intermediates" \
  --max-workers "$WORKERS" \
  --prompt generic \
  --generation-config "$THINK_GENERATION_CONFIG" \
  --parse-failure-dir "$EVOLUTION_DIR/parse_failures" \
  --patch-pipeline json \
  --seed "$SEED"
```

### Prompt Surfaces

Runtime agent prompts:

```text
tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_only_full_system_v1.txt
tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt
```

Analysis prompts:

```text
tmp/repros/trace2skill-upstream/analysis/error_analysis_system.txt
tmp/repros/trace2skill-upstream/analysis/error_analysis_user.txt
tmp/repros/trace2skill-upstream/analysis/error_analysis_system_llm.txt
tmp/repros/trace2skill-upstream/analysis/error_analysis_user_llm.txt
tmp/repros/trace2skill-upstream/analysis/success_analysis_system_llm.txt
tmp/repros/trace2skill-upstream/analysis/success_analysis_user_llm.txt
```

Skill evolution prompts:

```text
tmp/repros/trace2skill-upstream/skill_evolver/prompts/skill_evolving_agent/
tmp/repros/trace2skill-upstream/skill_evolver/prompts/success_evolving_agent/
tmp/repros/trace2skill-upstream/skill_evolver/prompts/parallel_evolving_agent/
```

Released skills:

```text
tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/
tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-122B-combined/
tmp/repros/trace2skill-upstream/released_skills/xlsx-35B/
tmp/repros/trace2skill-upstream/released_skills/xlsx-122B/
tmp/repros/trace2skill-upstream/spreadsheet_agent/skills/xlsx/
```

Paper appendix prompt text is abbreviated but explicitly includes:

- Stage 1 agent system prompt with preloaded skill content.
- Stage 2 error analyst prompt requiring artifact access, root-cause tracing,
  minimal fix, and re-evaluation.
- Stage 2 success analyst prompt for generalizable patterns.
- Stage 3 merge operator prompt for dedupe, conflict resolution, unique insight
  preservation, conciseness, line independence, and atomic create/link pairs.

### Infrastructure We Need

- A run root with separate `work`, `outputs`, `logs`, `error_analysis`,
  `success_analysis`, `intermediates`, and `parse_failures` directories.
- Spreadsheet filesystem sandbox preserving input/golden xlsx files.
- Official-compatible evaluator invocation after every agent run.
- Artifact access for analysts: logs, produced files, ground truth, scorer
  outputs, and minimal-fix workspace.
- Parallel patch proposal executor with bounded worker count.
- Hierarchical merge executor that preserves non-overlapping patches.
- Skill directory diff capture before and after evolution.
- Seed loop and best-seed selection by train validation.
- Held-out evaluation split after seed selection.

### One-Sample Gate

Use current materialized case:

```text
tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json
tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/1_13-1_init.xlsx
tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/1_13-1_golden.xlsx
tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1/prompt.txt
```

The smallest honest e2e should still:

1. Run `run_spreadsheetbench.py` on case `13-1`.
2. Run `evaluate_with_official.py`.
3. If failure, run error analysis with artifact access.
4. Produce one parsed patch.
5. Run the skill evolution entrypoint against a copied skill directory.
6. Re-run the same case with evolved skill and compare score/diff.

### Current Blockers / Non-Exactness

- The previous Leaven tiny example proved patch consolidation shape, not the
  upstream SpreadsheetBench runner.
- Full paper reproduction expects high worker counts and Qwen-family models.
  `gpt-5.4-mini` is a deliberate model substitution.
- Need to confirm whether the one-case runner can use `--start_idx`/`--end_idx`
  around case `13-1` or needs a tiny JSONL/dataset directory shim.

## Paper 3: Memento-Skills

Paper: "Memento-Skills: Let Agents Design Agents"

### What The Paper Actually Optimizes

Memento-Skills is a continual learning agent with a read-write reflective loop.
Its memory unit is an executable skill folder, not a scalar memory row. The loop:

1. Read: route the current task to a relevant skill.
2. Execute: run the skill through the agent/tool/sandbox stack.
3. Reflect: judge outcome, attribute failure to a skill or missing skill.
4. Write: update utility, rewrite the skill folder, or create a new skill.

The paper also trains/evaluates a behavior-aligned router. It generates
synthetic router goals from skill name/description/keywords, includes hard
negatives, and evaluates Recall@K plus end-to-end execution success.

### Concrete Upstream Setup

Code setup:

```bash
cd tmp/repros/memento-skills
python -m venv .venv
source .venv/bin/activate
pip install -e .
memento doctor
memento agent
```

Runtime configuration is not primarily `.env` based. First launch creates:

```text
~/memento_s/config.json
```

That file configures:

```json
{
  "llm": {
    "active_profile": "default",
    "profiles": {
      "default": {
        "model": "openai/gpt-4o",
        "api_key": "your-api-key",
        "base_url": "https://api.openai.com/v1",
        "max_tokens": 8192,
        "temperature": 0.7,
        "timeout": 120
      }
    }
  },
  "env": {
    "TAVILY_API_KEY": "your-search-api-key"
  }
}
```

Important runtime surfaces:

```text
memento agent
memento agent -m "..."
memento doctor
memento verify
memento-gui
```

Built-in skills live under:

```text
tmp/repros/memento-skills/builtin/skills/
```

Core code surfaces:

```text
tmp/repros/memento-skills/core/memento_s/agent.py
tmp/repros/memento-skills/core/prompts/templates.py
tmp/repros/memento-skills/core/skill/gateway.py
tmp/repros/memento-skills/core/skill/retrieval/
tmp/repros/memento-skills/core/skill/execution/
tmp/repros/memento-skills/infra/memory/
tmp/repros/memento-skills/middleware/sandbox/
tmp/repros/memento-skills/middleware/storage/
```

The current repo is product-shaped: CLI, GUI, IM gateways, SQLite storage,
skill market, local sandbox, config migration, context compaction, and
background daemon pieces. We should isolate the paper benchmark loop from this
broader app surface.

### Prompt Surfaces

Runtime prompts:

```text
tmp/repros/memento-skills/core/prompts/templates.py
tmp/repros/memento-skills/core/skill/execution/prompts.py
tmp/repros/memento-skills/daemon/agent_profile/soul_prompts.py
tmp/repros/memento-skills/daemon/agent_profile/user_prompts.py
```

Paper appendix router prompt:

```text
tmp/skill_opt_sources/arx_2603.18743/full_source_flat.tex
section: Prompt for Synthetic Router Goals
```

The router prompt generates JSON with positive queries and hard negative
queries from:

```text
skill_name
description
keywords_block
existing_pos_block
existing_neg_block
```

### Benchmark / Dataset Requirements

Paper benchmarks:

- GAIA: 165 validation questions split into 100 train and 65 test.
- HLE: 788 train and 342 test, evenly sampled across 8 categories.
- Underlying LLM in paper: Gemini-3.1-Flash.
- GAIA run: max three reflective retries per question.
- HLE run: training rounds R0-R3, final test-set evaluation.
- Seed skills: paper describes starting from five atomic skills; the current
  repo has a larger built-in skill set.

Current local data state:

```text
GAIA: blocked, gated Hugging Face dataset
HLE: blocked, gated Hugging Face dataset
```

The blocked reports are under:

```text
tmp/paper_exact_samples/memento-skills/gaia/access_blocked.json
tmp/paper_exact_samples/memento-skills/hle/access_blocked.json
```

### Infrastructure We Need

- Isolated `HOME` or config root so `~/memento_s/config.json`, SQLite DBs,
  skill dirs, context dirs, and session dirs do not touch the user's real
  Memento state.
- Deterministic skill library seed set. Need decide whether to use paper's
  five atomic skills or the current repo's ten built-ins; the doc/paper and
  current repo diverge here.
- A benchmark runner that can feed GAIA/HLE examples through `memento agent -m`
  or the lower-level agent API and collect:
  - selected skill,
  - full prompt,
  - tool calls,
  - skill execution artifacts,
  - judge result,
  - reflected write mutation.
- A skill mutation journal: before/after skill folder, utility change, failed
  trace, generated test, test result.
- Router training/eval substrate:
  - local skill catalog,
  - synthetic positive/hard-negative query generation,
  - BM25 baseline,
  - embedding baseline,
  - Memento-Qwen or substitute router,
  - Recall@K computation,
  - end-to-end route hit / judge success checks.
- Web/search credentials for GAIA-like tasks: `TAVILY_API_KEY` or exact paper
  web-search provider equivalent.
- Dataset access approvals for GAIA and HLE.

### One-Sample Gate

Exact one-sample gate is blocked until at least one GAIA or HLE example is
available under the accepted license/auth state.

Do not substitute another dataset and call it exact.

Once data is available, one-sample gate should:

1. Create isolated Memento config/home.
2. Seed the exact starting skill library.
3. Run one train example through the read-execute-reflect-write loop.
4. Persist the selected skill and any mutation.
5. Run the same or paired held-out example after mutation.
6. Capture judge/scorer result and skill diff.

### Current Blockers / Non-Exactness

- GAIA and HLE are gated for current Hugging Face auth.
- The current local repo is newer/product-shaped and may not match the exact
  paper benchmark code path.
- Need confirm exact seed skill set, judge prompts, and benchmark scripts from
  repo or authors.
- Current previous lane correctly refused a surrogate live run.

## Paper 4: D2Skill

Paper: "Dynamic Dual-Granularity Skill Bank for Agentic RL"

### What The Paper Actually Optimizes

D2Skill is not a prompt-only or post-hoc skill editor. It co-evolves a policy
and a dual-granularity skill bank during agentic RL.

Core loop:

1. Roll out groups under the same policy.
2. Split into baseline group and skill-injected group.
3. Retrieve task-level and step-level skills during interaction.
4. Compare performance gap to estimate hindsight utility.
5. Reflect on poor trajectories to generate task and step skills.
6. Insert/deduplicate skills with retrieval keys.
7. Update utility and retrieval counts.
8. Prune redundant/low-utility skills.
9. Train policy with GRPO/verl-agent.
10. Validate with fixed skill bank, no reflection/update.

The infrastructure center is distributed RL plus live environment interaction,
not a standalone skill prompt.

### Concrete Upstream Setup

Base install:

```bash
cd tmp/repros/d2skill-agenticrl
pip install -r requirements.txt
pip install vllm==0.11.0
pip install flash-attn==2.7.4.post1 --no-build-isolation --no-cache-dir
pip install -e .
pip install openai
cp env.example env.sh
```

ALFWorld:

```bash
pip install alfworld
pip install gymnasium==0.29.1
pip install stable-baselines3==2.6.0
alfworld-download -f
```

WebShop:

```bash
cd agent_system/environments/env_package/webshop
./setup.sh -d all
```

Retrieval service must start first:

```bash
cd tmp/repros/d2skill-agenticrl
bash examples_d2skill/skill_retrieval_launch.sh
```

Training starts second:

```bash
bash examples_d2skill/run_alfworld_d2skill.sh
bash examples_d2skill/run_webshop_d2skill.sh
```

The ALFWorld script assumes:

- `env.sh` at repo root.
- vLLM engine by default.
- `Qwen/Qwen3-4B-Instruct-2507` actor model.
- 8 GPUs per node.
- Ray.
- train data size 16, val data size 128.
- group size 8.
- `trainer.total_epochs=160`.
- validation every 5.
- embedding retrieval service at `http://127.0.0.1:8003/retrieve_batch`.
- embedding model default path
  `/data/group/project3/project3_cluster3_data/hf_models/Qwen3-Embedding-0.6B`.

The paper reports 8xH100 wall-clock measurements.

### Prompt Surfaces

Environment prompts:

```text
tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/alfworld.py
tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/webshop.py
tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/search.py
tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/appworld.py
tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/gym_cards.py
tmp/repros/d2skill-agenticrl/agent_system/environments/prompts/sokoban.py
```

The ALFWorld prompt with memory includes:

- task description,
- recent observation/action history,
- current observation,
- admissible actions,
- retrieved task-level and step-level experiences,
- warning that lessons may be outdated,
- required `<think>` and `<action>` tags.

### Dataset / Environment Surfaces

Current real sample:

```text
tmp/paper_exact_samples/d2skill/alfworld/first_traj_data.json
tmp/paper_exact_samples/d2skill/alfworld/d2skill_alfworld_prompt_source.py
```

Full exact environment needs:

- ALFWorld TextWorld game files.
- MaskRCNN detector if full `alfworld-download -f` / visual stack is used.
- WebShop data and server for WebShop experiments.
- verl-agent / GiGPO training stack.
- vLLM or compatible rollout engine.
- Ray cluster resources.
- CUDA GPUs sufficient for model and retrieval service.

### Infrastructure We Need

- Separate service supervisor for embedding retrieval server and training job.
- Health check for retrieval endpoint before training.
- Run config capture of every Hydra override.
- Skill bank persistent store:
  - task skills,
  - step skills,
  - retrieval keys,
  - embeddings,
  - utility EMA,
  - retrieval counts,
  - eviction decisions.
- Rollout capture with baseline/skill group membership.
- Environment transcript capture for each step: task, observation, admissible
  actions, retrieved skills, action, reward/done.
- Reflection output capture when skill update triggers.
- Policy checkpoint capture from trainer.
- Validation mode that freezes skill bank and disables updates.

### One-Sample Gate

Full RL is too heavy for the first gate, but the gate cannot be a fake prompt
render. The smallest honest gate should exercise:

1. One ALFWorld environment instance.
2. One policy call with exact ALFWorld prompt template.
3. Retrieval service call, even if the skill bank is tiny/empty.
4. One baseline rollout and one skill-injected rollout under same model.
5. One utility update decision from the performance gap.
6. One skill generation/reflection path if failure threshold is met.

If we cannot run the policy/trainer on local hardware, the doc should mark this
lane as requiring remote GPU infra. A prompt-only preflight is useful but not
replication.

### Current Blockers / Non-Exactness

- Full setup expects 8 GPUs and H100-class runtime for reported scale.
- Local one-sample materialization has ALFWorld JSON but not verified live
  TextWorld/THOR environment execution.
- Retrieval service currently points at a lab-local embedding model path.
- `env.sh` must be created and filled.

## Paper 5: SkillReducer

Paper: "SkillReducer: Optimizing LLM Agent Skills for Token Efficiency"

### What The Paper Actually Optimizes

SkillReducer compresses existing skills while preserving routing and functional
quality. It has two major stages:

Stage 1: routing description optimization.

1. Segment description into semantic clauses.
2. Build candidate pool: target skill, four TF-IDF distractors, one LLM-generated
   adversarial shadow skill.
3. Run simulated routing oracle.
4. Apply ddmin to find 1-minimal routing description.
5. Shorten retained units.
6. Validate in real agent runtime by deploying the skill and checking whether
   Claude Code invokes it for the same queries.
7. Selectively restore deleted units if real trigger fails.

Stage 2: body restructuring through progressive disclosure.

1. Segment body into paragraph-level items.
2. Classify each item into core rule, background, example, template, redundant.
3. Keep core rules always loaded.
4. Move examples/templates/background to references.
5. Deduplicate overlap between body and references.
6. Annotate references with `when` and topics metadata.
7. Run faithfulness gate.
8. Run task-based evaluation under D/A/C conditions.
9. Promote missed content back into core if compressed skill regresses.

Conditions:

- D: no skill.
- A: original body and all references.
- C: compressed core and references available via `read_file`.

### Concrete Setup From Paper

The paper does not expose a local SkillReducer code repo in our current sources.
GitHub search did not find an obvious public implementation repo for this title.
Therefore the paper source is currently the primary implementation spec.

External benchmark used by paper:

```text
tmp/repros/skillsbench
```

SkillsBench setup:

```bash
cd tmp/repros/skillsbench
uv sync --locked
uv run bench tasks check tasks/<task-id>
uv run bench eval create -t tasks/<task-id> -a oracle
```

SkillsBench task structure includes:

```text
task.toml
instruction.md
environment/
tests/test.sh
tests/test_outputs.py
solution/
```

Current one-sample fixture:

```text
tmp/repros/skillsbench/tasks/jax-computing-basics/task.toml
tmp/repros/skillsbench/tasks/jax-computing-basics/instruction.md
tmp/repros/skillsbench/tasks/jax-computing-basics/environment/problem.json
tmp/repros/skillsbench/tasks/jax-computing-basics/tests/test_outputs.py
tmp/repros/skillsbench/.agents/skills/skill-creator/SKILL.md
```

The sample task declares:

```toml
[verifier]
timeout_sec = 600.0

[agent]
timeout_sec = 600.0

[environment]
build_timeout_sec = 600.0
cpus = 1
memory_mb = 2048
storage_mb = 10240
gpus = 0
allow_internet = true
```

### Prompt / Model Surfaces

Paper implementation details:

- Stage 1 uses DeepSeek-V3 for segmentation/compression/adversarial skill
  generation.
- Stage 1 uses DeepSeek-R1 as simulated routing oracle.
- Stage 1 real-trigger validation runs Claude Code CLI and parses stream events
  to detect skill invocation.
- Stage 2 uses DeepSeek-V3 for content classification, body compression,
  reference deduplication, and faithfulness verification.
- Task generation, agent execution, and evaluation use Qwen3.5 under a separate
  API key/session.
- Tokenization uses OpenAI `cl100k_base` via `tiktoken`.
- Cross-model eval includes Qwen3-max, DeepSeek-V3, Qwen2.5-7B, GLM-5, and
  GPT-OSS-120B.
- Independent framework eval uses OpenCode v1.2.27 with DeepSeek-V3 backend.

Exact prompt text for the SkillReducer pipeline is not exposed in the local
paper source. The paper gives algorithms and detailed stage behavior, but not
the full model prompts. This must be labeled prompt-non-exact unless we obtain
the authors' implementation or prompt appendix.

### Infrastructure We Need

- Skill parser that preserves:
  - YAML/frontmatter description,
  - body,
  - reference files,
  - scripts/assets.
- Token accounting with `cl100k_base`.
- Description semantic segmenter.
- Simulated router oracle with candidate-pool construction:
  - target,
  - four TF-IDF distractors,
  - one adversarial shadow skill.
- ddmin executor and validation cache.
- Real agent trigger harness:
  - deploy original/compressed skill into Claude Code skill dir,
  - issue query,
  - parse stream events for skill invocation,
  - compute trigger preservation.
- Body classifier and restructuring writer.
- Faithfulness checker.
- Task generator: five tasks per skill, mix core-only and needs-reference.
- D/A/C evaluator:
  - condition D no skill,
  - condition A full original skill,
  - condition C compressed core plus `read_file` references,
  - deterministic code execution checker where applicable,
  - LLM judge where rubric-based.
- Feedback loop that promotes missing content back to core and reruns gates.
- SkillsBench runner integration for external deterministic tasks.

### One-Sample Gate

Use the current SkillsBench task plus `skill-creator` skill fixture.

Smallest honest gate:

1. Parse original `skill-creator/SKILL.md`.
2. Run Stage 1 on the description with a small candidate pool.
3. Run a real trigger check in an actual agent runtime.
4. Run Stage 2 classification/restructure on the body.
5. Create compressed skill folder.
6. Run SkillsBench task under A and C.
7. Compare pass/fail and token counts.

If we do not have the exact SkillReducer prompts, the gate can be
infrastructure-faithful but prompt-non-exact.

### Current Blockers / Non-Exactness

- No local public SkillReducer implementation repo found.
- Exact prompts are not exposed locally.
- Need real Claude Code stream-event trigger parsing or a documented substitute.
- Previous tiny live lane used a local prompt, so it is not prompt-exact.

## What Leaven Needs To Provide After These Inventories

The common Leaven substrate should not start as "Recall@K" or any particular
paper's scoring function. It should start as runtime infrastructure that can host
the paper loops without deforming them:

1. Workspace materializer.
   - Create isolated run dirs.
   - Copy or mount upstream repo at pinned commit.
   - Set isolated HOME/config roots when tools use global state.
   - Initialize nested git repos when paper requires them.

2. Command/run supervisor.
   - Start background services.
   - Wait for health checks.
   - Run foreground jobs with env vars, timeouts, and logs.
   - Preserve stdout/stderr and process metadata.

3. Agent invocation recorder.
   - Capture model, provider, base URL, prompt, messages, tool calls, outputs,
     token usage, and cost.
   - Allow paper-specific harnesses to invoke models directly while Leaven
     observes.

4. VCS observer.
   - Snapshot branches/tags/diffs before and after each candidate.
   - Keep paper git separate from the outer `jj` workspace.
   - Expose branch lineage and artifact diffs as evidence.

5. Artifact store.
   - Store datasets, rendered prompts, traces, generated files, score reports,
     skill folders, and checkpoints by run id.
   - Hash large artifacts.

6. Evaluation adapter boundary.
   - Call paper-native scorers first.
   - Normalize only the final record shape after paper scoring has run.
   - Do not move paper-specific scorer logic into Leaven until at least several
     paper-native implementations prove a shared contract.

7. Mutation observer/writer boundary.
   - For papers that mutate files, capture file operations and resulting diffs.
   - Use paper-native writer where present.
   - Only abstract common "skill folder patch" APIs after Trace2Skill,
     EvoSkill, Memento, and SkillReducer all run in their real forms.

8. Dataset/sample manifest.
   - Pin sample ids and split role.
   - Keep one-sample smoke manifests separate from full-paper manifests.
   - Mark gated data honestly.

## Immediate Next Work

1. Cleanly separate the false-start Leaven P5 code edits from this doc work.
2. Initialize EvoSkill OfficeQA nested git repo in an isolated copy, not in the
   upstream checkout directly.
3. Install/smoke `evoskill` and run one OfficeQA sample through the real
   `evoskill run` loop.
4. Build Trace2Skill one-case runner around upstream scripts.
5. Resolve Memento GAIA/HLE access or leave it blocked without a surrogate.
6. Decide whether D2Skill one-sample requires remote GPU infrastructure.
7. For SkillReducer, either find/obtain the authors' implementation/prompts or
   proceed with an explicitly prompt-non-exact infrastructure reconstruction.

