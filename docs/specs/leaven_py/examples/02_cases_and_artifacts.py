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

    # train = lv.cases.from_jsonl(str(FIXTURE), name="arithmetic-train", limit=6)
    # val   = lv.cases.from_jsonl(str(FIXTURE), name="arithmetic-val", limit=2)
    # splits = lv.cases.splits(train=train, val=val)

    # Manual construction (works today; no loader involved).
    case = lv.Case(
        id="ar-demo",
        input={"question": "2 + 3"},
        target={"answer": "5"},
        metadata={"difficulty": "trivial"},
    )
    print()
    print("hand-built case:", case.id, "/", case.input, "→", case.target)


if __name__ == "__main__":
    main()
