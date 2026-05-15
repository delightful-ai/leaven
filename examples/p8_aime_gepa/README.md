# AIME GEPA Public API Example

This example exercises the high-level Leaven API:

```bash
cargo run -p p8_aime_gepa
```

With no cache or live provider environment, this runs a deterministic smoke
fixture so CI can prove the LM-backed GEPA builder path without live model
spend. That smoke fixture is not benchmark evidence.

When a live OpenAI solver is configured, the example switches to the GEPA AIME
profile:

- seed prompt: `Solve the math problem carefully. Break down the steps and provide the final answer as a single number.`
- solver: `gpt-4.1-mini`
- solver sampling: `temperature=1.0`, `max_output_tokens=32000`
- metric-call budget: `500`
- evaluation parallelism: `32`
- reflection: Leaven GEPA default upstream-style reflection prompt,
  fenced replacement parser, and `gpt-5.4-mini` with medium reasoning by default

The GEPA AIME reference does not set a max-iterations hyperparameter. This
example currently has to set a Leaven-local internal iteration ceiling because
`leaven-gepa` still exposes an iteration cap where the reference is controlled
by `max_metric_calls`.

It reports baseline train score, optimized train score, validation score,
held-out test score, split reports, budget usage, emitted events, and the
selected prompt.

To materialize the upstream AIME HuggingFace cache:

```bash
uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py \
  --out target/leaven-aime-cache/aime.json

LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json \
  cargo run -p p8_aime_gepa
```

The cache uses `AI-MO/aimo-validation-aime` for train/validation and `MathArena/aime_2025` for final held-out test, matching the upstream GEPA example.

To run the GEPA AIME profile with OpenAI as the solver, while keeping reflection
on the deterministic fixture:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

To run OpenAI for reflection too, still using `gpt-5.4-mini` instead of the
paper artifact's `gpt-5.1` reflector:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_LIVE_OPENAI_REFLECTION=1
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

The GEPA CAIS artifact is the concrete paper-reproduction target: it reports
AIME Math test accuracy improving from 46.67% to 60.00%, with validation
reaching 57.78%. The DSPy tutorial reports an
`auto="light"` run from 46.6% to 56.6% and repeats AIME 2025 five times for a
more stable test estimate. Leaven's current P8 profile matches the public
dataset roles and the available optimizer/provider knobs, with one intentional
reflection-model difference (`gpt-5.4-mini` instead of `gpt-5.1`). The main
remaining exact-parity gap is that this Rust example calls the solver as a
direct answer-field request rather than reproducing DSPy's full
`ChainOfThought` prompt lowering. A second library-level gap is that Leaven
currently has one run budget across optimization and final validation/test
evaluation, while the GEPA reference's `max_metric_calls=500` is an optimization
engine setting.
