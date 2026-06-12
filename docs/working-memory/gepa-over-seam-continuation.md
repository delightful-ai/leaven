# GEPA over the seam — progress + continuation (2026-06-10)

Status: slices 1–4 DONE and verified (slice 4 = AIME live cutoff MET, see below).
Slice 5 (codex-kit on terminal-bench) implementation DONE (Rust host stages
5A-i/ii + Python SDK/harbor stage 5B); see "Slice 5B landed" and the Slice 5
live evidence below for the live-cutoff status. Design doc (amended,
authoritative): `docs/plans/2026-06-10-gepa-over-seam-design.md`.

## Goal

`lv.optimize(seed, environment, optimizer, runtime).run()` drives the REAL
`leaven-gepa` loop over the durable public seam. Two product goals, both
through the Python SDK:
- (A) optimize AIME (shrunk p8 scale; live gpt-4.1-mini solver,
  gpt-5.4-mini reflection, OpenAI direct).
- (B) optimize a Codex AgentKit on ONE terminal-bench-2 task (`regex-log`,
  n=1) via harbor, live Codex gpt-5.4-mini agentic reflection.

Cutoff per goal (darin, locked): first optimizer iteration where GEPA authors
a CHANGED artifact, APPLIES it through RunContext, and the child is
RE-EVALUATED onto the frontier. Apply-without-re-eval does not count.
First-class requirements: agentic materialization/reflection with real traces;
full instrumentation (events, receipts, costs) durable and inspectable.

## Landed commits (stack on `public-seam-production-closeout`)

