"""Run every numbered example script in order.

Only explicitly classified shape or optional-adapter examples may stop at known
boundary errors. No-spend and live-gated mechanics examples must fail loudly if
they hit a scaffold boundary.
"""

import importlib.util
import sys
import traceback
from pathlib import Path

HERE = Path(__file__).parent
EXPECTED_BOUNDARY_EXAMPLES = {
    "04_evoskill_skill_bank.py": "unsupported-seed front-door boundary",
    "06_reflect_propose_custom.py": "unsupported-seed front-door boundary",
    "09_full_repro.py": "unsupported-seed front-door boundary",
}


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
                    if _is_expected_boundary_error(script.name, e):
                        print(f"(expected: {EXPECTED_BOUNDARY_EXAMPLES[script.name]}) {e}")
                    else:
                        raise
        except Exception as e:
            if _is_expected_boundary_error(script.name, e):
                print(f"(expected: {EXPECTED_BOUNDARY_EXAMPLES[script.name]}) {e}")
                continue
            failures.append((script.name, f"{type(e).__name__}: {e}"))
            traceback.print_exc(limit=2)

    print("\n" + "=" * 70)
    if failures:
        print(f"{len(failures)} example(s) hit unexpected failures:")
        for name, msg in failures:
            print(f"  {name}: {msg}")
        return 1
    print(f"all {len(scripts)} examples completed with explicit maturity classifications.")
    return 0


def _is_expected_boundary_error(script_name: str, error: Exception) -> bool:
    """Return whether a named example hit its documented scaffold boundary."""
    if script_name not in EXPECTED_BOUNDARY_EXAMPLES:
        return False
    return isinstance(error, TypeError) and str(error).startswith(
        "lv.optimize optimizes a PromptArtifact or AgentKitArtifact seed"
    )


if __name__ == "__main__":
    raise SystemExit(main())
