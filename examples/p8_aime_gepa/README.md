# AIME GEPA Public API Example

This example exercises the high-level Leaven API:

```bash
cargo run -p p8_aime_gepa
```

The default path is deterministic and uses a scripted solver plus a local
provider-neutral `Lm` fixture for reflection, so CI can prove the LM-backed GEPA
builder path without live model spend. The reflector uses Leaven GEPA's default
upstream-style reflection prompt and fenced replacement parser. It reports
baseline train score, optimized train score, validation score, held-out test
score, split reports, budget usage, emitted events, and the selected prompt.

To materialize the upstream AIME HuggingFace cache:

```bash
uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py \
  --out target/leaven-aime-cache/aime.json

LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json \
  cargo run -p p8_aime_gepa
```

The cache uses `AI-MO/aimo-validation-aime` for train/validation and `MathArena/aime_2025` for final held-out test, matching the upstream GEPA example.

To run the same harness with OpenAI as the solver, while keeping reflection on
the deterministic fixture:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_OPENAI_MODEL=gpt-4.1-mini
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

To run OpenAI for reflection too:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI_REFLECTION=1
export LEAVEN_AIME_REFLECTION_MODEL=gpt-5.4-mini
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

The OpenAI path is an opt-in native async solver swap over the same
`leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()`
surface. Both live solver and live reflection use `leaven-lm-openai`; solver
LM spend is attached to `RunOutput` and charged through evaluation accounting.
The score function is the normal async/fallible Leaven scorer surface; this
example uses a fixed-reference checker, while model judges can return
`ScoreError`, scorer cost, and feedback attachments through the same path.
The live reflection path uses `LmBackedReflector`, with `gpt-5.4-mini` and
medium reasoning as the default reflection model controls. The deterministic
path proves public API mechanics and LM-backed reflection invariants; it is not
evidence of live AIME improvement.
