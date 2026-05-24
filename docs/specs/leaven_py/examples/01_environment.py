"""Example 01 — environment composition.

`lv.environment(...)` bundles workspace + LM(s) + agent(s) + sandbox + trust
profile + budget + cache into a single value passed to `lv.optimize(...)`.

This file shows every slot the environment accepts. It does not run an
optimization — just composes the typed config and prints its shape.
"""

from __future__ import annotations

import leaven as lv


def main() -> None:
    # Minimal: one LM, default sandbox, default cache, trusted local profile.
    minimal = lv.environment.local(budget=lv.budget(usd=10))
    print("minimal:", minimal.trust_profile, "/ budget:", minimal.budget)

    # Full: every slot wired explicitly.
    env = lv.environment(
        workspace=lv.workspace.local(root=".agents"),
        lm={
            "executor": lv.lm.anthropic(model="claude-opus-4-7"),
            "grader": lv.lm.openai(model="gpt-5", reasoning_effort="medium"),
            "reflector": lv.lm.anthropic(model="claude-opus-4-7"),
        },
        agent={
            "executor": lv.agent.codex(model="gpt-5-codex"),
            "judge": lv.agent.claude_code(model="claude-opus-4-7"),
        },
        sandbox=lv.sandbox.docker(image="python:3.12"),
        trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
        budget=lv.budget(usd=200, calls=2000, lm_tokens=10_000_000),
        cache=lv.environment.cache.sqlite_default(),
    )

    print()
    print("full:")
    print("  workspace :", env.workspace.backend, "@", env.workspace.root)  # type: ignore[attr-defined]
    print("  lm roles  :", sorted(env.lm) if isinstance(env.lm, dict) else env.lm)
    print("  agent     :", sorted(env.agent) if isinstance(env.agent, dict) else env.agent)
    print("  sandbox   :", env.sandbox.backend if env.sandbox else None)
    print("  trust     :", env.trust_profile.value)
    print("  budget    :", env.budget)
    print("  cache     :", env.cache_config.backend if env.cache_config else "(engine default)")


if __name__ == "__main__":
    main()
