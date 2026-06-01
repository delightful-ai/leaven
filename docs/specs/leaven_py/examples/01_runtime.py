"""Example 01 — runtime composition.

`lv.runtime(...)` bundles workspace + LM(s) + agent(s) + sandbox + trust
profile + budget + cache into a single value passed to `lv.optimize(...)`.

This file shows every slot the runtime accepts, then composes a minimal
`lv.optimize(seed=..., environment=..., optimizer=..., runtime=...)` so the
runtime's place in the front door is visible. It does not run an
optimization — just composes the typed config and prints its shape.
"""

from __future__ import annotations

import leaven as lv


def main() -> None:
    # Minimal: one mock LM, default sandbox, default cache, trusted local profile.
    minimal = lv.runtime.local(budget=lv.budget(usd=10))
    print("minimal:", minimal.trust_profile, "/ budget:", minimal.budget)

    # Full: every slot wired explicitly.
    runtime = lv.runtime(
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
        cache=lv.runtime.cache.sqlite_default(),
    )

    print()
    print("full:")
    print("  workspace :", runtime.workspace.backend, "@", runtime.workspace.root)  # type: ignore[attr-defined]
    print("  lm roles  :", sorted(runtime.lm) if isinstance(runtime.lm, dict) else runtime.lm)
    print("  agent     :", sorted(runtime.agent) if isinstance(runtime.agent, dict) else runtime.agent)
    print("  sandbox   :", runtime.sandbox.backend if runtime.sandbox else None)
    print("  trust     :", runtime.trust_profile.value)
    print("  budget    :", runtime.budget)
    print("  cache     :", runtime.cache_config.backend if runtime.cache_config else "(engine default)")

    # Where the runtime lands: the fourth argument to `lv.optimize(...)`, beside
    # the seed artifact, the environment (task + rollout + rubric), and the
    # optimizer. The runtime supplies execution, budget, and trust; it owns no
    # task or scoring semantics.
    builder = lv.optimize(
        seed=lv.PromptArtifact(template="Answer: {question}\nA:"),
        environment=lv.Environment(
            task=lv.Task(cases=[]),
            rollout=lv.Rollout.agent(),
            rubric=lv.Rubric([]),
        ),
        optimizer=lv.optimizers.gepa(population_size=4),
        runtime=minimal,
    )
    print()
    print("composed builder runtime trust:", builder.runtime.trust_profile.value)


if __name__ == "__main__":
    main()
