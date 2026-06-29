# Leaven

Leaven optimizes things agents can change and measure: prompts, skills,
`AGENTS.md` and `CLAUDE.md` kits, harnesses, repos, and environments.

Bring a seed artifact, a task or harness, a rollout, a reward/rubric, and an
optimizer. Leaven runs the loop: select, propose, evaluate, keep what worked,
and preserve the evidence.

The first-time user surface is Python from this source checkout. The engine,
optimizer contracts, and durable run machinery are Rust underneath.

## Start Here

Run the no-spend GEPA example first. It uses the real `lv.optimize(...).run()`
path over the durable seam, with a mock LM reflector, and asserts that the
optimized prompt beats the seed.

```bash
git clone https://github.com/delightful-ai/leaven.git
cd leaven/sdk/python
uv sync
just example 03
```

For the full Python example tour:

```bash
just examples
```

Live provider and Harbor examples are skipped by default unless you set their
explicit opt-in environment variables.

## Pick Your Path

| Want to... | Start here | What it proves |
| --- | --- | --- |
| Run GEPA with no spend | [`sdk/python/examples/03_prompt_optimize.py`](sdk/python/examples/03_prompt_optimize.py) | Real prompt optimization through `lv.optimize(...).run()` with mock LM reflection. |
| Run live AIME GEPA | [`sdk/python/examples/14_live_optimize_aime.py`](sdk/python/examples/14_live_optimize_aime.py) | Live OpenAI solver/reflection path, bounded by metric calls. |
| Optimize a Codex agent kit through Harbor | [`sdk/python/examples/15_live_optimize_codex_terminal_bench.py`](sdk/python/examples/15_live_optimize_codex_terminal_bench.py) | Served Harbor/Codex path on one pinned Terminal-Bench-2 task. |
| Run Claude Code through Harbor | [`sdk/python/examples/codex_terminal_bench/live_claude_code_trial.py`](sdk/python/examples/codex_terminal_bench/live_claude_code_trial.py) | One live Harbor trial through the generic agent-kit adapter. |
| Understand Harbor integration | [`docs/specs/harbor_leaven_adapter.md`](docs/specs/harbor_leaven_adapter.md) | How Harbor tasks, trials, verifier output, and trajectories map into Leaven. |
| Author a new optimizer | [`crates/leaven-engine/src/stage/optimizer.rs`](crates/leaven-engine/src/stage/optimizer.rs), [`crates/leaven-gepa`](crates/leaven-gepa) | The Rust optimizer trait and GEPA as the reference implementation shape. |
| Check example maturity | [`sdk/python/examples/README.md`](sdk/python/examples/README.md), [`examples/AGENTS.md`](examples/AGENTS.md) | Which examples are product proof, mechanics proof, live proof, or scaffold. |

## The Shape

In Python, a Leaven program is:

```text
seed x environment x optimizer x runtime
```

The environment contains:

- `Task`: cases, splits, and inputs
- `Rollout`: how the current artifact runs on one case
- `Rubric`: how outputs become reward/evidence

The optimizer chooses what to try next. Today the Python front door configures
GEPA:

```python
result = await lv.optimize(
    seed=lv.PromptArtifact(template="You are a calculator. Always answer 0."),
    environment=lv.Environment(
        task=lv.Task(cases=cases),
        rollout=lv.Rollout.fn(run),
        rubric=lv.Rubric([exact, concise]),
    ),
    optimizer=lv.optimizers.gepa(population_size=2, minibatch_size=1),
    runtime=lv.runtime.local(
        budget=lv.budget(metric_calls=4),
        lm=lv.lm.mock(responses=[reflected_template]),
    ),
).run()
```

That compact shape is the load-bearing product path: Leaven owns the search
loop and run graph; you own the artifact, rollout, and reward.

## Harbor Compatibility

Harbor turns an agent plus a task harness into a real trial. Leaven can use that
trial as a rollout.

That means Harbor-compatible agents can be optimized through Leaven as agent
kits: system prompts, repo instructions, skills, and harness scaffolding. The
current checkout includes proofs for:

- **Codex**: materializes a kit as in-container `AGENTS.md` plus skills, runs
  one Harbor Trial per rollout, and preserves verifier reward, CTRF, token/cost
  totals, verifier output, and trajectory paths as structured evidence.
- **Claude Code**: materializes a kit as `CLAUDE.md` plus `.claude/skills`,
  runs a live Harbor trial through the same generic adapter, and prints the
  structured `HarborTrialOutcome`.
- **Other Harbor agents**: the adapter shape is `lv.x.harbor.rollout.agent_kit(...)`;
  agents that Harbor can launch can be routed through this path with an agent
  descriptor/configuration.

