# Examples

Thirteen example scripts show the current Leaven Python surface, but they do
not all prove the same maturity level. The table below is the contract: cite an
example only for the proof class it names.

Most examples are shape scaffolds that import, typecheck, and demonstrate the
authoring API while stopping at expected `NotImplementedError` boundaries.
Example 03 is the no-spend wired prompt mechanics path. Examples 10-13 are
live-gated seam/provider proofs and skip by default unless the required
environment variables are set.

## Run

From `sdk/python/`:

```bash
uv sync                           # one-time
uv run python examples/01_runtime.py
just examples                     # run all thirteen in order; live-gated examples skip by default
just example 03                   # run just one (by number prefix)
LEAVEN_LIVE_CODEX=1 just example 10
LEAVEN_LIVE_OPENAI=1 just example 13
LEAVEN_LIVE_OPENAI=1 uv run --project examples/live_openai_lm live-openai-lm
```

## The Tour

| # | File | Proof class | Default verification | Shows |
|---|------|-------------|----------------------|-------|
| 01 | `01_runtime.py` | shape scaffold | `just examples` | Runtime composition slots and a minimal builder composition. |
| 02 | `02_cases_and_artifacts.py` | shape scaffold | `just examples` | Prompt artifacts, skill banks, JSONL cases, and split-tagged `lv.Case` records. |
| 03 | `03_prompt_optimize.py` | no-spend mechanics proof | `just examples`, `just example 03` | `lv.optimize(...).run()` over the durable `leaven seam serve --stdio --config` mechanics path for a `PromptArtifact` seed. This does not prove optimizer search, proposal application, or Rust checkpoint readback. |
| 04 | `04_evoskill_skill_bank.py` | shape scaffold | expected boundary in `just examples` | EvoSkill-class `SkillBank` composition shape. Current front door rejects non-`PromptArtifact` seeds. |
| 05 | `05_evaluator_with_judge.py` | shape scaffold | `just examples` | Advanced evaluator authoring shape and evidence vocabulary. |
| 06 | `06_reflect_propose_custom.py` | shape scaffold | expected boundary in `just examples` | Separated reflector/proposer authoring shape. Current front door rejects non-`PromptArtifact` seeds here. |
| 07 | `07_serve_stage_worker.py` | shape scaffold | `just examples` | Standalone worker declaration shape. This is not stdio worker execution proof. |
| 08 | `08_dspy_dropin.py` | optional-adapter scaffold | `just examples` without `dspy-ai` installed | DSPy adapter import/configuration shape. It is not LM execution proof. |
| 09 | `09_full_repro.py` | shape scaffold | expected boundary in `just examples` | Full front-door role composition shape. It is not a runnable product reproduction. |
| 10 | `10_live_codex_seam.py` | live-gated substrate proof | skips unless `LEAVEN_LIVE_CODEX=1` | Direct Python client proof for `leaven/agent.run` over `leaven seam serve --stdio --config`; not the finished engine-supplied `cx.agent.run` path. |
| 11 | `11_live_optimize_codex_stage.py` | live-gated product-path mechanics proof | skips unless `LEAVEN_LIVE_CODEX=1` | `lv.optimize(...).run()` dispatches a Python runner that calls `cx.agent.run` through the seam. Not Codex evolution proof. |
| 12 | `12_live_optimize_codex_proposer.py` | live-gated product-path mechanics proof | skips unless `LEAVEN_LIVE_CODEX=1` | A configured proposer calls `cx.agent.run` against `cx.parent_workspace` and submits a proposal batch. It does not prove proposal application. |
| 13 | `13_live_optimize_openai_lm.py` | live-gated product-path mechanics proof | skips unless `LEAVEN_LIVE_OPENAI=1` | A runner calls `cx.lm.complete` through the seam and validates text, usage, model, and receipt projection. |

## Fixtures

`fixtures/arithmetic.jsonl` — 8 trivial-to-medium arithmetic QA cases used by examples 03, 04, 06, 09. Each line is one `{id, input, target, metadata}` record matching the JSONL loader's default fields.

## What this is not

Shape scaffold examples do not run a real optimization. They exist so:

- You can read the file and the SHAPE of user code fires your taste
- IDE autocomplete works on every decorator, builder, and context object
- `ty` proves the type signatures hold across the full surface
- When the engine wires up behind the seam, the same example files run
  end-to-end without source changes
- Live-gated substrate proofs can exercise the real public seam without
  pretending the high-level SDK path is finished

When something feels wrong in an example, the spec at
[`../../../docs/specs/leaven_python.md`](../../../docs/specs/leaven_python.md) is the governing truth;
update the spec and these examples in the same change.
