# AIME GEPA Public API Example

This example exercises the high-level Leaven API:

```bash
cargo run -p p8_aime_gepa
```

The default path is deterministic and uses a scripted solver, so CI can prove GEPA mechanics without live model spend. It reports baseline train score, optimized train score, validation score, held-out test score, split reports, budget usage, emitted events, and the selected prompt.

To materialize the upstream AIME HuggingFace cache:

```bash
uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py \
  --out target/leaven-aime-cache/aime.json

LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json \
  cargo run -p p8_aime_gepa
```

The cache uses `AI-MO/aimo-validation-aime` for train/validation and `MathArena/aime_2025` for final held-out test, matching the upstream GEPA example.

To run the same harness with OpenAI as the solver:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_OPENAI_MODEL=gpt-4.1-mini
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

The OpenAI path is an opt-in runner/provider swap over the same `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()` surface. The deterministic path proves public API mechanics and invariants; it is not evidence of live AIME improvement.
