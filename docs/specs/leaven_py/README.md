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

Open [`examples/`](examples/README.md) — four short files that read like real programs:

- **`prompt_optimize.py`** — the minimal ~25-line run
- **`environment.py`** — how `lv.environment(...)` composes
- **`gepa_skill_bank.py`** — GEPA + SkillBank config
- **`evaluator.py`** — the full evaluator shape from the public seam

## Verify

```bash
just check
```
