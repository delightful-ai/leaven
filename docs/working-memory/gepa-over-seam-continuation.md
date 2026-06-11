# GEPA over the seam — progress + continuation (2026-06-10)

Status: slices 1–3 DONE and verified. Slice 4 (AIME live) next, slice 5
(codex-kit on terminal-bench) after. Design doc (amended, authoritative):
`docs/plans/2026-06-10-gepa-over-seam-design.md`.

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
