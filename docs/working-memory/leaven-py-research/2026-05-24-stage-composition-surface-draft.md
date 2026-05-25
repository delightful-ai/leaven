# Stage Composition Surface Draft

Status: design draft, captured from the 2026-05-24 Python-surface conversation.

## Thesis

Leaven Python should feel like FlashEvolve stage composition over Inspect-style
tasks, without giving up Leaven's graph, evidence, target-isolation, and
workspace authority.

The high-level product shape is:

```python
evolution = lv.evolve(
    artifact=artifact,
    task=task,
    stages=lv.Stages(
        rollout=rollout,
        score=score,
        reflect=reflect,
        propose=propose,
        evaluate=evaluate,
    ),
    optimizer=optimizer,
    runtime=runtime,
)
```

Mental model:

```text
Evolution = Artifact x Task x swappable Stages x Optimizer x Runtime
```

## Ownership

- **Artifact** is the mutable behavior package: prompt, skill bank, directory,
  repo, DSPy program, harness, agent kit, playbook, or memory package.
- **Task** is the benchmark world: cases, inputs, hidden targets, files,
  setup requirements, sandbox requirements, split/provenance metadata.
- **Stages** are swappable algorithm roles. `Rollout` runs the current artifact
  on a case. `ScoreStage` scores the rollout with access to the prepared
  rollout workspace. `Reflect` diagnoses. `Propose` mutates the artifact.
  `Evaluate` ties rollout + score over a named split.
- **Layout** is a stage-owned workspace contract. It says where artifact,
  sample files, instructions, outputs, and mutable roots appear. It does not
  allocate workspaces.
- **Runtime** allocates workspaces/sandboxes, enforces trust and budget, runs
  LM/agent/command effects, and records receipts/evidence.

## Artifact vs Rollout

Do not put `entrypoint` on the base artifact concept. An artifact is what may
change. A rollout is how the current artifact version is interpreted for a
sample.

For a mutable Python harness:

```python
artifact = lv.artifacts.directory("./agent_harness")

rollout = lv.Rollout.command(
    argv=["uv", "run", "python", "target/current/run.py"],
    layout=lv.layouts.case_workspace(),
    output=lv.output.files(["output/result.json"]),
)
```

If the run command itself should evolve, put a manifest inside the artifact and
make the rollout read that manifest under policy. The stable rollout contract
then becomes "execute the current artifact's harness manifest", while the
manifest and code remain mutable under `target/current/`.

## Surface Rules

- Keep decorators as authoring sugar. Explicit stage objects are the composition
  surface.
- Keep task data inert. `Task` does not allocate workspaces.
- Keep layout off artifacts. Artifact adapters own projection/readback; stages
  own composition layouts.
- Keep public API small. Add one canonical spelling per concept; avoid synonym
  builders until a real workflow needs them.

## Current Scaffold Spike

The scaffold now exposes the smallest deliberate surface:

- `lv.Task`, `lv.Case`
- `lv.Stages`, `lv.Rollout`, `lv.ScoreStage`, `lv.Reflect`, `lv.Propose`, `lv.Evaluate`
- `lv.evolve(...)`
- `lv.runtime(...)` as the stage-composition spelling for the run environment
- namespaces `lv.artifacts`, `lv.layouts`, `lv.setup`

The old `lv.optimize(..., runner=..., scorer=...)` path remains as legacy/minimal
sugar while this surface is evaluated.
