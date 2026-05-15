## Boundary
This package is the high-level public API GEPA example. It has a deterministic default path and opt-in data/provider paths that must remain visibly separate.

P8 owns the example artifact, edit surface, deterministic AIME-shaped cases, cache loader, upstream/source IDs carried in `AimeCase`, native OpenAI solver wiring through `leaven-lm-openai`, and public `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()` demonstration. Reusable GEPA, surface, runner, scoring, report-schema, or provider abstractions belong in the owning crates.

## Code Landmarks
- `src/main.rs::run_aime` is the public builder shell to preserve when proving
  ordinary-user ergonomics. The deterministic smoke profile deliberately caps
  GEPA at one iteration; the live GEPA AIME profile raises that Leaven-local
  ceiling because the reference is controlled by `max_metric_calls`, not
  `max_iterations`.
- `AimePromptSurface` is the local artifact/surface handhold: one `"system"`
  part and artifact-native `AimePromptChange`.
  Reusable surface rules belong in `leaven-surface` or GEPA docs, not here.
- `aime_lm_reflector` is the LM-backed reflector. By default it injects a local
  `DeterministicReflectionLm`; with `LEAVEN_AIME_LIVE_OPENAI_REFLECTION=1` it
  injects the P8-local OpenAI reflection role with `gpt-5.4-mini` and medium
  reasoning unless `LEAVEN_AIME_REFLECTION_MODEL` overrides the model. Both
  routes use `DefaultReflectionRenderer` / `PlainTextEditParser` and apply the
  resulting proposal through GEPA and `RunContext`.
- `run_solver` returns only the generated answer as `RunOutput` plus metered
  cost. `AimeCase.source_id`, problem text, and prompt text stay in the case or
  artifact instead of being smuggled through trace/report strings.
- `score_answer` proves the async/fallible score surface on a fixed-reference
  local checker by producing scalar scores and feedback text. It
  does not prove live model-judge quality or domain-specific score semantics.
- `run_openai_solver` is a native async runner over the P8-local OpenAI solver
  role. It returns `RunOutput` with the LM cost attached so solver spend is
  charged through normal evaluation accounting. Live solver and reflection cache
  policies are independent env knobs: `LEAVEN_AIME_SOLVER_CACHE_POLICY` and
  `LEAVEN_AIME_REFLECTION_CACHE_POLICY`, accepting `never`, `read-write`,
  `read-only`, or `refresh`; deterministic smoke keeps both at `never`.
  `LEAVEN_AIME_LM_CACHE_BACKEND` currently accepts only `in-memory`, and P8
  reports `lm_cache_durable=false` so the opt-in response cache is not confused
  with durable run/resume storage. `LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS`
  configures the OpenAI provider semaphore for both live roles.

## Proof Paths
- Clean deterministic proof: `just milestone-p8` with `LEAVEN_AIME_CACHE` and `LEAVEN_AIME_LIVE_OPENAI` unset proves public builder mechanics, split reporting, and the production LM-backed GEPA reflection route through provider-neutral `leaven-lm`. It does not prove live provider quality.
- Local cached-data proof: materialize `target/leaven-aime-cache/aime.json` with `uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py --out target/leaven-aime-cache/aime.json`, then run `LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa`; this proves the same harness can consume the upstream-shaped AIME cache under the deterministic provider fixture, not live GEPA AIME quality.
- Live solver proof: `OPENAI_API_KEY=... LEAVEN_AIME_LIVE_OPENAI=1 LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa` swaps the runner to native `leaven-lm-openai` and uses the GEPA AIME profile knobs available in Leaven. It spends provider resources, records solver LM cost in evaluation budget, still uses deterministic reflection, and is not part of the cheap milestone lane.
- Live reflection proof: `OPENAI_API_KEY=... LEAVEN_AIME_LIVE_OPENAI_REFLECTION=1 LEAVEN_AIME_REFLECTION_MODEL=gpt-5.4-mini LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa` swaps reflection to `leaven-lm-openai` through the same `LmBackedReflector` and default GEPA prompt renderer. It spends provider resources and is not part of the cheap milestone lane.
- Unit tests in `src/main.rs` prove deterministic improvement, train-only
  absent validation/test scores, missing score refusal, cache role
  preservation, live LM cache/runtime report truth, and configured OpenAI
  concurrency parsing. The test named
  `deterministic_aime_acceptance_shows_public_gepa_improvement` proves the
  public builder path now uses the LM-backed reflection route.

## Local Rules
- If your shell may already export live/cache variables, unset them before claiming deterministic p8 behavior: `env -u LEAVEN_AIME_CACHE -u LEAVEN_AIME_LIVE_OPENAI -u LEAVEN_AIME_LIVE_OPENAI_REFLECTION just milestone-p8`.
- Preserve the train/validation/test roles in both deterministic cases and cache JSON. Cache JSON must also preserve a stable `source_id` for every case using `dataset:config:split:row` for HuggingFace materialization. The example is specifically proving the public API facade reports split scores and held-out test scores.
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
  construction, metered `RunOutput` cost, and source-id trace propagation. The
  solver prompt is intentionally Rust-native: system prompt plus answer-only
  user turn through `leaven-lm`, not DSPy `ChainOfThought` prompt lowering or
  rationale-field extraction. Do not add provider-specific behavior beyond the
  local example adapter.

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
  It no longer uses `leaven::prelude::*`. Keep imports on those explicit routes;
  the route classification itself is proven in `crates/leaven`.