- `ututrvql` design doc; `skmsqswk` amendment (host = configured composition
  in seam-service, NO new crate — leaven-run's builder IS the loop, p8-style).
- `zsuwlkzo` public-seam: scorer stage dispatch. `StageRunKind::Scorer` (wire
  "scorer", reuses locked `ScoreContext` payload role), typed
  `StageScoreFact{value, rewards[]}` / `StageRewardFact{id,value,weight,feedback?}`,
  laws: scorer results MUST carry score, runner/proposer MUST NOT; finite only.
- `toqxppwk` public-seam: `leaven/optimize.run` locked method. ALL=26,
  `WORKER_PROFILE`=25 (client→host direction split; profile advertises 25;
  runtime special-cases optimize.run before the profile gate). ArtifactRecord
  triple `{artifact_type, artifact_schema, artifact}` (prompt: `{template}`).
  Round-1 review caught+fixed a profile-scraper authorization leak.
- `pkyvxvpz` seam-service: `optimize_run_service` module — the host. Real GEPA
  via leaven-run builder + `Gepa::reflect_with_lm(configured LM, request model)`;
  runner/scorer worker-dispatching closures over the existing CommandRunner
  machinery; `case.target` REFUSED during runner stages, served+receipted
  during scorer stages; tokio current-thread runtime + scoped-thread worker LM
  callbacks (round-1 review caught: OpenAI reflection panics under plain
  block_on); evaluation_parallelism(1); new dep edges seam-service →
  leaven-gepa, leaven-eval, leaven-surface. Loop-law test:
  `optimize_run_drives_the_real_gepa_loop_to_a_changed_re_evaluated_child`
  asserts metric_calls_used == 8 EXACTLY (1 seed val + 3 parent screen + 3
  child screen + 1 child val). Reward feedback reaches reflection via
  Score.feedback → CaseAssessmentEvidence → GepaReflectiveDataset (Score has
  NO metrics channel; vector rides Score.trace — revisit if SDK needs
  per-reward result readback).
- `kvsyypnk` matrix row `ps1.optimize.run_dispatch` promoted to PROVEN via
  mutation-tested closeout review
  `docs/specs/public-seam-v1/reviews/2026-06-10-optimize-run-dispatch-loop-law-review.md`
  (both fixture mutations performed live, both failed the test). Added
  "unavailable" to the denial vocabulary (support.rs); updated proven-set
  fixture in contract_package.rs.
- `vkwvyolk` gepa knobs (darin: pool-cap semantics): population_size →
  `max_candidates` candidate-pool cap (graph-truth counter
  `graph().candidate_count()`, new engine `StopReason::CandidateCapReached`,
  service law ≥2), minibatch_size → `train_minibatch_size` override
  (order-independent vs `with_profile`). Absent knobs = bit-for-bit reference.
- `szltsvnp` usd cost-axis ceiling: optional `max_cost_usd_micro` on the wire
  optimizer config → engine Budget `usd_micro` axis; host test proves the
  ceiling stops the loop.
- `znkmsspl` python-sdk cutover: ONE `optimize.run` request (hand-authored
  msgspec records `_seam/optimize_run.py`, regenerated wire records fixing the
  stale stage_run fingerprint da2d026c→9f5df7a8); worker serves scorer stages
  (`_seam_worker/scorer.py`: gated+receipted `case.*` callbacks, exact
  weighted-mean aggregation); mechanics path DELETED (run_prompt_mechanics,
  receipts/scoring/rewards/status.py, _runs/rust_checkpoint.py + 8 test
  files); `lv.budget(metric_calls=)` required for gepa, `usd=` →
  max_cost_usd_micro, ALL unrouted gepa()/budget axes refused loudly
  (frontier, parent_selector, max_iterations, reflect, propose, calls,
  lm_tokens, wall_seconds, concurrent_calls); `result.assessments()` raises
  typed `AssessmentsUnavailableError` (per-case assessments NOT readable from
  optimize.run durable checkpoints — known gap); UNIQUE run ids
  (`<slug>_<time_ns hex>_<token_hex>`) fixing a stale-checkpoint RESUME HAZARD
  (constant run dir silently resumed old state, masking regressions);
  example 03 is REAL: seed 0.091 → best 1.000, reflection authors
  `Solve this arithmetic problem: {question}. Output only the integer.`,
  tour test pins improvement (fails on regression to seed-echo).

Verified personally (2026-06-10): example 03 improves; 12-example tour green;
310 SDK pytest; Rust: public-seam 301, seam-service 47, gepa 27, runtime 12,
topology 8; fmt/clippy clean. `just check`/`just coverage` NOT run since the
closeout gate — run `just release-check` before declaring the goal closed.

## Working method (converged, keep it)

Per-slice background Workflow (darin opted into ultracode; NO codex
implementer — darin explicitly declined; NO worktrees — canonical jj repo):
ONE opus implementer with pinned contract decisions + GROUND-marked
resolutions ("resolve from code, report what you chose; if a decision can't be
honored, blocker + stop, never improvise different semantics"), then parallel
adversarial opus reviewers — spec-compliance (re-runs gates, distrusts report)
+ code-quality (repo-standards craft) + a specialized third where the claim
warrants it (loop-law mutation reviewer; product-truth reviewer that RUNS the
examples and does real mutation probes with jj restore). Fixer folds via
`jj squash`; ≤3 rounds. Each slice = one coherent jj commit (describe + new).
Scripts to copy: `~/.claude/projects/-Users-darin-src-personal-leaven/aa24e439-1349-4b61-8810-4251cf0f15b5/workflows/scripts/slice{1,2,2-5,3}*.js`.

## Wire/API quick reference

Request: `{run_id, seed:{artifact_type:"prompt", artifact_schema, artifact:{template}},
cases:[{case, input, target, metadata?, split?}], optimizer:{max_metric_calls,
max_cost_usd_micro?, population_size?, minibatch_size?, objective},
reflection:{kind:"lm", model}|{kind:"agentic"}, capability_fingerprint}`.
Result: `{best, frontier[], iterations, metric_calls_used, cost,
run:{run, revision}, applied_proposals[]}`. Laws: best ∈ frontier; only
objective="instance" executes (others schema-valid, service-refused);
reflection kind="agentic" currently refused (slice 5 wires it);
population_size ≥ 2. Python: `lv.budget(metric_calls=N, usd=X)`,
`lv.runtime.local(lm=..., budget=...)`, `gepa(population_size=, minibatch_size=,
reflection_lm=?)`.

## Slice 4 — AIME (task #4, in_progress, NOT yet dispatched)

Plan (grounded; dispatch as the usual workflow):
- Data: reuse `examples/p8_aime_gepa/scripts/materialize_hf_aime.py`
  (AI-MO/aimo-validation-aime → cache JSON outside repo; MathArena/aime_2025
  is held-out). New live-gated SDK example (pattern of examples 10–13: skip
  without env gate) loading ~10 train cases; skip with actionable message if
  cache missing. Mock-first: deterministic AIME-shaped fixture test proving
  the example mechanics without network.
- Rollout: `@lv.runner` → `cx.lm.complete` with the prompt template; live
  OpenAI **gpt-4.1-mini** via host `SeamLmConfig::OpenAi` (key from repo
  `.env`, `set -a; source .env`). GROUND: how the per-call model is chosen —
  solver model from runtime LM config vs reflection model from request
  `reflection.model` (**gpt-5.4-mini**) through the same provider; verify the
  host honors per-request model override or fix at the owning layer.
- Rubric: exact integer match with p8-style answer normalization
  ("42"/"042"/"42.0"; answers 0–999).
- Seed prompt: take p8's seed for parity (weak enough for headroom).
- Budget: `metric_calls≈30`, `usd` ceiling (~$10) — darin pre-approved spend.
- Cutoff proof: result shows ≥1 admitted child ≠ seed with score > seed,
  applied_proposals non-empty. Record exact command + env + output + run dir
  (p8 precedent: `.leaven/release-runs/`) in this file when done.
- Carry-forward to decide: per-case `assessments()` unavailable for
  optimize.run runs — fine for cutoff (frontier scores suffice)?

## Slice 5 — codex-kit on terminal-bench via harbor (task #5, pending)

Plan (grounded by scout 2026-06-10; refresh before implementing):
- Harbor = terminal-bench 2.x harness. Installed `harbor` uv tool v0.1.43 is
  STALE → `uv tool install --force 'harbor==0.13.1'` and pin `harbor==0.13.1`
  in the example package; vendored clone
  `~/vendor/github.com/laude-institute/harbor` is a month stale → `git pull`
  before subclassing. Docker daemon works on this Mac; oracle trial verified
  live in 40s (`harbor trial start -p examples/tasks/hello-world --agent oracle`).
- Rollout primitive: `Trial` (Python API: TrialConfig/TaskConfig/AgentConfig →
  `(await Trial.create(cfg)).run()` → `result.verifier_result.rewards["reward"]`,
  ctrf.json for per-test partial credit, agent tokens/cost, trajectory.json
  ATIF transcript). Task: **regex-log** (terminal-bench-2 git-pinned;
  alternates: log-summary-date-ranges, fix-git). n=1.
- Codex adapter: built-in `codex` agent installs @openai/codex in-container,
  `-m openai/gpt-5.4-mini`, `prompt_template_path` kwarg = evolvable wrapper
  (Jinja with `{{ instruction }}`). AgentKit channel: `LeavenCodex(Codex)`
  subclass (via AgentConfig import_path) with `agent_kit_dir` kwarg that
  `environment.upload_file()`s AGENTS.md + skills into the WORKDIR before
  `codex exec` (codex reads AGENTS.md from cwd natively).
- Leaven side (PREMISE CORRECTED 2026-06-11: there is NO flat-content
  AgentKitArtifact — the real AgentKit IS a Git revision, `GitProgramArtifact`;
  `leaven-artifact-agent-kit` owns only manifest/profile/path vocabulary; the
  real reflector is `leaven-gepa-agentic-git::GepaGitProgramAgenticReflector`
  composing AgenticProposer + CodexAgentKitMaterializer, and its renderer
  already pours run.feedback into the agent instructions +
  `.leaven/gepa-reflection.md`): the wire record {system_prompt,
  skills:[{path,content}]} is a PROJECTION; the host constructs a real
  run-scoped bare Git repo + seed commit from wire content at setup, runs the
  loop over GitProgramArtifact, and reads child revisions back to flat parts
  for payloads/results. Deterministic proof uses
  `leaven_agent::test_support::FakeAgentRuntime` (reuse pattern:
  leaven-gepa-agentic-git test `reflector_wrapper_runs_agentic_proposer_...`).
  optimize_run_service is SeamPromptArtifact-monomorphic (27 refs / 6 modules)
  — genericize or parallel path. Wires `reflection:{kind:"agentic"}`. Reflection MUST consume
  real rollout traces: harbor trajectory + verifier output projected into the
  reflective workspace (darin: "agentic materialization/reflection rather
  important"). Boundary: harbor stays INSIDE the Python rollout fn (its spend
  reported back as rollout evidence, not capability-gated — accepted V1
  caveat, document at owning surface); harbor is NOT a leaven-workspace
  backend.
- Deterministic kit-evolution proof first (scripted agent, no docker/live),
  then live cutoff on regex-log.
- TB2 canary GUID: do not fold task internals/solutions into stored corpora.

### Slice 5B landed (2026-06-11) — SDK kit authoring + harbor + cutoffs

Stages 5A-i/ii (Rust host) were already in the parent commits. 5B is the
Python-SDK + harbor-example slice (Rust untouched):

- SDK agent-kit artifact: `leaven.AgentKitArtifact` (`system_prompt`, `skills:
  [AgentKitSkill(path, content)]`, `candidate_id`) projecting to/from the locked
  `agent_kit` wire body. `sdk/python/src/leaven/artifacts/agent_kit.py`,
  exported top-level.
- Generalized the SeamPromptArtifact-monomorphic optimize path to dispatch by
  artifact type via the new single-owner
  `_seam_optimize/artifact_projection.py` (`project_seed` → wire record +
  reflection kind; `artifact_from_record` → typed candidate). The driver now
  wires the host agent runtime for the agentic kit path: `gepa(reflection_agent=
  lv.agent.codex(transport="cli", model=..., bin_path_env=...))` lowers to a
  `CodexCliRuntimeConfig` in the served `SeamServiceConfig`. Reflection-kind x
  artifact-type matrix enforced at lowering (kit requires agentic + agent;
  prompt refuses an agent). The worker runner reconstructs the typed artifact
  from `candidate_agent_kit` or `candidate_template`; the scorer is unchanged
  (artifact-agnostic). Result projection dispatches the candidate artifact by
  `artifact_type`. Null targets now supported (rollout-judged tasks have no held
  answer).
- Harbor rollout package `sdk/python/examples/codex_terminal_bench/` (own uv
  project, `harbor==0.13.1`): `LeavenCodex(Codex)` uploads AGENTS.md (from
  system_prompt) + skills into `/app` before `super().run`; `@lv.runner` runs
  ONE git-pinned regex-log Trial (`terminal-bench-2` @
  `2fd12b88aafdd04a52c298e3940bcb189f9766d6`, path `regex-log`, image
  `alexgshaw/regex-log:20251031`) and serializes reward + CTRF fraction + tokens
  + trajectory; rubric = verifier reward (w=1) + CTRF fraction (w=0.25), feedback
  = verifier output + agent-own-behavior trajectory excerpts (no task
  solutions). Seed kit is deliberately weak-but-honest. Oracle smoke verified the
  git-pinned task downloads, container builds, verifier writes reward+ctrf.
- Reachability (decision 3, CONFIRMED): the Rust `FakeAgentRuntime` is
  `#[cfg(test)]`-only and NOT reachable through the served-CLI `SeamServiceConfig`
  (only `SeamAgentConfig::None | CodexCli`). So the served path's only
  deterministic agentic option is a SCRIPTED CODEX BINARY. The no-spend proof
  `examples/codex_terminal_bench/tests/test_kit_optimization_mechanics.py` drives
  the REAL served optimize path (real GEPA loop, real Git-backed kit
  materialization/readback, real agentic reflection) with two no-spend
  substitutions: a fake-codex binary that rewrites
  `repos/agent_kit/system_prompt.md` (the deepest deterministic agentic cut) and
  an explicit env-gated fake-trial seam (`LEAVEN_CODEX_TB_FAKE_TRIAL`, since the
  rollout runs in a worker subprocess where the live `Trial` can't be
  monkeypatched). It asserts the kit child is applied + re-evaluated onto the
  frontier beating the seed; it fails if not.
- Live example: `sdk/python/examples/15_live_optimize_codex_terminal_bench.py`
  self-skips without `LEAVEN_CODEX_LIVE=1` + Docker + `OPENAI_API_KEY`, else
  delegates to the harbor project's `codex-terminal-bench` console script.
- Gates: sdk `just check` green; `uv run pytest` 333 passed; `just examples` (14;
  15 self-skips); `just compile-examples` ok; harbor project ruff/ty/pytest (9)
  green. Live cutoff evidence is in "Slice 5 live evidence" below.

## Other carry-forwards

- `leaven-acp-stage-bridge` + `leaven-cli serve.rs` (legacy demo loop) are now
  fully superseded by the real path — candidate for a cleanup slice (hard
  cutover removal) after the goal closes.
- Promotion candidate: SeamPromptArtifact → a real `leaven-artifact-prompt`
  crate once a second consumer exists.
- p8 example itself could later route through the seam host (paper-scale flag
  on the same code path) — not in scope for the cutoff.

## Slice 4 live evidence (2026-06-11) — AIME cutoff MET

Slice 4 landed: `sdk/python/examples/14_live_optimize_aime.py` optimizes a real
AIME solver instruction live through `lv.optimize(...).run()` over the durable
seam, with gpt-4.1-mini solver and gpt-5.4-mini reflection both served by one
`SeamLmConfig::OpenAi` provider.

Decision-1 grounding (the open risk): NO Rust host fix needed. The system already
threads per-call models correctly. `SeamLmConfig::OpenAi` carries NO model field;
the model is required per request. Solver: the worker's `cx.lm.complete` ships
`model=<runtime LM>` (gpt-4.1-mini) and the OpenAI provider uses
`request.model.as_str()` (`leaven-lm-openai/src/client.rs:59`). Reflection:
`Gepa::reflect_with_lm(configured_lm, reflection_model)` stores the wire
`reflection.model` (gpt-5.4-mini) and the reflection `LmRequest::new(input.model, ...)`
(`leaven-gepa/src/reflection.rs:105`) uses it. Slice 4 is one commit, Python-only.

Owning-layer fixes found while making the live path real (all Python SDK):
- `_seam/lm_plans.py`: the worker hardcoded `LmOutputFinalMessage(max_bytes=512)`,
  refusing any reasoning-length solver response. Now sized from `max_tokens`
  (`max_tokens * 8` bytes), so a reasoning runner's response is not refused.
- `_seam_optimize/driver.py`: the optimize client timeout was hardcoded 600s with
  no override; a live GEPA run (sequential solves + slow reasoning reflection)
  exceeds it. Added `LEAVEN_OPTIMIZE_TIMEOUT_S` operator override (default 600).
- Cost truth: Leaven meters TOKENS, not USD (`TokenUsage::to_cost` has no dollar
  field; no model-pricing table). So `total_cost_usd` is 0.0 and the `usd`
  ceiling never bites; the run is bounded by `metric_calls`. Documented at the
  example/README/method-status. A pricing table is a separate future slice.

Live cutoff run (sanctioned spend, recorded):
- Command: from `sdk/python`, `set -a; source ../../.env; set +a;
  LEAVEN_LIVE_OPENAI=1 LEAVEN_OPTIMIZE_TIMEOUT_S=2400
  LEAVEN_RUNS_ROOT=.leaven/release-runs uv run python examples/14_live_optimize_aime.py`
- Run dir: `.leaven/release-runs/run_aime_gepa_18b7fd7b048102e8_8777b0a2`
  (run id `run_aime_gepa_18b7fd7b048102e8_8777b0a2`).
- Result: seed score 0.000 -> best score 1.000, improved=True, iterations=3,
  metric_calls_used=12 (<= 30), lm_tokens=22422, cost_status=known,
  total_cost_usd=0.0 (token metering only).
- Durable evidence (per-case): seed validation 0/2 (cases 7,11); parent screen
  0/2 (train 0,1); child-1 screen 0/2 (rejected); child-2 screen 2/2 (242,227 ->
  beats parent -> ADMITTED); admitted child re-validation 2/2 (73,104 -> beats
  seed). This is the cutoff: a CHANGED child, APPLIED through RunContext,
  RE-EVALUATED onto the frontier, beating the seed.
- Seed instruction: "Respond with only your immediate best-guess integer. Do not
  calculate or show any working." Optimized (gpt-5.4-mini reflection) instruction:
  a detailed solver brief that forbids guessing and requires explicit
  step-by-step working and verification before the final integer.

Reliability levers used (all faithful, recorded honestly):
- Weak guess-only seed (reliably fails) vs reasoning child (reliably solves) at
  solver temperature 0.3 (temp 1.0 made the seed solve by luck and flip-flop).
- Curated real AIME train rows (indices 0,1 train; 7,11 validation) where
  gpt-4.1-mini's success depends on the prompt -- the optimization is genuine
  (reflection must discover reasoning helps); curation only de-flakes the demo.
- Informative per-case scorer feedback (RewardValue.feedback) that names the
  failure mode WITHOUT leaking the target, so reflection improves the
  generalizable instruction instead of memorizing answers.
- Runner injects the problem + answer-format footer around the evolved
  instruction (mirrors p8: optimize the instruction, not the injection plumbing),
  so a reflected instruction that drops a `{problem}` placeholder still solves.

No-spend proof of the same code path: `tests/examples/test_live_optimize_aime.py`
(`test_example_14_optimization_mechanics_improve_with_mock_lm`) drives the same
runner/rubric/`build_optimization` with the mock LM over AIME-shaped fixtures
(empty seed -> 0; reflected instruction -> admitted child beats seed). NOTE: the
host rebuilds the runtime LM per `lm.complete`, so a mock solver replays
responses[0] every call; the mock proves loop mechanics, not prompt-sensitive
solving (that is the live example's job).

Verification run for slice 4 (Python-only, focused): sdk `just check` green; sdk
`uv run pytest` 320 passed; sdk `just examples` green (13 examples, 14 self-skips
without the gate); sdk `just compile-examples` ok. No Rust changed, so no cargo
gates needed. `just check`/`just coverage`/`release-check` (repo-root) NOT run
(out of scope; sdk gates are the owning surface).

## Slice 5 live evidence (2026-06-11) — kit cutoff NOT met live; BLOCKER (headroom)

The served kit-optimization path is fully built and VERIFIED FUNCTIONAL end to
end, and the cutoff mechanic is PROVEN deterministically (no-spend). The LIVE
cutoff (a kit child that strictly beats the seed on a real TB2 task) was NOT met,
because gpt-5.4-mini solves the chosen tasks regardless of the AGENTS.md kit. This
is a truthful negative recorded per the headroom ladder's final rung ("never lower
the bar"), not an implementation gap.

What is proven (load-bearing): the deterministic no-spend cutoff
`examples/codex_terminal_bench/tests/test_kit_optimization_mechanics.py` drives the
REAL served `leaven/optimize.run` path — real GEPA loop, real Git-backed kit
materialization/readback, real agentic Git-program reflection, real worker
runner/scorer dispatch, real frontier admission — with two no-spend substitutions
that are the deepest cut reachable through the served CLI: a scripted fake-codex
binary (the Rust `FakeAgentRuntime` is `#[cfg(test)]`-only and unreachable through
`SeamServiceConfig`, so a scripted codex binary is the only deterministic agentic
option) and an explicit env-gated fake-trial seam (the rollout runs in a worker
subprocess where the live Harbor `Trial` cannot be monkeypatched). It asserts the
kit child is authored, applied through the run graph, and re-evaluated onto the
frontier beating the seed; it fails if any of that does not happen. Passes
reliably in ~31s.

What ran live (verified functional, with concrete evidence):
- The full live machinery works: `leaven seam serve --stdio` host + GEPA kit loop;
  the harbor rollout runs ONE real Harbor Trial per candidate with `@openai/codex`
  installed in-container; the `LeavenCodex` agent uploads the seed kit to
  `/app/AGENTS.md` (verified: the seed system prompt appeared in `/app/AGENTS.md`
  inside a running container); codex (gpt-5.4-mini) solves the task; the verifier
  writes `reward.txt` and `ctrf.json`; the durable Git-backed run persists under
  `.leaven/release-runs/run_codex_terminal_bench_*` (with `kit-stores/agent_kit.git`,
  checkpoints, blobs). An oracle smoke (no codex spend) first confirmed the
  git-pinned task downloads/builds/scores.
- Pinned task: `terminal-bench-2` @ commit
  `2fd12b88aafdd04a52c298e3940bcb189f9766d6`, image `alexgshaw/regex-log:20251031`
  (and `alexgshaw/password-recovery:20251031` for the second attempt). NOTE: the
  TB2 images are x86_64; on this ARM Mac they run under qemu emulation, so each
  in-container codex trial is slow (~15-20 min).

Live cutoff attempts (sanctioned spend, recorded honestly):
- Command (from `sdk/python`): `set -a; source ../../.env; set +a;
  export LEAVEN_CODEX_BIN=/Users/darin/.codex/packages/standalone/current/codex;
  LEAVEN_CODEX_LIVE=1 LEAVEN_CODEX_BIN=$LEAVEN_CODEX_BIN [LEAVEN_CODEX_TB_TASK=...]
  LEAVEN_OPTIMIZE_TIMEOUT_S=7200 LEAVEN_RUNS_ROOT=.leaven/release-runs
  uv run python examples/15_live_optimize_codex_terminal_bench.py`.
  (`LEAVEN_CODEX_BIN` is required: the scenario configures
  `gepa(reflection_agent=lv.agent.codex(transport='cli', bin_path_env='LEAVEN_CODEX_BIN'))`,
  and that env points the host's agentic reflector at the local codex binary.)
- Run dirs: `.leaven/release-runs/run_codex_terminal_bench_18b80839d2635738_57eab38e`
  (regex-log, original weak seed), `..._18b808a718ae7430_5b6e2507` (regex-log,
  weakened seed), `..._18b808c7750b9698_fcc810f5` (password-recovery, weakened seed).
- Attempt 1 (regex-log, original weak seed "...Work quickly."): the SEED scored
  validation reward=1 (verifier `reward.txt`=1). Weighted-mean caps at 1.0 (verifier
  w=1 + ctrf w=0.25 both at 1.0), so no child can strictly beat it. Headroom ladder
  rung 1: weaken the seed.
- Attempt 2 (regex-log, weakened seed "Move fast: write the first solution that
  looks plausible and finish immediately; do not explore inputs or test"): the SEED
  again scored validation reward=1. The kit did not change the outcome.
- Attempt 3 (password-recovery, the same weakened seed, a HARD method-dependent
  forensic task whose method is NOT in the instruction): the SEED agent, despite the
  "rush" kit, did methodical forensics (carved nested disk images) and RECOVERED the
  password `8XDP5Q2RT9ZK7VB3BV4WW54` (23 chars, `8XD...W54`, valid) → reward=1. Run
  stopped once the seed solution was written (no headroom).

Root cause (the truthful finding): codex reads BOTH the kit's `AGENTS.md` AND the
task instruction, and TB2 task instructions are self-contained. For a strong model
(gpt-5.4-mini), the AGENTS.md kit is largely redundant — it can shape style but does
not gate the OUTCOME on a self-contained task the model can solve from the
instruction alone. So a "weak-but-honest" seed kit does not make the agent fail, and
there is no headroom for a reflected kit to win. This held on both an easy (regex-log)
and a hard (password-recovery) task. Demonstrating a live kit-gated 0->1 cutoff would
require a task whose required method/knowledge is NOT in the instruction AND that the
base model cannot do zero-shot but can with the right kit — a narrow "Goldilocks"
regime that standard TB2 tasks plus this model did not provide within the attempts
made. I did NOT lower the bar (no adversarial/sabotaging seed, no tie accepted, no
faked result).

Spend: three partial live runs, each one in-container codex trial (gpt-5.4-mini)
before stopping; token usage was on the order of the per-trial codex transcripts
(e.g. the password-recovery seed turn reported ~416k input / ~11k output tokens).
USD is not metered (Leaven meters tokens; `total_cost_usd` reports 0.0).

Resume options for a future live cutoff (do NOT lower the bar):
- Pick a task whose success genuinely depends on persistent agent guidance not in
  the instruction (a private convention, a brittle multi-step workflow, an
  environment quirk) so the seed reliably fails and a reflected kit reliably helps;
  or use a weaker in-container model so the kit's method gates the outcome.
- Make the rubric reward partial progress the kit can move (CTRF fraction is wired
  at w=0.25) on a task where the seed gets partial and a kit gets full credit.
- The deterministic proof already locks the mechanic; the live gap is task/model
  selection, not the optimize path.

No-spend proof of the same served path:
`examples/codex_terminal_bench/tests/test_kit_optimization_mechanics.py` (above).
Harbor pure-logic units: `tests/test_trial.py`, `tests/test_scenario.py`.

Verification run for slice 5B (Python-only + harbor example, focused): sdk
`just check` green; sdk `uv run pytest` 333 passed; sdk `just examples` green (14
examples; example 15 self-skips without the gate + Docker); sdk
`just compile-examples` ok; harbor project `ruff`/`ty`/`pytest` (9) green; oracle
Trial smoke confirmed the git-pinned task path. No Rust changed (5A owns it). Repo
-root `just check`/`coverage`/`release-check` NOT run (out of scope; sdk gates are
the owning surface).

## Session 2026-06-11 (cont.) — reflection traces real + clean; isolation fixed

Goal of the session: prove the agentic reflection is REAL and its traces are
saved/accessible/interpretable, and that the optimization library + eval
machinery actually work end to end. It is, with two fixes landed.

### What we found (the load-bearing facts)
- ROLLOUT trajectories existed and are real (harbor `agent/trajectory.json` +
  `agent/codex.txt` + raw codex `sessions/*.jsonl`, verifier `reward.txt`/`ctrf.json`)
  under `sdk/python/.leaven/codex-tb-trials/` from the prior live attempts.
- REFLECTION trajectories did NOT exist before this session. Every prior live
  kit run's `kit-stores/agent_kit.git` held ONLY the seed commit — reflection
  never fired live (the seed maxed at reward=1, the loop ended). The only
  "reflection" ever run end to end was the deterministic test's FAKE-codex
  script. So a real LLM authoring a kit from real feedback had ZERO captured
  evidence. (This was the real gap behind "we have traces for all of it?".)
- To capture a real reflection without the slow/headroom-blocked qemu trials:
  run REAL codex reflection (gpt-5.4-mini, ChatGPT-subscription auth, NOT the API
  key) + the no-spend fake-trial seam (`LEAVEN_CODEX_TB_FAKE_TRIAL=1`). The
  seed scores 0, so GEPA reflects, real codex authors a real child kit, it is
  applied through `RunContext` + re-evaluated (rejected only because the fake
  trial rewards a magic marker — expected, the live-headroom story is unchanged).
- CONTAMINATION discovered by reading the captured trajectory: the reflection
  codex runs on the operator's machine and absorbed (1) `~/.codex/AGENTS.md`
  doctrine — a regex-log kit came back authored in "hard-cutover style ... not a
  compatibility layer ... keep scaffolding separated" (verbatim operator
  doctrine, not task signal); and (2) the operator's whole personal skill
  arsenal — 40+ skills from `~/.agents/.skill-lock.json`, `~/.codex/superpowers`,
  `~/src/personal/skills` (codex literally "used the superpowers workflow").
  KEY: the skill registry is `$HOME`-rooted, NOT `$CODEX_HOME`-rooted, so
  CODEX_HOME isolation alone does NOT sever skills; HOME isolation does.

### Fixes landed (jj commits on top of de30e431)
- `8463408a` seam-service+python-sdk: isolate reflection codex HOME/CODEX_HOME.
  SDK driver (`_seam_optimize/driver.py::_isolated_codex_home`) auto-prepares a
  run-scoped HOME with `CODEX_HOME=<home>/.codex` carrying ONLY a copied
  `auth.json` (subscription preserved), no AGENTS.md/config.toml, no `~/.agents`.
  Plumbing: codex-cli `CodexCliConfig.home_dir` -> `HOME` env (next to
  CODEX_HOME); `SeamAgentConfig::CodexCli.home_dir`; wire
  `CodexCliRuntime{Config,Document}.home_dir`; `lv.agent.codex(codex_home=...)`
  opts out. Home lives UNDER the runs root (durable) so the codex session
  trajectory (`$CODEX_HOME/sessions`) is captured, not erased — a self-deleting
  temp home was the first attempt and ate the trajectory (regression caught
  mid-build). Result (verified live): reflection now sees ONLY codex built-ins
  (imagegen/openai-docs/plugin-creator/skill-creator/skill-installer) + the
  materialized workspace (kit) skills — reproducible across machines — and
  authors a clean task-driven kit (verify-first loop; edge cases incl.
  ordering/partial-matches, which is exactly regex-log's "last date per line").
- `795679e0` leaven-run+examples/p8_aime_gepa: own
  `OptimizationStopReason::as_str()`; drop p8's hand-match. p8 stringified the
  stop reason with a local `report_stop_reason()` that enumerated every variant,
  so the pool-cap `CandidateCapReached` variant broke the bin's TEST target
  (E0004) — hidden because `just check` / `cargo build --bin leaven` do not
  compile example test targets; only `cargo check --workspace --all-targets`
  surfaces it. Rather than patch another arm onto a brittle match, the canonical
  `snake_case` string now lives on the owning type in `leaven-run`
  (`OptimizationStopReason::as_str()`, const fn, matching the
  `leaven-public-seam` `as_str` pattern, per-variant test) and p8 uses
  `result.stop.as_str()`; a future variant can no longer drift the example.

### Verification (no-spend unless noted)
SDK `uv run pytest` 335 (added 3 driver isolation law tests); `just examples` 14;
`just compile-examples`; kit mechanics test 2 (real served host, fake codex);
Rust `cargo test -p leaven-agent-codex-cli` 8 (+ HOME-emission assertion),
`-p leaven-seam-service` 50, `cargo test -p leaven --test topology_contract` 8
(no new dep edges — codex-cli<->seam-service already existed); `cargo check
--workspace --all-targets` clean after the p8 fix. LIVE: AIME example 14
(gpt-4.1-mini solver + gpt-5.4-mini reflection) on the rebuilt binary improved
seed 0.000 -> best 0.500 (3 iters) — Goal A's prompt/LM path unaffected by the
isolation change (it only touches the agentic kit path). Repo-root
`just check` was kicked off as the broad gate (log:
`/Users/darin/tmp/reflect-capture/just_check.log`).

### Repro of the fast capture (real reflection, no docker)
From `sdk/python`, with a fresh runs root:
`set -a; source ../../.env; set +a; LEAVEN_CODEX_TB_FAKE_TRIAL=1
LEAVEN_CODEX_BIN=/Users/darin/.codex/packages/standalone/current/codex
LEAVEN_BIN=<repo>/target/debug/leaven LEAVEN_RUNS_ROOT=$PWD/.leaven/<name>
uv run --project examples/codex_terminal_bench python <driver calling
build_optimization(cases=pinned_task_cases(), metric_calls=8, minibatch_size=1,
population_size=2).run()>`. Reflection trajectory lands at
`<runs_root>/codex-homes/codex-home-*/.codex/sessions/2026/.../rollout-*.jsonl`;
authored kit at `<run-dir>/kit-stores/agent_kit.git` (seed commit + child commit,
`git --git-dir=... diff <seed> <child> system_prompt.md`).

### Next (still open)
- VM-isolated reflection (the fully-clean sandbox, the operator's repeated ask):
  run the reflection codex inside a `leaven-workspace-firkin` Apple/VZ pod — no
  operator paths at all. Decided slice shape: "routing now, live Firkin next".
  Routing-now = a seam-service workspace-backend config `{ Local | Firkin }` so
  the reflection factory is no longer the hardcoded `LocalWorkspaceFactory::temp()`
  at `crates/leaven-seam-service/src/optimize_run_service/agent_kit/loop_run.rs:200`;
  prove the Firkin materialization with the crate's fake runtime (currently
  test-only in `leaven-workspace-firkin/tests/`). NEW topology edge
  seam-service -> leaven-workspace-firkin will need the contract + AGENTS update.
  Live-Firkin = a LINUX codex binary + auth (copy `auth.json`, or inject
  OPENAI_API_KEY) + network IN the booted pod; harbor's in-container
  `@openai/codex` install is the provisioning template (`firkin-apple-vz-live`
  feature, `LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE`, signed Apple/VZ). This is the
  honest answer to the host-path skill leak (HOME isolation is the host-path
  patch; the VM is the real boundary).
  DE-RISKED: the kit-in-pod half is ALREADY proven live. The reflector uses
  `GitProgramMaterializer`/`GitProgramReadback` over a `WorkspaceFactory`, and
  `leaven-workspace-firkin/tests/firkin_contract/firkin_live_git_e2e.rs`
  (`live_apple_vz_product_pod_materializes_and_reads_back_git_workspaces`,
  signed Apple/VZ, alpine/git image) already materializes + reads back Git
  workspaces in a booted pod with those SAME primitives. So routing-now is just
  swapping `LocalWorkspaceFactory` for `FirkinWorkspaceFactory` in
  `build_reflector` via config; the ONLY genuinely new work is codex EXECUTION
  in the pod (image carries node+codex or installs it at setup, auth +
  network) — a provisioning/image/spend decision that is darin's to make.
  Governing design: `docs/specs/agentic_trace_reflection_product_backend.md`
  (+ `firkin_git_workspace_backend.md`, `firkin_git_workspace_api_shape.md`);
  boundary law there: `leaven-workspace-firkin` stays workspace substrate (must
  not know GEPA/artifact/Firkin-kit layout), the GEPA Git-program bridge stays
  in `leaven-gepa-agentic-git`.
- The LIVE benchmark cutoff (a kit child strictly BEATING the seed on a real TB2
  task) is STILL headroom-blocked: gpt-5.4-mini solves regex-log AND
  password-recovery regardless of kit. Unchanged. Pursue a headroom-bearing
  task/model (reasoning_effort=low solver, or a method-dependent task) only on an
  explicit live request.
