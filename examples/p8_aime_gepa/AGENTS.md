## Boundary
This package is the high-level public API GEPA example. It has a deterministic default path and opt-in data/provider paths that must remain visibly separate.

P8 owns the example artifact, edit surface, deterministic AIME-shaped import records, cache loader, AIME-local lowering into `Case<AimeInput, AimeTarget>`, source/report metadata sidecar, native OpenAI solver wiring through `leaven-lm-openai`, and public `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()` demonstration. Reusable GEPA, surface, runner, scoring, report-schema, or provider abstractions belong in the owning crates.

## Code Landmarks
- `src/main.rs::run_aime` is the public builder shell to preserve when proving
  ordinary-user ergonomics. The deterministic smoke profile deliberately caps
  GEPA at one iteration; the live GEPA AIME profile raises that Leaven-local
  ceiling because the reference is controlled by `max_metric_calls`, not
  `max_iterations`.
- `AimePromptSurface` is the local artifact/surface handhold: one `"system"`
  part and artifact-native `AimePromptChange`.
  Reusable surface rules belong in `leaven-surface` or GEPA docs, not here.
- `aime_lm_reflector` is the LM-backed reflector. The deterministic smoke path
  injects a local `DeterministicReflectionLm`. The live GEPA AIME profile
  injects the P8-local OpenAI reflection role by default with `gpt-5.4-mini` and
  medium reasoning unless `LEAVEN_AIME_REFLECTION_MODEL` overrides the model.
  `LEAVEN_AIME_DETERMINISTIC_REFLECTION=1` is the explicit debug/scaffold path
  for live solver plus deterministic reflection. Both routes use
  `DefaultReflectionRenderer` / `PlainTextEditParser` and apply the resulting
  proposal through GEPA and `RunContext`.
- `run_solver` receives `RunCase<AimeInput>` and returns only the generated
  answer as `RunOutput` plus metered cost. The ordinary runner path sees the
  problem input and case id, not AIME targets, source metadata, split role, or
  report tags.
- `score_answer` receives `ScoreContext<AimePrompt, AimeInput, AimeTarget>` and
  reads the hidden answer/reference solution through scorer context. It proves
  the async/fallible score surface on a fixed-reference local checker by
  producing scalar scores and feedback text. It does not prove live model-judge
  quality or domain-specific score semantics.
- `run_openai_solver` is a native async runner over the P8-local OpenAI solver
  role. It returns `RunOutput` with the LM cost attached so solver spend is
  charged through normal evaluation accounting. The live solver/reflection
  cache policy env knobs (`LEAVEN_AIME_SOLVER_CACHE_POLICY` and
  `LEAVEN_AIME_REFLECTION_CACHE_POLICY`) are advanced P8 scaffolding for role
  experiments, not required product setup. Omitted live-role policy means
  read/write response-cache use; deterministic smoke remains explicitly
  no-cache with `never`.
  `LEAVEN_AIME_LM_CACHE_BACKEND` defaults to `sqlite` for live OpenAI roles and
  stores the reusable `leaven-lm-cache` database at `<run-dir>/lm-cache.sqlite`.
  Explicit `in-memory` is the throwaway/debug backend and reports
  `lm_cache_durable=false`. `LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS` configures
  the OpenAI provider semaphore for both live roles.
- P8 report lines project `source_id` through a local sidecar keyed by stable
  AIME case id because the generic `leaven-run` report facade does not yet carry
  a `CaseSourceRef`. Default lines include output/feedback lengths only and
  must not disclose target answers or reference solution text.
- P8 role report lines are local operator projection until generic run reports
  own LM role summaries. They print proof classification, search
  metric-call cap/spent, final-report metric calls, evaluation-cache counts,
  solver/reflection runtime fingerprints, LM calls/tokens/cost, cache
  hit/miss/bypass counts, and typed provider-failure counters. Fingerprints
  are short human summaries and must exclude API keys, bearer tokens, and local
  cache paths.
- `AimeReflectiveDataset` is a local bridge over the current GEPA
  reflective-dataset seam. It projects problem input from P8's target-free case
  sidecar and score/output/feedback from evidence; it must not read raw
  `AimeTarget`. Remove the bridge when the shared GEPA/report projection can
  recover target-safe inputs and source refs without P8-local `RunContext` glue.

