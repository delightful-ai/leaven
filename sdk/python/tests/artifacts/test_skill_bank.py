from pathlib import Path

import leaven as lv
from leaven.artifacts.skill_bank import SkillBank


def test_skill_bank_loads_markdown_files_from_directory_layout(tmp_path: Path) -> None:
    skills = tmp_path / "skills"
    references = tmp_path / "references"
    skills.mkdir()
    references.mkdir()
    (skills / "solver.md").write_text("# Solver\n\nUse arithmetic.\n", encoding="utf-8")
    (references / "rubric.md").write_text("# Rubric\n\nExact answer.\n", encoding="utf-8")
    (tmp_path / "notes.txt").write_text("ignored\n", encoding="utf-8")

    bank = SkillBank.from_directory(str(tmp_path))

    assert [file.path for file in bank.files] == [
        "references/rubric.md",
        "skills/solver.md",
    ]
    assert bank.files[0].content == "# Rubric\n\nExact answer.\n"
    assert bank.files[1].references == ["references/rubric.md"]
    assert bank.candidate_id is None


def test_top_level_skill_file_is_exported() -> None:
    file = lv.SkillFile(path="skills/solver.md", content="# Solver\n")
    assert file.references == []
