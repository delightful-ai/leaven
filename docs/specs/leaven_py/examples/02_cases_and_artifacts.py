"""Example 02 — cases and artifacts.

Two things users build at the boundary:

- Cases: loaded from JSONL/Parquet/CSV via `lv.cases.from_*(...)`. The
  loader produces a `CaseSet` of typed `Case` records.
- Artifacts: the thing being optimized. `PromptArtifact` and `SkillBank`
  are built-in; user code typically constructs the seed by hand.

This file shows both shapes side-by-side. It does not call any loader
(since they raise NotImplementedError), but the construction pattern is
exactly what the user writes.
"""

from __future__ import annotations

from pathlib import Path

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


def main() -> None:
    # ---- Artifacts ----------------------------------------------------------
    prompt = lv.PromptArtifact(
        template="Answer the arithmetic question:\nQ: {question}\nA:",
        examples=["Q: 1 + 1\nA: 2"],
    )
    print("prompt artifact:")
    print("  template      :", repr(prompt.template))
    print("  examples count:", len(prompt.examples))

    empty_bank = lv.SkillBank.empty()
    print()
    print("empty skill bank:", empty_bank)

    seeded_bank = lv.SkillBank(
        files=[
            lv.SkillFile(
                path="skills/arithmetic.md",
                content="# Arithmetic\n\nEvaluate the expression left-to-right.",
            ),
            lv.SkillFile(
                path="skills/numeric-format.md",
                content="# Numeric Format\n\nReturn integers without trailing zeros.",
                references=["references/style-guide.md"],
            ),
        ],
    )
    print("seeded skill bank:", len(seeded_bank.files), "files")

    # ---- Case loaders -------------------------------------------------------
    # These calls raise NotImplementedError in the scaffold; the construction
    # pattern is what matters. The fixture file exists at:
    print()
    print("fixture path :", FIXTURE)
    print("fixture lines:", sum(1 for _ in FIXTURE.open()))

    # cases = lv.cases.from_jsonl(str(FIXTURE), name="arithmetic", limit=8)
    # The engine reads train/val/test from each Case's `split` tag — there is
    # no separate `train=`/`val=` argument. Loaders carry the tag through from
    # the source rows; hand-built cases set it directly (below).

    # Manual construction (works today; no loader involved). `split=` is the tag
    # the engine reads when it lowers a `Task` into train/val/test sets.
    train_case = lv.Case(
        id="ar-train",
        input={"question": "2 + 3"},
        target={"answer": "5"},
        metadata={"difficulty": "trivial"},
        split="train",
    )
    val_case = lv.Case(
        id="ar-val",
        input={"question": "7 + 6"},
        target={"answer": "13"},
        metadata={"difficulty": "trivial"},
        split="val",
    )
    print()
    print("hand-built train case:", train_case.id, "/", train_case.input, "→", train_case.target, f"[{train_case.split}]")
    print("hand-built val case  :", val_case.id, "/", val_case.input, "→", val_case.target, f"[{val_case.split}]")


if __name__ == "__main__":
    main()
