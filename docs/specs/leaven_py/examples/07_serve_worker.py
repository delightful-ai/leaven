"""Example 07 — the out-of-process stage worker (`lv.serve`).

You normally never write a worker entry point: in `lv.evolve(...).run()` the
SDK manages the worker lifecycle and serves your Python stage functions back
over the seam transparently. `lv.serve(...)` exists ONLY for the other
deployment mode — when something other than your script drives the run (a
`leaven` CLI invocation, a cloud/multi-tenant engine) and your script is
launched purely as a stage worker.

The locked seam INVERTS ordinary ACP: the engine is the ACP client/driver;
this Python worker is the ACP agent. `lv.serve` registers ONLY the
Python-authored stages it is given. Engine-mediated built-ins (`Rollout.agent`,
`Reflect.agent`, `Propose.agent_edit`) are configured in the plan, NOT served
here. The functions are identical to the ones passed to
`lv.evolve(stages=...)`; only the lifecycle differs.

Governing spec: `docs/specs/leaven_python.md` — How Python code reaches the
engine (the ACP worker model) / lv.serve.
"""

from __future__ import annotations

import leaven as lv
import leaven.adapters


# The Python-authored stages (same functions you would pass to lv.evolve).
@lv.runner
async def run(artifact: lv.artifacts.PromptArtifact, case: lv.Case, cx: lv.adapters.RunContext) -> str:
    return (await cx.lm.complete_text(artifact.render(**case.input))).strip()


@lv.scorer
async def correctness(run: lv.RolloutResult[str], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    expected = (case.target or {})["answer"]
    return lv.Score(value=float(run.output == expected), feedback=f"got {run.output!r}")


def main() -> None:
    # Launched by an external engine over ACP stdio. Registers only the
    # Python-authored stages; engine-mediated built-ins are not served.
    try:
        lv.serve(rollout=run, score=correctness)
    except NotImplementedError as e:
        print(f"(expected) {e}")


if __name__ == "__main__":
    main()
