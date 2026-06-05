"""Tests for `leaven._runs.rust_export`."""

import json
from pathlib import Path

import pytest

from leaven import runs
from leaven._runs import optimized_from_rust_readback
from leaven._runs.rust_export import load_rust_blob_readback, load_rust_run_readback
from leaven.run_inspection import RustRunReadback


def test_load_rust_run_readback_skips_runs_without_latest_checkpoint(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir()

    assert load_rust_run_readback(run_dir, leaven_bin=tmp_path / "missing") is None


def test_load_rust_run_readback_invokes_leaven_run_inspect(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    calls = tmp_path / "calls.txt"
    fake = tmp_path / "leaven"
    fake.write_text(
        "#!/bin/sh\n"
        "printf '%s\\n' \"$@\" > \"$LEAVEN_TEST_CALLS\"\n"
        "cat <<'JSON'\n"
        "{\n"
        '  "schema_version": "leaven.run_inspection_export.v1",\n'
        '  "run_id": "run_1",\n'
        '  "latest_checkpoint": "checkpoint_1",\n'
        '  "checkpoint": {\n'
        '    "format_version": 1,\n'
        '    "graph_snapshot": {\n'
        '      "store": "file",\n'
        '      "key": "graph.blob",\n'
        '      "schema": "060606",\n'
        '      "format": "Json"\n'
        "    },\n"
        '    "artifact_refs": [{"store": "file", "key": "artifact.blob"}],\n'
        '    "artifact_ref_count": 1,\n'
        '    "evidence_refs": [{"store": "evidence", "key": "evidence.json"}],\n'
        '    "evidence_ref_count": 1,\n'
        '    "stage_journal_refs": [{"store": "file", "key": "stage.blob"}],\n'
        '    "stage_journal_ref_count": 1,\n'
        '    "workspace_journal_refs": [{"store": "file", "key": "workspace.blob"}],\n'
        '    "workspace_journal_ref_count": 1,\n'
        '    "has_optimizer_state": false,\n'
        '    "has_cache_index": false\n'
        "  },\n"
        '  "graph": {\n'
        '    "blob": {\n'
        '      "store": "file",\n'
        '      "key": "graph.blob",\n'
        '      "schema": "060606",\n'
        '      "format": "Json"\n'
        "    },\n"
        '    "bytes": 128,\n'
        '    "run_id": "run_graph",\n'
        '    "best_candidate_id": "cand_child",\n'
        '    "candidates": [\n'
        '      {"id": "cand_seed", "parent_id": null, "artifact": {"template": "seed"}},\n'
        '      {"id": "cand_child", "parent_id": "cand_seed", "artifact": {"template": "child"}}\n'
        "    ],\n"
        '    "candidate_count": 2,\n'
        '    "proposal_batch_count": 1,\n'
        '    "proposal_count": 1,\n'
        '    "apply_attempt_count": 1,\n'
        '    "evaluation_request_count": 1,\n'
        '    "assessment_count": 2,\n'
        '    "event_count": 3\n'
        "  },\n"
        '  "cost": {\n'
        '    "metric_calls": 0,\n'
        '    "lm_calls": 2,\n'
        '    "prompt_tokens": 7,\n'
        '    "completion_tokens": 11,\n'
        '    "seconds": 0.0\n'
        "  }\n"
        "}\n"
        "JSON\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)

    with pytest.MonkeyPatch.context() as monkeypatch:
        monkeypatch.setenv("LEAVEN_TEST_CALLS", str(calls))
        readback = load_rust_run_readback(run_dir, leaven_bin=fake)

    assert readback is not None
    assert readback.latest_checkpoint == "checkpoint_1"
    assert readback.checkpoint.artifact_refs[0].key == "artifact.blob"
    assert readback.checkpoint.artifact_ref_count == 1
    assert readback.checkpoint.evidence_refs[0].key == "evidence.json"
    assert readback.checkpoint.evidence_ref_count == 1
    assert readback.checkpoint.stage_journal_refs[0].key == "stage.blob"
    assert readback.checkpoint.stage_journal_ref_count == 1
    assert readback.checkpoint.workspace_journal_refs[0].key == "workspace.blob"
    assert readback.checkpoint.workspace_journal_ref_count == 1
    assert readback.graph.candidate_count == 2
    assert readback.graph.bytes == 128
    assert calls.read_text(encoding="utf-8").splitlines() == [
        "run",
        "inspect",
        "--run-dir",
        str(run_dir),
    ]


def test_load_rust_blob_readback_invokes_leaven_run_blob(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir()
    calls = tmp_path / "calls.txt"
    fake = tmp_path / "leaven"
    fake.write_text(
        "#!/bin/sh\n"
        "printf '%s\\n' \"$@\" > \"$LEAVEN_TEST_CALLS\"\n"
        "cat <<'JSON'\n"
        "{\n"
        '  "schema_version": "leaven.run_blob_export.v1",\n'
        '  "blob": {"store": "file", "key": "graph.blob"},\n'
        '  "bytes": 19,\n'
        '  "sha256": "cab11e0c83798e18f101ec99395ac4ebbf38c1739abe06a70ec8264954bf0bd8",\n'
        '  "content_base64": "ZHVyYWJsZSBibG9iIGJ5dGVzCg=="\n'
        "}\n"
        "JSON\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)
    readback = load_rust_run_readback_fixture()

    with pytest.MonkeyPatch.context() as monkeypatch:
        monkeypatch.setenv("LEAVEN_TEST_CALLS", str(calls))
        blob = load_rust_blob_readback(run_dir, readback.graph.blob, leaven_bin=fake)

    assert blob.bytes == 19
    assert blob.blob.store == "file"
    assert blob.blob.key == "graph.blob"
    assert blob.content_bytes() == b"durable blob bytes\n"
    assert calls.read_text(encoding="utf-8").splitlines() == [
        "run",
        "blob",
        "--run-dir",
        str(run_dir),
        "--store",
        "file",
        "--key",
        "graph.blob",
    ]


def test_load_rust_run_readback_reports_cli_failure(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    fake = tmp_path / "leaven"
    fake.write_text("#!/bin/sh\necho nope >&2\nexit 7\n", encoding="utf-8")
    fake.chmod(0o755)

    with pytest.raises(RuntimeError, match="leaven run inspect failed"):
        load_rust_run_readback(run_dir, leaven_bin=fake)


def test_load_rust_blob_readback_reports_cli_failure(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir()
    fake = tmp_path / "leaven"
    fake.write_text("#!/bin/sh\necho nope >&2\nexit 7\n", encoding="utf-8")
    fake.chmod(0o755)
    readback = load_rust_run_readback_fixture()

    with pytest.raises(RuntimeError, match="leaven run blob failed"):
        load_rust_blob_readback(run_dir, readback.graph.blob, leaven_bin=fake)


def test_optimized_from_rust_readback_uses_graph_candidates() -> None:
    readback = load_rust_run_readback_fixture()

    result = optimized_from_rust_readback(readback, run_dir="/tmp/run")

    assert result.run_id == "run_1"
    assert result.best.id == "cand_child"
    assert result.best.parent_id == "cand_seed"
    assert result.best.artifact == {"template": "child"}
    assert [candidate.id for candidate in result.lineage("cand_child")] == [
        "cand_child",
        "cand_seed",
    ]
    assert result.summary.run_dir == "/tmp/run"
    assert result.summary.cost_status == "unsupported_dependency"
    assert result.summary.total_lm_tokens == 18
    assert result.summary.usage_status == "known"
    assert [fact.surface for fact in result.summary.unsupported] == [
        "run.cost",
        "run.inspection",
    ]


def test_runs_open_prefers_rust_checkpoint_without_optimized_json(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    fake = tmp_path / "leaven"
    output = tmp_path / "readback.json"
    output.write_text(
        json.dumps(load_rust_run_readback_fixture().model_dump(mode="json", by_alias=True)),
        encoding="utf-8",
    )
    fake.write_text(f"#!/bin/sh\ncat {output}\n", encoding="utf-8")
    fake.chmod(0o755)
    monkeypatch.setenv("LEAVEN_BIN", str(fake))

    result = runs.open(run_dir)

    assert result.best.id == "cand_child"
    assert result.summary.run_dir == str(run_dir)
    assert [candidate.id for candidate in result.lineage("cand_child")] == [
        "cand_child",
        "cand_seed",
    ]


def load_rust_run_readback_fixture() -> RustRunReadback:
    return RustRunReadback.model_validate(
        {
            "schema_version": "leaven.run_inspection_export.v1",
            "run_id": "run_1",
            "latest_checkpoint": "checkpoint_1",
            "checkpoint": {
                "format_version": 1,
                "graph_snapshot": {
                    "store": "file",
                    "key": "graph.blob",
                    "schema": "060606",
                    "format": "Json",
                },
                "artifact_refs": [],
                "artifact_ref_count": 0,
                "evidence_refs": [],
                "evidence_ref_count": 0,
                "stage_journal_refs": [],
                "stage_journal_ref_count": 0,
                "workspace_journal_refs": [],
                "workspace_journal_ref_count": 0,
                "has_optimizer_state": False,
                "has_cache_index": False,
            },
            "graph": {
                "blob": {
                    "store": "file",
                    "key": "graph.blob",
                    "schema": "060606",
                    "format": "Json",
                },
                "bytes": 128,
                "run_id": "run_graph",
                "best_candidate_id": "cand_child",
                "candidates": [
                    {
                        "id": "cand_seed",
                        "parent_id": None,
                        "artifact": {"template": "seed"},
                    },
                    {
                        "id": "cand_child",
                        "parent_id": "cand_seed",
                        "artifact": {"template": "child"},
                    },
                ],
                "candidate_count": 2,
                "proposal_batch_count": 1,
                "proposal_count": 1,
                "apply_attempt_count": 1,
                "evaluation_request_count": 1,
                "assessment_count": 2,
                "event_count": 3,
            },
            "cost": {
                "metric_calls": 0,
                "lm_calls": 2,
                "prompt_tokens": 7,
                "completion_tokens": 11,
                "seconds": 0.0,
            },
        }
    )
