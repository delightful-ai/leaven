# leaven

Python authoring surface for Leaven. This is the real in-repo Python SDK
project: importable package, dependency declaration, examples, tests, codegen,
and private public-seam client substrate.

Most high-level SDK calls are still scaffold. Example 03 runs the no-spend
prompt path, and example 10 is a live-gated Codex public-seam proof.

Spec: [`docs/specs/leaven_python.md`](../../docs/specs/leaven_python.md)

```bash
cd sdk/python
uv sync
```

```python
import leaven as lv
```

## Examples

Open [`examples/`](examples/README.md) — the numbered tour files read like real programs:

- **`03_prompt_optimize.py`** — the minimal ~25-line run
- **`01_runtime.py`** — how `lv.runtime(...)` composes
- **`04_evoskill_skill_bank.py`** — GEPA + SkillBank config
- **`05_evaluator_with_judge.py`** — the full evaluator shape from the public seam

## Verify

```bash
just check
```
