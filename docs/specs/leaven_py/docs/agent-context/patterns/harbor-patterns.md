# Harbor Patterns — Leaven Python Scaffold Compatibility

**Date:** 2026-05-24  
**Vendored at:** `repos/harbor/` (`harbor-framework/harbor@main`)  
**Scope:** Containerized agent evaluation harness — task directories, trial/job
orchestration, verifiers — and how Leaven should interoperate via `lv.x.harbor.*`.

Harbor is the official harness for Terminal-Bench and hosts a registry of
third-party agent benchmarks (SWE-Bench, Aider Polyglot, etc.). It runs **agents**
(Claude Code, Codex CLI, OpenHands, …) inside **container environments**, then
executes **verifier scripts** to produce rewards.

**Identity confirmation:** This is `harbor-framework/harbor` (Apache-2.0, Terminal-Bench creators). It is **not**:

- `meridianlabs-ai/inspect_harbor` — Inspect AI eval registry adapter (already referenced inside vendored `inspect_ai`)
- Prime Intellect Verifiers — separate RL environment library (see `verifiers-patterns.md`; includes a `HarborTaskset` bridge)

---

## 1. What to read first

| File | Why |
|------|-----|
| `repos/harbor/README.md` | `harbor run` CLI, dataset@version syntax, cloud env providers. |
| `repos/harbor/AGENTS.md` | Task layout, agent list, architecture map for coding agents. |
| `repos/harbor/src/harbor/models/task/task.py` | Task directory contract (`instruction.md`, `task.toml`, tests, environment). |
| `repos/harbor/src/harbor/models/dataset/manifest.py` | `dataset.toml` — pinned task refs with content digests. |
| `repos/harbor/src/harbor/agents/base.py` | `BaseAgent` lifecycle: `setup()`, `run(instruction, environment, context)`. |
| `repos/harbor/src/harbor/verifier/verifier.py` | Post-agent test execution → reward file parsing. |
| `repos/harbor/src/harbor/trial/trial.py` | Single trial phases: env setup → agent → verifier. |
| `repos/harbor/examples/tasks/hello-world/` | Minimal task skeleton. |

---

## 2. What existing API code users bring

### Task directories (primary artifact)

A Harbor **task** is a filesystem tree:

```
my-task/
├── instruction.md      # Agent-facing prompt (may contain canary markers)
├── task.toml           # Timeouts, resources, MCP servers, metadata
├── environment/        # Dockerfile / compose — sandbox definition
├── solution/           # Oracle/reference (optional)
└── tests/              # Verifier scripts → reward.json or reward.txt
```

Users author tasks; they do not typically write Python `@decorator` scoring functions.
Scoring is **out-of-process**: verifier shell/python tests write reward files.

**Leaven mapping:**

| Harbor artifact | Leaven concept |
|-----------------|----------------|
| `instruction.md` | `case.input` (runner-visible) |
| hidden tests / oracle | `case.target` or opaque verifier handle in metadata |
| `task.toml` metadata | `case.metadata` (timeouts, difficulty, tags) |
| verifier reward | `@lv.scorer` result or external verifier stage output |

### Dataset manifests

Collections of tasks are declared in `dataset.toml` with pinned `org/name@sha256:...`
refs. Registry CLI: `harbor datasets list`, `harbor run -d "terminal-bench@2.0"`.

**Leaven mapping → `lv.cases.*`:**

- Do **not** embed Harbor registry catalog in Leaven.
- Provide `lv.x.harbor.cases_from_dataset("terminal-bench@2.0")` that downloads/caches
  tasks and emits a `CaseSet` with stable ids derived from task digest + name.
- Align with spec rule: generic loaders only; benchmark names live in user config.

### Agents (`BaseAgent`)

Built-in agents wrap external CLIs (Claude Code, Codex, etc.). Custom agents implement:

```python
class MyAgent(BaseAgent):
    @staticmethod
    def name() -> str: ...
    async def setup(self, environment: BaseEnvironment) -> None: ...
    async def run(self, instruction: str, environment: BaseEnvironment, context: AgentContext) -> None: ...
```

**Leaven mapping → `@lv.runner` / `@lv.agent`:**

Harbor agents are **full terminal agents**, not lightweight prompt→string runners.
Two compatibility tiers:

1. **Task-only import:** user brings Harbor tasks; Leaven supplies its own runner/scorer.
2. **Agent passthrough:** `lv.x.harbor.run_agent(agent_name, ...)` delegates to Harbor's
   agent inside Leaven-managed sandbox — heavy, optional.

Most Leaven optimizer users will tier (1): reuse benchmark tasks, not Harbor's CLI agents.

### Verifiers (Harbor sense)

Harbor `Verifier` uploads/runs `tests/` inside the container and parses
`reward.json` / `reward.txt`. This is **not** Prime Intellect `@vf.reward`.

**Leaven mapping → `@lv.scorer`:**

Implement `lv.x.harbor.verifier_as_scorer(task_dir)` that:

1. materializes workspace from Harbor `environment/`
2. runs agent output through Harbor verifier scripts (or reuses cached trial artifacts)
3. returns float reward as `lv.Assessment`

Keep verifier execution in sandbox boundary (`cx.sandbox`), not in reflector stages.

### Jobs and trials

- **Trial** = one agent run on one task (Harbor's "example").
- **Job** = YAML/`harbor run` config expanding agents × models × tasks with concurrency.

**Leaven mapping → `lv.optimize(...).run()`:**

| Harbor | Leaven |
|--------|--------|
| `JobConfig` | optimize config + environment builder |
| trial result JSON | run report / assessment rows |
| `n-concurrent` | engine worker pool (Rust-side) |
| job dir under `~/.cache/harbor/jobs` | durable run store (spec-owned layout) |

---

## 3. Dataset loading semantics

### Registry download path

Harbor resolves `dataset@version` via registry clients
(`src/harbor/registry/client/`), caches under user cache dir, validates content
digests from manifest.

**Adapter responsibilities:**

1. Resolve dataset ref → list of task paths.
2. For each task, read `instruction.md` (+ step instructions if multi-step).
3. Build `lv.Case`:
   - `id`: prefer `org/name@digest` or Harbor task name
   - `input`: stripped instruction (`Task.strip_canary`)
   - `target`: **do not** embed oracle solution in target; store verifier config path in metadata
   - `metadata`: difficulty, category, env resource limits, MCP configs

### Local task paths

`harbor run --path ./my-task` supports ad-hoc directories. Adapter:
`lv.x.harbor.cases_from_task_dir(path)` / `cases_from_tasks(root_glob)`.

### Multi-step tasks

`task.toml` may define `[[steps]]` with per-step instructions and verifiers.
Map to either:

- one Leaven case with step metadata list, or
- multiple cases sharing a parent id (evaluator groups by metadata)

Prefer explicit metadata [`harbor_steps`] until spec defines multi-step case law.

---

## 4. Task semantics vs Leaven stages

| Harbor phase | Leaven stage | Visibility |
|--------------|--------------|------------|
| Agent reads `instruction.md` | `@lv.runner` input | runner (+ agent builder) |
| Agent executes in container | `@lv.runner` + `cx.sandbox` | runner |
| Verifier runs tests | `@lv.scorer` or verifier hook | scorer only |
| Oracle `solution/` | never expose to reflector | target isolation |
| Trial metrics / trajectories | run report / evidence | evaluator |

**Reflector/proposer/judge:** Harbor has no first-class reflection loop. Leaven
agentic stages are **additive** when optimizing prompts/programs on Harbor tasks.

---

## 5. What Leaven must provide (`lv.x.harbor.*`)

```python
import leaven as lv

cases = lv.x.harbor.cases_from_dataset("terminal-bench@2.0", limit=10)

@lv.runner
async def run(case, cx):
    # Leaven runner — not Harbor CLI agent
    ...

@lv.scorer
async def score(output, case, cx):
    return lv.x.harbor.verify_trial(case, output, cx.sandbox)

lv.optimize(seed=...)
    .val(cases)
    .runner(run)
    .scorer(score)
    .run()
```

Recommended surfaces:

| API | Purpose |
|-----|---------|
| `lv.x.harbor.cases_from_dataset(ref)` | Registry → `CaseSet` |
| `lv.x.harbor.cases_from_task_dir(path)` | Local task tree → single case |
| `lv.x.harbor.case_from_task(task: harbor.Task)` | Lower-level helper |
| `lv.x.harbor.verify(task_meta, sandbox)` | Run Harbor verifier scripts → assessment |
| `lv.x.harbor.materialize_environment(case, cx.workspace)` | Build container spec from `environment/` |
| `lv.x.harbor.read_trial_result(path)` | Ingest existing Harbor job output into Leaven report |

Optional advanced:

| API | Purpose |
|-----|---------|
| `lv.x.harbor.agent_runner(name, model)` | Delegate to Harbor `BaseAgent` implementations |
| `lv.x.harbor.export_job_config(...)` | Emit Harbor YAML for cross-tool comparison |

---

## 6. Minimal glue beyond existing code

Users should keep Harbor task directories **unchanged**.

Leaven glue must:

1. Download/cache tasks via Harbor's own registry client (runtime dep on `harbor` package, not vendored source).
2. Map instructions → `lv.Case.input` with canary stripping parity.
3. Run verifier scripts in sandbox matching Harbor semantics (reward file paths, timeouts from `task.toml`).
4. Never copy oracle solutions into reflector-visible fields.

Users **should not** need to rewrite tasks into JSONL unless they want a static snapshot
(`lv.cases.from_jsonl` after one-time export).

---

## 7. Anti-patterns / what NOT to steal

| Do not import | Why |
|---------------|-----|
| Harbor CLI job orchestration as Leaven core | Leaven engine owns run graph; optional export only. |
| Built-in CLI agents as default Leaven runner | Couples product to third-party CLIs; keep opt-in. |
| Harbor viewer / Supabase upload stack | Operational tooling, not optimizer seam. |
| Entire registry catalog into repo | Violates spec ("don't bundle benchmark catalogs"). |
| Docker/Modal/Daytona provider matrix | Workspace/sandbox backends are Leaven-owned (`lv.sandbox`, `leaven-workspace-*`). |
| `inspect_harbor` task names | Different package; use Inspect integration separately. |

**Container assumptions:** Harbor tasks assume Docker (or cloud env providers).
Leaven adapters must declare sandbox capability requirements upfront — do not
 pretend a Harbor task runs without container support.

---

## 8. Cross-ecosystem relationships

```
Harbor task dir ──► Verifiers HarborTaskset ──► vf.Env rollout
       │                                        │
       └──────► lv.x.harbor.cases_* ──► lv.Case ──► lv.optimize()
                           │
                           └──► lv.x.verifiers.* (when env wraps Harbor)
```

Inspect AI lists Harbor benchmarks via `inspect_harbor/*` codes in vendored
`inspect_ai/docs/evals/`. Leaven should treat that as **Inspect registry metadata**,
not as a substitute for reading Harbor task layout from this repo.

---

## 9. Surprises

1. **"Verifier" overload** — Harbor verifier = test script executor; Prime Verifiers = reward library. Adapter docs must name which sense applies.
2. **Reward file contract** — Verifier writes `reward.json` or plain text float; parser errors are typed (`RewardFileEmptyError`, etc.). Leaven scorers should surface the same failure modes.
3. **Canary strings** — Instructions embed provenance markers stripped before agents see them; Leaven import must call equivalent stripping when building `case.input`.
4. **ATIF trajectories** — Some agents emit Harbor trajectory format; useful for evidence store, not required for optimizer v1.
5. **Task digest pinning** — Dataset manifests hash-pin tasks; Leaven case ids should incorporate digest for cache/resume parity (aligns with `aime_case_report_adapter.md` identity themes).

---

## 10. Recommended next adapter targets

| Priority | Surface | Purpose |
|----------|---------|---------|
| P0 | `lv.x.harbor.cases_from_task_dir` | Local task → `CaseSet` (smallest proof) |
| P0 | `lv.x.harbor.verify` | Verifier scripts → `Assessment` |
| P1 | `lv.x.harbor.cases_from_dataset` | Registry ref → cases (runtime `harbor` dep) |
| P1 | `lv.x.harbor.materialize_environment` | `environment/` → workspace/sandbox spec |
| P2 | `lv.x.harbor.agent_runner` | Opt-in CLI agent delegation |
| P2 | `lv.x.harbor.import_trial_results` | Compare against native Harbor job output |

---

**Last updated:** 2026-05-24  
**Confidence:** High on task layout and verifier contract; medium on agent passthrough scope.
