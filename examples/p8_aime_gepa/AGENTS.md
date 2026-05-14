## Boundary
This package is the high-level public API GEPA example. It has a deterministic default path and opt-in data/provider paths that must remain visibly separate.

P8 owns the example artifact, edit surface, deterministic AIME-shaped cases, cache loader, OpenAI solver script, and public `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()` demonstration. Reusable GEPA, surface, runner, scoring, or provider abstractions belong in the owning crates.

## Code Landmarks
- `src/main.rs::run_aime` is the public builder shell to preserve when proving
  ordinary-user ergonomics. Its use of `Gepa::builder().surface(...).population(...).reflector(...).max_iterations(1)` is a GEPA wiring demo, not a final API map.
- `AimePromptSurface` is the local artifact/surface handhold: one `"system"`
  part and artifact-native `AimePromptChange`.
  Reusable surface rules belong in `leaven-surface` or GEPA docs, not here.
- `aime_lm_reflector` is the deterministic LM-backed reflector. It injects a
  local `DeterministicReflectionLm` into the production `LmBackedReflector` /
  `ReflectRequest` / `ReflectionOutputParser` path and applies the resulting
  proposal through GEPA and `RunContext`.
- `score_answer` proves current score/report plumbing by producing scalar
  scores, feedback text, and trace lines. It does not prove rich scoring,
  evaluator errors, attachments, or evidence refs.
- `run_openai_solver` is an external process runner around
  `scripts/openai_solver.py`. It is intentionally a live solver smoke path
  outside Leaven solver/runtime/cache roles.

## Proof Paths
- Clean deterministic proof: `just milestone-p8` with `LEAVEN_AIME_CACHE` and `LEAVEN_AIME_LIVE_OPENAI` unset proves public builder mechanics, split reporting, and the production LM-backed GEPA reflection route through provider-neutral `leaven-lm`. It does not prove live provider quality.
- Local cached-data proof: materialize `target/leaven-aime-cache/aime.json` with `uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py --out target/leaven-aime-cache/aime.json`, then run `LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa`; this proves the same harness can consume the upstream-shaped AIME cache, not that the default deterministic fixture changed.
- Live solver proof: `OPENAI_API_KEY=... LEAVEN_AIME_LIVE_OPENAI=1 LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json cargo run -p p8_aime_gepa` swaps only the runner to the OpenAI Responses script. It spends provider resources, still uses deterministic reflection, and is not part of the cheap milestone lane.
- Unit tests in `src/main.rs` prove deterministic improvement, train-only
  absent validation/test scores, missing score refusal, and cache role
  preservation. The test named
  `deterministic_aime_acceptance_shows_public_gepa_improvement` proves the
  public builder path now uses the LM-backed reflection route.

## Local Rules
- If your shell may already export live/cache variables, unset them before claiming deterministic p8 behavior: `env -u LEAVEN_AIME_CACHE -u LEAVEN_AIME_LIVE_OPENAI just milestone-p8`.
- Preserve the train/validation/test roles in both deterministic cases and cache JSON. The example is specifically proving the public API facade reports split scores and held-out test scores.
- Keep the OpenAI integration in `scripts/openai_solver.py` as an opt-in example runner. Do not move OpenAI Responses payload details into the public GEPA example surface or provider-neutral crates.
- Live-provider product proof still requires a concrete provider `Lm`. The
  deterministic path is production Leaven plumbing with fake model output, not
  evidence of model quality.
- Generated HuggingFace cache files belong under `target/leaven-aime-cache/`; do not commit materialized upstream data.
- If you change the deterministic cases, preserve why baseline fails and the
  deterministic reflection improves it. Otherwise the example stops proving builder/report
  mechanics and becomes a noisy fixture tweak.
- If you change the live provider path, the honest target is to remove the
  Python process boundary in favor of Leaven LM/runtime role construction. Do
  not add more provider-specific behavior to `run_solver`.

## Bait
- A passing deterministic p8 run proves public API mechanics, invariant
  reporting, and the LM-backed GEPA reflection route; it is not evidence of
  live AIME benchmark improvement.
- A cached or live p8 run proves operator wiring over a particular local dataset/provider environment; it is not a replacement for the deterministic default acceptance path.
- `DeterministicReflectionLm` is still a deterministic LM fixture. Do not cite
  P8 as proof of provider-native transport, cache behavior, or live model
  reflection quality.
- `leaven::prelude::*` makes this example compact, but it also imports advanced
  engine/GEPA/cache names today. Do not use P8 as evidence that the ordinary
  prelude is clean; that belongs in `crates/leaven`.