Run the live Codex Terminal-Bench proof from `sdk/python/`:

```bash
LEAVEN_CODEX_LIVE=1 \
  uv run --project examples/codex_terminal_bench codex-terminal-bench
```

Run the live Claude Code Harbor smoke from the repo root:

```bash
LEAVEN_LIVE_CLAUDE_CODE=1 \
  uv run --project sdk/python/examples/codex_terminal_bench \
  python sdk/python/examples/codex_terminal_bench/live_claude_code_trial.py
```

Live Harbor runs are intentionally opt-in. They may require Docker, provider
credentials, agent CLI credentials, and real spend. The live Codex example
proves the served path works end to end; its deterministic no-spend test proves
changed-child mechanics. It does not claim broad Terminal-Bench coverage or a
live child that strictly beats the seed on every task.

## Run GEPA

The simplest GEPA run is the Python no-spend example:

```bash
cd sdk/python
just example 03
```

For a Rust builder example:

```bash
cargo run -p p8_aime_gepa
```

For live AIME GEPA with OpenAI, materialize the cache and opt in:

```bash
uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py \
  --out target/leaven-aime-cache/aime.json

export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

The deterministic Rust path is mechanics/API proof, not benchmark evidence. The
live path is opt-in and bounded by the example's metric-call and provider
settings.

## Author Your Own Optimizer

Python users configure optimizers today. Optimizer authors start in Rust.

The core contract is:

```rust
pub trait Optimizer<P: OptimizationProblem>: Send {
    async fn initialize(&mut self, ctx: &mut RunContext<'_, P>) -> Result<(), OptimizerError>;
    async fn step(&mut self, ctx: &mut RunContext<'_, P>) -> Result<StepStatus, OptimizerError>;
    fn best_candidate(&self, graph: RunGraphView<'_, P>) -> Option<CandidateId>;
}
```

Start with:

- [`crates/leaven-engine/src/stage/optimizer.rs`](crates/leaven-engine/src/stage/optimizer.rs)
  for the optimizer trait.
- [`RunContext`](crates/leaven-engine/src/context/run_context.rs) for graph
  mutation, evaluation, proposals, budgets, and receipts.
- [`crates/leaven-gepa`](crates/leaven-gepa) for a behavior-bearing optimizer
  crate to copy structurally.
- [`docs/specs/initial_library.md`](docs/specs/initial_library.md) for the
  artifact/evidence/optimizer model.

New optimizer crates should be behavior-bearing: real strategy state, tests,
topology coverage, and a seam-backed path if they are exposed to Python.

## Point A Coding Agent Here

Paste this into a coding agent after it clones the repo:

```text
You are working in the Leaven repo from source.

Choose one lane:
- Run GEPA: read sdk/python/examples/03_prompt_optimize.py and run
  `cd sdk/python && uv sync && just example 03`.
- Optimize a Harbor harness: read sdk/python/examples/README.md example 15 and
  docs/specs/harbor_leaven_adapter.md.
- Try Codex or Claude Code through Harbor: inspect sdk/python/examples/codex_terminal_bench/.
- Author an optimizer: read crates/leaven-engine/src/stage/optimizer.rs and
  crates/leaven-gepa/.

Rules:
- Do not cite scaffold examples as product proof.
- Prefer deterministic no-spend examples before live provider runs.
- Live provider, Harbor, Codex, and Claude paths require explicit env vars and
  may spend money.
- Python's product shape is seed x environment x optimizer x runtime.
- The durable public seam path is `leaven seam serve --stdio --config`.
```

## Status

Leaven is early alpha.

- The Python SDK is real in this repo, but this README does not promise a PyPI
  release yet.
- The Rust crates are real in this workspace, but this README does not promise
  crates.io availability yet.
- Example proof classes matter. Some examples are real product optimization,
  some are live-gated provider proofs, and some are scaffolded API shape.
- Live examples require explicit opt-in and may use Docker, provider APIs, or
  agent CLIs.

When in doubt, trust the proof label in
[`sdk/python/examples/README.md`](sdk/python/examples/README.md).

## Specs And References

- [`docs/specs/leaven_python.md`](docs/specs/leaven_python.md) - Python product
  shape and `lv.optimize(...).run()`.
- [`docs/specs/harbor_leaven_adapter.md`](docs/specs/harbor_leaven_adapter.md) -
  Harbor task/trial/evidence mapping.
- [`docs/specs/initial_library.md`](docs/specs/initial_library.md) - core Rust
  library model.
- [`docs/specs/guiding_principles.md`](docs/specs/guiding_principles.md) -
  product constraints and design principles.
- [`docs/testing/README.md`](docs/testing/README.md) - test contract and proof
  model.

## License

MIT or Apache-2.0, at your option.
