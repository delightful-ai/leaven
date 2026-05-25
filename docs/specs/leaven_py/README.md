# leaven

Python authoring surface for Leaven. **Scaffold only** — importable types and
signatures, not a running engine.

Spec: [`docs/specs/leaven_python.md`](../leaven_python.md)

```bash
cd docs/specs/leaven_py
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
