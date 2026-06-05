from pathlib import Path

from leaven._runs.store import list_run_dirs


def test_list_run_dirs_returns_only_rust_checkpoint_runs(tmp_path: Path) -> None:
    checkpoint_run = tmp_path / "run_with_checkpoint"
    (checkpoint_run / "checkpoints").mkdir(parents=True)
    (checkpoint_run / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")

    optimized_json_only = tmp_path / "python_projection_only"
    optimized_json_only.mkdir()
    (optimized_json_only / "optimized.json").write_text("{}", encoding="utf-8")

    plain_dir = tmp_path / "plain_dir"
    plain_dir.mkdir()

    assert list_run_dirs(tmp_path) == ["run_with_checkpoint"]


def test_list_run_dirs_returns_empty_for_missing_root(tmp_path: Path) -> None:
    assert list_run_dirs(tmp_path / "missing") == []
