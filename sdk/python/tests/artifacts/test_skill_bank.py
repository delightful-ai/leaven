from pathlib import Path

import leaven as lv
from leaven.artifacts.skill_bank import (
    SkillBank,
    SkillBankAtomicChange,
    SkillBankChangeFile,
    SkillBankRenameFileChange,
    SkillBankRenameSkillChange,
    SkillBankWriteFileChange,
)


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


def test_skill_bank_write_file_change_projects_to_json_literal() -> None:
    change = SkillBankWriteFileChange(
        skill="alpha",
        path="SKILL.md",
        file=SkillBankChangeFile(content="# Alpha\n", executable=True),
    )

    assert change.to_json_value() == {
        "kind": "write_file",
        "skill": "alpha",
        "path": "SKILL.md",
        "file": {"content": "# Alpha\n", "executable": True},
    }


def test_skill_bank_change_aliases_match_rust_variant_fields() -> None:
    rename_skill = SkillBankRenameSkillChange(from_name="alpha", to="beta")
    rename_file = SkillBankRenameFileChange(skill="beta", from_path="old.md", to="new.md")

    assert rename_skill.to_json_value() == {
        "kind": "rename_skill",
        "from": "alpha",
        "to": "beta",
    }
    assert rename_file.to_json_value() == {
        "kind": "rename_file",
        "skill": "beta",
        "from": "old.md",
        "to": "new.md",
    }


def test_skill_bank_atomic_change_preserves_nested_variant_shapes() -> None:
    change = SkillBankAtomicChange(
        changes=[
            SkillBankWriteFileChange(
                skill="alpha",
                path="SKILL.md",
                file=SkillBankChangeFile(content="# Alpha\n"),
            )
        ]
    )

    assert change.to_json_value() == {
        "kind": "atomic",
        "changes": [
            {
                "kind": "write_file",
                "skill": "alpha",
                "path": "SKILL.md",
                "file": {"content": "# Alpha\n", "executable": False},
            }
        ],
    }


def test_top_level_common_skill_change_constructors_are_exported() -> None:
    change = lv.SkillBankWriteFileChange(
        skill="alpha",
        path="SKILL.md",
        file=lv.SkillBankChangeFile(content="# Alpha\n"),
    )

    assert change.file.content == "# Alpha\n"
