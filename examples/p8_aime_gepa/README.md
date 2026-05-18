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

It optimizes exactly the `AimePrompt.system` field through the local
`AimePromptSurface`; problems, answers, solutions, and case source IDs are task
inputs, not optimized artifact fields. It reports baseline train score,
optimized train score, validation score, held-out test score, the held-out test
score use (`final_report_only`), proof classification, split reports, search
metric-call cap/spent, final-report metric-call cost, total budget usage,
evaluation-cache counts, solver/reflection runtime fingerprints, LM
calls/tokens/cost by role, LM cache hit/miss/bypass counts by role, typed
provider-failure counters, emitted events, case IDs/source IDs for reported
cases, selected prompt, and AIME dataset proof fields: train/validation/test
counts, source dataset/config/split counts, split seed, test-repeat policy, and
the materialized cache SHA-256 when `LEAVEN_AIME_CACHE` is used.
GEPA search minibatches remain optimization evidence only; the public
baseline/optimized train scores come from explicit full-train final report
evaluations so report aggregates are not confused with sampled feedback.

To materialize the upstream AIME HuggingFace cache:

```bash
uv run --with datasets python examples/p8_aime_gepa/scripts/materialize_hf_aime.py \
  --out target/leaven-aime-cache/aime.json

LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json \
  cargo run -p p8_aime_gepa
```

The cache uses `AI-MO/aimo-validation-aime` for train/validation and `MathArena/aime_2025` for final held-out test, matching the upstream GEPA example. Each cached case carries a stable `source_id` in the form `dataset:config:split:row`, and the top-level `train`/`validation`/`test` keys remain the split-role boundary. P8 consumes structured Leaven `CaseId` report rows and carries each case's upstream `source_id` into the public evaluation report trace.

To run the GEPA AIME profile with OpenAI as the solver and reflector:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

For a strict upstream-reflector comparison run, keep the same profile and set
the reflection model to the upstream AIME example's model:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
export LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1
cargo run -p p8_aime_gepa
```

The P8 report prints `comparison_reflection_model_alignment=upstream-matched`
only when the effective reflection model matches the upstream AIME profile. The
default `gpt-5.4-mini` reflector remains a deliberate Leaven model delta, not a
byte-for-byte upstream runtime claim.

To compare Leaven against the published DSPy/GEPA AIME quickstart denominator
without running DSPy, use the Leaven DSPy-comparison profile:

```bash
export OPENAI_API_KEY=...
export LEAVEN_AIME_PROFILE=dspy-quickstart
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
cargo run -p p8_aime_gepa
```

That profile uses the same real AIME cache and the same upstream GEPA
optimize-anything reflection prompt template plus DSPy `ChainOfThought`
ChatAdapter solver message format, but caps Leaven search at the DSPy
quickstart's `150` metric calls and reports
`comparison_target=dspy_gepa_quickstart_aime_2025` with the published
`56.6%` held-out test target. It remains a Leaven-native run: no DSPy runtime
is linked into the Rust example.

The OpenAI path is an opt-in native async solver swap over the same
`leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()`
surface. `LEAVEN_AIME_LIVE_OPENAI=1` enables both live solver and live
reflection by default, matching the GEPA AIME reproduction path. Both live
solver and live reflection use `leaven-lm-openai` wrapped by
the P8-local OpenAI LM role, with response caching configured internally;
solver LM spend is attached to `RunOutput` and charged through evaluation
accounting. Live roles use the run's LM response cache by default, and the
live GEPA profile declares deterministic evaluator caching over the durable
run store to match GEPA's `cache_evaluation=True` behavior. The
role-specific cache env vars are advanced P8 scaffold for experiments, not
required product setup:
`LEAVEN_AIME_SOLVER_CACHE_POLICY` and `LEAVEN_AIME_REFLECTION_CACHE_POLICY`
accept `auto`, `never`, `read-write`, `read-only`, `cache-only`, or `refresh`,
and omitted values default to read/write cache use. `cache-only` is the
fail-closed replay mode for no-spend release proof: it reads compatible cache
entries and errors on any miss instead of calling the provider.
`LEAVEN_AIME_LM_CACHE_BACKEND` defaults to `sqlite`, placing the reusable
`leaven-lm-cache` store at `<run-dir>/lm-cache.sqlite`. `eager-sqlite`
uses a shared workspace cache at `.leaven/lm-cache.sqlite` so fresh release
runs can reuse compatible solver/reflection responses from earlier runs
without reusing the whole run directory. Explicit `in-memory` is the
throwaway/debug backend and prints `lm_cache_durable=false`.
`LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS` bounds in-process OpenAI request
concurrency for both live roles and defaults to `32`. The P8 binary runs on a
32-worker Tokio runtime so cache-missing case batches can make real progress
up to that provider throttle instead of serializing on the executable runtime.
`LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS` controls the per-request OpenAI
transport timeout and defaults to `120`; long release runs that encounter slow
Responses API calls should set it explicitly, for example `600`, and the P8
report prints the effective timeout for both live roles.
`LEAVEN_AIME_RUN_DIR` is the explicit resume handle and uses the same durable
run-directory layout as `.run_dir(path)`. Omit it to get a managed
`.leaven/runs/<run-id>/` directory. `LEAVEN_AIME_DETERMINISTIC_REFLECTION=1`
is the debug/scaffold path for live solver with deterministic local reflection.
The score function is the normal async/fallible Leaven scorer surface. This
example uses a fixed-reference checker that mirrors the GEPA/DSPy AIME feedback
shape: exact-match scalar score, correctness text, full reference solution, and
takeaway prompt for reflection. Model judges can return `ScoreError`, scorer
cost, and feedback attachments through the same path.
The live reflection path uses `LmBackedReflector`, text output, and the
plain-text fenced parser, with `gpt-5.4-mini` and medium reasoning as the default
reflection model controls. `LEAVEN_OPENAI_MODEL` and
`LEAVEN_AIME_REFLECTION_MODEL` keep solver and reflection models independently
swappable. The deterministic path proves public API mechanics and LM-backed
reflection invariants; it is not evidence of live AIME improvement.

The GEPA CAIS artifact is the concrete paper-reproduction target: it reports
AIME Math test accuracy improving from 46.67% to 60.00%, with validation
reaching 57.78%. The DSPy tutorial reports an
`auto="light"` run from 46.6% to 56.6% and repeats AIME 2025 five times for a
more stable test estimate. Leaven's current P8 profile matches the public
dataset roles and the available optimizer/provider knobs, except that Leaven
deliberately defaults reflection to `gpt-5.4-mini` with medium reasoning. The main
remaining exact-parity gap is implementation provenance rather than model
experience: this Rust example locally renders DSPy's `ChainOfThought`
ChatAdapter message shape and parses the `answer` field, but it does not link
or execute the DSPy Python runtime. Leaven's public report separates
optimization cost from final validation/test report cost; the remaining local
control difference is the Leaven-local iteration ceiling described above.