## Proof Paths
- Clean deterministic proof: `just milestone-p8` with `LEAVEN_AIME_CACHE` and `LEAVEN_AIME_LIVE_OPENAI` unset proves public builder mechanics, split reporting, and the production LM-backed GEPA reflection route through provider-neutral `leaven-lm`. It does not prove live provider quality.
- Local cached-data proof: materialize `target/leaven-aime-cache/aime.json` with `uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py --out target/leaven-aime-cache/aime.json`, then run `LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa`; this proves the same harness can consume the upstream-shaped AIME cache under the deterministic provider fixture, not live GEPA AIME quality.
- Full live GEPA AIME proof: `OPENAI_API_KEY=... LEAVEN_AIME_LIVE_OPENAI=1 LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa` swaps both solver and reflection to native `leaven-lm-openai`, uses durable SQLite LM response caching, enables deterministic evaluator caching on the durable run store, and uses `gpt-5.4-mini` medium-reasoning reflection by default. It spends provider resources, records solver LM cost in evaluation budget, and is not part of the cheap milestone lane.
- Unit tests in `src/main.rs` prove deterministic improvement, target-safe AIME
  lowering, runner target-invisibility by type route, scorer target visibility,
  source-id report projection without target disclosure, duplicate source-id
  refusal, train-only absent validation/test scores, missing score refusal,
  cache role preservation, cache-hit zero-new-cost behavior for deterministic
  LM fixtures, live LM cache/runtime report truth, typed missing-credential
  failure redaction, and configured OpenAI concurrency parsing. The test named
  `deterministic_aime_acceptance_shows_public_gepa_improvement` proves the
  public builder path now uses the LM-backed reflection route.

## Local Rules
- If your shell may already export live/cache variables, unset them before claiming deterministic p8 behavior: `env -u LEAVEN_AIME_CACHE -u LEAVEN_AIME_LIVE_OPENAI -u LEAVEN_AIME_DETERMINISTIC_REFLECTION just milestone-p8`.
- Preserve the train/validation/test roles in both deterministic cases and cache JSON. Cache JSON must also preserve a stable `source_id` for every import record using `dataset:config:split:row` for HuggingFace materialization. P8 lowers those records into `Case<AimeInput, AimeTarget>` before they reach the runner. The example is specifically proving the public API facade reports split scores, held-out test scores, and report-visible source IDs without making targets runner-visible.
- Keep the OpenAI solver integration on the same async `.runner(...)` surface
  and concrete `leaven-lm-openai` provider wrapped by `leaven-lm-cache` as
  reflection. Do not reintroduce a Python process boundary or move OpenAI
  Responses payload details into the public GEPA example surface or
  provider-neutral crates.
- The deterministic path is production Leaven plumbing with fake model output,
  not evidence of model quality.
- Generated HuggingFace cache files belong under `target/leaven-aime-cache/`; do not commit materialized upstream data.
- If you change the deterministic cases, preserve why baseline fails and the
  deterministic reflection improves it. Otherwise the example stops proving builder/report
  mechanics and becomes a noisy fixture tweak.
- If you change the live solver path, preserve native Leaven LM/runtime role
  construction, metered `RunOutput` cost, and source-id report projection
  outside the runner. The solver prompt is intentionally Rust-native: system
  prompt plus answer-only user turn through `leaven-lm`, not DSPy
  `ChainOfThought` prompt lowering or rationale-field extraction. Do not add
  provider-specific behavior beyond the local example adapter.

## Bait
- A passing deterministic p8 run proves public API mechanics, invariant
  reporting, and the LM-backed GEPA reflection route; it is not evidence of
  live AIME benchmark improvement.
- A cached or live p8 run proves operator wiring over a particular local dataset/provider environment; it is not a replacement for the deterministic default acceptance path.
- `DeterministicReflectionLm` is still a deterministic LM fixture. Do not cite
  the default P8 path as proof of provider-native transport, cache behavior, or
  live model reflection quality. It should mimic upstream GEPA reflection output
  by returning fenced replacement instruction text, not a local JSON schema.
- P8 imports through the audience-routed umbrella surface: ordinary types from
  `leaven::prelude`, component-author types from `leaven::extend`, identity
  internals from `leaven::plumbing`, and `Gepa` from the `leaven::gepa` alias.
  The current `AimeReflectiveDataset` bridge is the explicit exception: it uses
  `leaven::engine::RunContext` and `leaven::run::RunProblem` as local scaffold,
  not as public-user ergonomics. It no longer uses `leaven::prelude::*`. Keep
  imports on explicit routes; the route classification itself is proven in
  `crates/leaven`.
