"""Run every numbered example script in order.

Most scaffold examples raise NotImplementedError at the engine boundary; this
runner catches that and reports the example as expected-failed so the tour
completes end-to-end without crashing. Live-gated examples are responsible for
skipping themselves unless their opt-in env vars are set.
"""

from __future__ import annotations

import importlib.util
import sys
import traceback
from pathlib import Path

HERE = Path(__file__).parent


def main() -> int:
    scripts = sorted(p for p in HERE.glob("[0-9][0-9]_*.py") if p.is_file())
    if not scripts:
        print("no numbered examples found", file=sys.stderr)
        return 1

    failures: list[tuple[str, str]] = []
    for script in scripts:
        print(f"\n=== {script.name} " + "=" * (60 - len(script.name)), flush=True)
        spec = importlib.util.spec_from_file_location(script.stem, script)
        if spec is None or spec.loader is None:
            failures.append((script.name, "could not load spec"))
            continue
        module = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(module)
            if hasattr(module, "main"):
                module.main()
            elif hasattr(module, "amain"):
                import asyncio

                try:
                    asyncio.run(module.amain())
                except Exception as e:
                    if _is_expected_boundary_error(e):
                        print(f"(expected) {e}")
                    else:
                        raise
        except Exception as e:
            failures.append((script.name, f"{type(e).__name__}: {e}"))
            traceback.print_exc(limit=2)

    print("\n" + "=" * 70)
    if failures:
        print(f"{len(failures)} example(s) hit unexpected failures:")
        for name, msg in failures:
            print(f"  {name}: {msg}")
        return 1
    print(f"all {len(scripts)} examples completed (scaffold engine boundaries are expected).")
    return 0


def _is_expected_boundary_error(error: Exception) -> bool:
    """Return whether an example hit a known scaffold engine boundary."""
    if isinstance(error, NotImplementedError):
        return True
    return isinstance(error, TypeError) and str(error).startswith(
        "this slice optimizes a PromptArtifact seed"
    )


if __name__ == "__main__":
    raise SystemExit(main())
