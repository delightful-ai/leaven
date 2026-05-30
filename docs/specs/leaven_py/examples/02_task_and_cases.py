"""Example 02 — the task world: Task and Case with splits, files, setup.

`Task` declares an immutable task world: the case inventory plus task-global
runtime requirements (sandbox, splits). `Case` is one immutable case.
`split=` is a free user-defined LABEL string (NOT a fixed train/val/test
enum); one label per case. `files=` materialize via `lv.assets.path(...)`;
`setup=` runs a `lv.setup.bash(...)` step before rollout.

Governing spec: `docs/specs/leaven_python.md` — Task and Case.
"""

from __future__ import annotations

import leaven as lv


def main() -> None:
    try:
        # Hand-built cases. `split` labels are user-chosen ("train"/"held_out"),
        # not a fixed enum.
        task = lv.Task(
            cases=[
                lv.Case(
                    id="ctf-001",
                    input={"instructions": "find the flag."},
                    target={"flag": "picoCTF{...}"},
                    files={"challenge": lv.assets.path("assets/challenge")},
                    setup=lv.setup.bash("chmod +x case/files/challenge"),
                    split="train",
                    metadata={"difficulty": "medium"},
                ),
                lv.Case(
                    id="ctf-002",
                    input={"instructions": "find the second flag."},
                    target={"flag": "picoCTF{...}"},
                    split="held_out",
                ),
            ],
            sandbox=lv.sandbox.docker(image="python:3.12"),
        )
        print(f"hand-built task: {len(task.cases)} cases")

        # Loader sugar for big datasets: a label -> slice mapping defines splits.
        loaded = lv.Task(
            cases=lv.cases.from_jsonl(
                "fixtures/arithmetic.jsonl",
                splits={"train": slice(0, 6), "val": slice(6, 8)},
            ),
        )
        print(f"loaded task: {type(loaded).__name__!r}")
    except NotImplementedError as e:
        print(f"(expected) {e}")


if __name__ == "__main__":
    main()
