"""Example 01 — compose a runtime (the execution substrate).

`runtime` is the execution substrate: LMs, the engine-mediated agent
executor, sandbox, workspace backend, trust profile, budget, cache. It is
NOT the task world (that is `Task`/`Case`) and NOT artifact state (the
mutable behavior package). `runtime.agent` is the engine-mediated executor:
mutating Codex-shaped artifact behavior never changes which agent the
runtime spawns.

Every builder body raises `NotImplementedError` in the scaffold; the point
is the SHAPE of the composition and that it typechecks. Nothing runs.

Governing spec: `docs/specs/leaven_python.md` — Runtime.
"""

from __future__ import annotations

import leaven as lv


def main() -> None:
    try:
        # The full constructor — every substrate slot.
        full = lv.runtime(
            lm=lv.lm.anthropic(model="claude-opus-4-7"),
            agent=lv.agent.codex(model="gpt-5.5"),
            sandbox=lv.sandbox.docker(image="python:3.12"),
            workspace=lv.workspace.local(root=".leaven/work"),
            trust_profile=lv.trust.managed_sandbox,
            budget=lv.budget(usd=200, calls=2000),
        )

        # Multiple LMs and role-keyed agents are allowed.
        multi = lv.runtime(
            lm=[lv.lm.anthropic(model="claude-opus-4-7"), lv.lm.openai(model="gpt-5.5")],
            agent={
                "executor": lv.agent.codex(model="gpt-5-codex"),
                "judge": lv.agent.command(["my-judge-cli", "--stdio"]),
            },
            trust_profile="managed_sandbox",
        )

        # Convenience shortcuts for common shapes.
        local = lv.runtime.local(budget=lv.budget(usd=20))
        acp = lv.runtime.acp(worker="leaven serve --stdio", budget=lv.budget(usd=50))

        for label, rt in [("full", full), ("multi", multi), ("local", local), ("acp", acp)]:
            print(f"{label}: composed {type(rt).__name__!r}")
    except NotImplementedError as e:
        print(f"(expected) {e}")


if __name__ == "__main__":
    main()
