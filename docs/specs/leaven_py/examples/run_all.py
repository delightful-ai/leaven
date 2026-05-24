"""Run every numbered example script in order.

Each example raises NotImplementedError at the engine boundary; this
runner catches the exception and reports the example as expected-failed
so the tour completes end-to-end without crashing.
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
        print(f"\n=== {script.name} " + "=" * (60 - len(script.name)))
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
                except NotImplementedError as e:
                    print(f"(expected) {e}")
        except Exception as e:
            failures.append((script.name, f"{type(e).__name__}: {e}"))
            traceback.print_exc(limit=2)

    print("\n" + "=" * 70)
    if failures:
        print(f"{len(failures)} example(s) hit unexpected failures:")
        for name, msg in failures:
            print(f"  {name}: {msg}")
        return 1
    print(f"all {len(scripts)} examples completed (NotImplementedError at engine boundaries is expected).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
