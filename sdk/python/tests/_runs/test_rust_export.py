"""Tests for `leaven._runs.rust_export`."""

import base64
import hashlib
import json
from pathlib import Path

import pytest

from leaven import PromptArtifact, runs
from leaven._receipts import WriteReceipt
from leaven._runs import optimized_from_rust_readback
from leaven._runs.rust_export import (
    load_rust_blob_readback,
    load_rust_evidence_readback,
    load_rust_run_readback,
)
from leaven.assessment import Assessment
from leaven.case import Case
from leaven.evidence import EvidenceEnvelope, EvidencePublicPayload
from leaven.run_inspection import RustRunReadback
from leaven.score import Score
from tests.support.rust_evidence import rust_case_assessment_evidence_bytes


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
        'printf \'%s\\n\' "$@" > "$LEAVEN_TEST_CALLS"\n'
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
        '    "assessments": [\n'
        "      {\n"
        '        "id": "assessment_child",\n'
        '        "request_id": "eval_req_child",\n'
        '        "evaluator": "evaluator/exact",\n'
        '        "target_kind": "independent",\n'
        '        "candidate_ids": ["cand_child"],\n'
        '        "target": {"Independent": {"candidate": "cand_child", "target": "Unscoped"}},\n'
        '        "evidence": {"store": "leaven-run", "key": "0"},\n'
        '        "metadata": {"split": "validation"},\n'
        '        "created_at": "2026-06-04T00:00:02Z"\n'
        "      }\n"
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
        'printf \'%s\\n\' "$@" > "$LEAVEN_TEST_CALLS"\n'
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


def test_load_rust_evidence_readback_invokes_leaven_run_evidence(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir()
    calls = tmp_path / "calls.txt"
    fake = tmp_path / "leaven"
    fake.write_text(
        "#!/bin/sh\n"
        'printf \'%s\\n\' "$@" > "$LEAVEN_TEST_CALLS"\n'
        "cat <<'JSON'\n"
        "{\n"
        '  "schema_version": "leaven.run_evidence_export.v1",\n'
        '  "evidence": {"store": "leaven-run", "key": "0"},\n'
        '  "bytes": 23,\n'
        '  "sha256": "5369812b12c948efac405e61b4e926b1f639ff019922e88d0c1955f981b103aa",\n'
        '  "content_base64": "eyJzY29yZSI6MSwiY2FzZSI6ImEifQo="\n'
        "}\n"
        "JSON\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)
    readback = load_rust_run_readback_fixture()

    with pytest.MonkeyPatch.context() as monkeypatch:
        monkeypatch.setenv("LEAVEN_TEST_CALLS", str(calls))
        evidence = load_rust_evidence_readback(
            run_dir,
            readback.graph.assessments[0].evidence,
            leaven_bin=fake,
        )

    assert evidence.bytes == 23
    assert evidence.evidence.store == "leaven-run"
    assert evidence.evidence.key == "0"
    assert evidence.content_bytes() == b'{"score":1,"case":"a"}\n'
    assert evidence.content_json() == {"score": 1, "case": "a"}
    assert calls.read_text(encoding="utf-8").splitlines() == [
        "run",
        "evidence",
        "--run-dir",
        str(run_dir),
        "--store",
        "leaven-run",
        "--key",
        "0",
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


def test_load_rust_evidence_readback_reports_cli_failure(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir()
    fake = tmp_path / "leaven"
    fake.write_text("#!/bin/sh\necho nope >&2\nexit 7\n", encoding="utf-8")
    fake.chmod(0o755)
    readback = load_rust_run_readback_fixture()

    with pytest.raises(RuntimeError, match="leaven run evidence failed"):
        load_rust_evidence_readback(
            run_dir,
            readback.graph.assessments[0].evidence,
            leaven_bin=fake,
        )


def test_optimized_from_rust_readback_uses_graph_candidates() -> None:
    readback = load_rust_run_readback_fixture()

    result = optimized_from_rust_readback(readback, run_dir="/tmp/run")

    assert result.run_id == "run_1"
    assert result.best.id == "cand_child"
    assert result.best.parent_id == "cand_seed"
    assert result.best.artifact == PromptArtifact(template="child")
    assert [candidate.id for candidate in result.lineage("cand_child")] == [
        "cand_child",
        "cand_seed",
    ]
    assert result.summary.run_dir == "/tmp/run"
    assert readback.graph.assessments[0].id == "assessment_child"
    assert readback.graph.assessments[0].candidate_ids == ["cand_child"]
    assert readback.graph.assessments[0].evidence.key == "0"
    assert readback.graph.proposal_batches[0].id == "pb_child"
    assert readback.graph.proposal_batches[0].proposal_ids == ["prop_child"]
    assert [receipt.receipt_id for receipt in result.proposal_receipts] == ["pb_child"]
    assert result.proposal_receipts[0].proposal_ids == ["prop_child"]
    assert result.summary.cost_status == "unsupported_dependency"
    assert result.summary.total_lm_tokens == 18
    assert result.summary.usage_status == "known"
    assert [fact.surface for fact in result.summary.unsupported] == [
        "run.cost",
        "run.inspection",
    ]


def test_summary_score_ignores_train_screening_assessments() -> None:
    """Regression: reopen must not dilute validation scores with GEPA train rows."""

    readback = load_rust_run_readback_fixture()
    data = readback.model_dump(mode="json", by_alias=True)
    data["graph"]["assessments"] = [
        {
            "id": "assessment_train_a",
            "request_id": "eval_req_search",
            "evaluator": "evaluator/exact",
            "target_kind": "independent",
            "candidate_ids": ["cand_child"],
            "target": {"Independent": {"candidate": "cand_child", "target": "Unscoped"}},
            "evidence": {"store": "leaven-run", "key": "train-a"},
            "metadata": {},
            "purpose": "Search",
            "created_at": "2026-06-04T00:00:01Z",
        },
        {
            "id": "assessment_train_b",
            "request_id": "eval_req_search",
            "evaluator": "evaluator/exact",
            "target_kind": "independent",
            "candidate_ids": ["cand_child"],
            "target": {"Independent": {"candidate": "cand_child", "target": "Unscoped"}},
            "evidence": {"store": "leaven-run", "key": "train-b"},
            "metadata": {},
            "purpose": "Search",
            "created_at": "2026-06-04T00:00:01Z",
        },
        {
            "id": "assessment_val",
            "request_id": "eval_req_validation",
            "evaluator": "evaluator/exact",
            "target_kind": "independent",
            "candidate_ids": ["cand_child"],
            "target": {"Independent": {"candidate": "cand_child", "target": "Unscoped"}},
            "evidence": {"store": "leaven-run", "key": "val"},
            "metadata": {},
            "purpose": "Validation",
            "created_at": "2026-06-04T00:00:02Z",
        },
    ]
    readback = RustRunReadback.model_validate(data)
    evidence = EvidenceEnvelope.public_only(
        payload=EvidencePublicPayload(summary="ok", output="ok", metrics={}),
        data_classes=["public"],
    )
    rows = [
        Assessment(
            case=Case(id="train-a", input={}),
            candidate_id="cand_child",
            score=Score(value=0.0, feedback="train miss"),
            evidence=evidence,
            receipt=WriteReceipt(receipt_id="assessment_train_a"),
            replayability="boundary_managed",
        ),
        Assessment(
            case=Case(id="train-b", input={}),
            candidate_id="cand_child",
            score=Score(value=0.0, feedback="train miss"),
            evidence=evidence,
            receipt=WriteReceipt(receipt_id="assessment_train_b"),
            replayability="boundary_managed",
        ),
        Assessment(
            case=Case(id="val-1", input={}),
            candidate_id="cand_child",
            score=Score(value=1.0, feedback="val hit"),
            evidence=evidence,
            receipt=WriteReceipt(receipt_id="assessment_val"),
            replayability="boundary_managed",
        ),
    ]

    result = optimized_from_rust_readback(
        readback,
        run_dir="/tmp/run",
        assessment_rows=rows,
    )

    assert result.best.summary_score == 1.0


def test_optimized_from_rust_readback_rejects_unknown_artifact_shape() -> None:
    """Boundary check: Rust run open does not erase unsupported artifacts to object."""

    readback = load_rust_run_readback_fixture()
    data = readback.model_dump(mode="json", by_alias=True)
    data["graph"]["candidates"][0]["artifact"] = {"kind": "unknown"}

    with pytest.raises(TypeError, match="not a PromptArtifact"):
        optimized_from_rust_readback(RustRunReadback.model_validate(data), run_dir="/tmp/run")


def test_runs_open_prefers_rust_checkpoint_without_optimized_json(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    fake = tmp_path / "leaven"
    output = tmp_path / "readback.json"
    evidence = tmp_path / "evidence.json"
    evidence_bytes = rust_case_assessment_evidence_bytes()
    output.write_text(
        json.dumps(load_rust_run_readback_fixture().model_dump(mode="json", by_alias=True)),
        encoding="utf-8",
    )
    evidence.write_text(
        json.dumps(
            {
                "schema_version": "leaven.run_evidence_export.v1",
                "evidence": {"store": "leaven-run", "key": "0"},
                "bytes": len(evidence_bytes),
                "sha256": hashlib.sha256(evidence_bytes).hexdigest(),
                "content_base64": base64.b64encode(evidence_bytes).decode(),
            }
        ),
        encoding="utf-8",
    )
    fake.write_text(
        "#!/bin/sh\n"
        'case "$2" in\n'
        f"  inspect) cat {output} ;;\n"
        f"  evidence) cat {evidence} ;;\n"
        "  *) exit 9 ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)
    monkeypatch.setenv("LEAVEN_BIN", str(fake))

    result = runs.open(run_dir)

    assert result.best.id == "cand_child"
    assert result.summary.run_dir == str(run_dir)
    assert [candidate.id for candidate in result.lineage("cand_child")] == [
        "cand_child",
        "cand_seed",
    ]
    assessment = result.assessment("1")
    assert assessment.case.target == {"answer": "42"}
    assert assessment.case.split == "validation"
    assert assessment.score.value == 0.75
    assert assessment.score.feedback == "exact match"
    assert [
        (reward.id, reward.value, reward.weight, reward.feedback) for reward in assessment.rewards
    ] == [("score", 0.75, 1.0, "exact match")]
    assert [fact.surface for fact in result.summary.unsupported] == ["run.cost"]
    assert result.assessment("1").effect_receipts[0].receipt_id == "lmrec_completion"
    assert result.assessment("1").effect_receipts[0].blob_refs[0].blob_id == "blob_transcript"


def test_runs_inspect_uses_rust_checkpoint_blob_and_evidence_without_optimized_json(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    run_dir = tmp_path / "run"
    (run_dir / "checkpoints").mkdir(parents=True)
    (run_dir / "checkpoints" / "LATEST").write_text("checkpoint_1\n", encoding="utf-8")
    fake = tmp_path / "leaven"
    calls = tmp_path / "calls.txt"
    readback = tmp_path / "readback.json"
    graph_blob = tmp_path / "graph_blob.json"
    artifact_blob = tmp_path / "artifact_blob.json"
    stage_blob = tmp_path / "stage_blob.json"
    workspace_blob = tmp_path / "workspace_blob.json"
    evidence = tmp_path / "evidence.json"
    evidence_bytes = rust_case_assessment_evidence_bytes()
    readback.write_text(
        json.dumps(load_rust_run_readback_fixture().model_dump(mode="json", by_alias=True)),
        encoding="utf-8",
    )
    graph_blob.write_text(
        json.dumps(
            {
                "schema_version": "leaven.run_blob_export.v1",
                "blob": {"store": "file", "key": "graph.blob"},
                "bytes": 19,
                "sha256": "cab11e0c83798e18f101ec99395ac4ebbf38c1739abe06a70ec8264954bf0bd8",
                "content_base64": "ZHVyYWJsZSBibG9iIGJ5dGVzCg==",
            }
        ),
        encoding="utf-8",
    )
    artifact_blob.write_text(
        json.dumps(
            {
                "schema_version": "leaven.run_blob_export.v1",
                "blob": {"store": "file", "key": "artifact.blob"},
                "bytes": 15,
                "sha256": "artifact-sha",
                "content_base64": "YXJ0aWZhY3QgYnl0ZXMK",
            }
        ),
        encoding="utf-8",
    )
    stage_blob.write_text(
        json.dumps(
            {
                "schema_version": "leaven.run_blob_export.v1",
                "blob": {"store": "file", "key": "stage.blob"},
                "bytes": 12,
                "sha256": "stage-sha",
                "content_base64": "c3RhZ2UgYnl0ZXMK",
            }
        ),
        encoding="utf-8",
    )
    workspace_blob.write_text(
        json.dumps(
            {
                "schema_version": "leaven.run_blob_export.v1",
                "blob": {"store": "file", "key": "workspace.blob"},
                "bytes": 16,
                "sha256": "workspace-sha",
                "content_base64": "d29ya3NwYWNlIGJ5dGVzCg==",
            }
        ),
        encoding="utf-8",
    )
    evidence.write_text(
        json.dumps(
            {
                "schema_version": "leaven.run_evidence_export.v1",
                "evidence": {"store": "leaven-run", "key": "0"},
                "bytes": len(evidence_bytes),
                "sha256": hashlib.sha256(evidence_bytes).hexdigest(),
                "content_base64": base64.b64encode(evidence_bytes).decode(),
            }
        ),
        encoding="utf-8",
    )
    fake.write_text(
        "#!/bin/sh\n"
        'printf \'%s\\n\' "$@" >> "$LEAVEN_TEST_CALLS"\n'
        'case "$2" in\n'
        f"  inspect) cat {readback} ;;\n"
        f'  blob) case "$8" in graph.blob) cat {graph_blob} ;; artifact.blob) cat {artifact_blob} ;; stage.blob) cat {stage_blob} ;; workspace.blob) cat {workspace_blob} ;; *) exit 8 ;; esac ;;\n'
        f"  evidence) cat {evidence} ;;\n"
        "  *) exit 9 ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)

    monkeypatch.setenv("LEAVEN_BIN", str(fake))
    monkeypatch.setenv("LEAVEN_TEST_CALLS", str(calls))

    inspection = runs.inspect(run_dir)

    assert inspection.run_id == "run_1"
    assert inspection.best_candidate_id == "cand_child"
    assert inspection.rust_readback is not None
    assert inspection.rust_graph_blob is not None
    assert inspection.rust_graph_blob.content_bytes() == b"durable blob bytes\n"
    assert [blob.content_bytes() for blob in inspection.rust_artifact_blobs] == [
        b"artifact bytes\n"
    ]
    assert [blob.content_bytes() for blob in inspection.rust_stage_journal_blobs] == [
        b"stage bytes\n"
    ]
    assert [blob.content_bytes() for blob in inspection.rust_workspace_journal_blobs] == [
        b"workspace bytes\n"
    ]
    assert len(inspection.rust_evidence) == 1
    assert inspection.evidence[0].case_id == "1"
    assert inspection.evidence[0].candidate_id == "cand_child"
    assert inspection.evidence[0].payload == EvidencePublicPayload(
        summary="42",
        output="42",
        metrics={"reward_count": 1.0},
    )
    assert inspection.evidence[0].target_derived is True
    assert inspection.evidence[0].data_classes == [
        "candidate.output",
        "case.input",
        "case.target",
        "public",
    ]
    assert [
        (reward.id, reward.value, reward.weight, reward.feedback)
        for reward in inspection.evidence[0].rewards
    ] == [("score", 0.75, 1.0, "exact match")]
    call_receipts = [receipt for receipt in inspection.receipts if receipt.kind == "call"]
    assert [receipt.receipt_id for receipt in call_receipts] == ["lmrec_completion"]
    assert call_receipts[0].blob_refs[0].blob_id == "blob_transcript"
    assert [fact.surface for fact in inspection.unsupported] == ["run.cost"]
    assert calls.read_text(encoding="utf-8").splitlines() == [
        "run",
        "inspect",
        "--run-dir",
        str(run_dir),
        "run",
        "blob",
        "--run-dir",
        str(run_dir),
        "--store",
        "file",
        "--key",
        "graph.blob",
        "run",
        "blob",
        "--run-dir",
        str(run_dir),
        "--store",
        "file",
        "--key",
        "artifact.blob",
        "run",
        "blob",
        "--run-dir",
        str(run_dir),
        "--store",
        "file",
        "--key",
        "stage.blob",
        "run",
        "blob",
        "--run-dir",
        str(run_dir),
        "--store",
        "file",
        "--key",
        "workspace.blob",
        "run",
        "evidence",
        "--run-dir",
        str(run_dir),
        "--store",
        "leaven-run",
        "--key",
        "0",
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
                "artifact_refs": [{"store": "file", "key": "artifact.blob"}],
                "artifact_ref_count": 1,
                "evidence_refs": [],
                "evidence_ref_count": 0,
                "stage_journal_refs": [{"store": "file", "key": "stage.blob"}],
                "stage_journal_ref_count": 1,
                "workspace_journal_refs": [{"store": "file", "key": "workspace.blob"}],
                "workspace_journal_ref_count": 1,
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
                "proposal_batches": [
                    {
                        "id": "pb_child",
                        "proposal_ids": ["prop_child"],
                    }
                ],
                "assessments": [
                    {
                        "id": "assessment_child",
                        "request_id": "eval_req_child",
                        "evaluator": "evaluator/exact",
                        "target_kind": "independent",
                        "candidate_ids": ["cand_child"],
                        "target": {
                            "Independent": {
                                "candidate": "cand_child",
                                "target": "Unscoped",
                            }
                        },
                        "evidence": {
                            "store": "leaven-run",
                            "key": "0",
                        },
                        "metadata": {"split": "validation"},
                        "purpose": "Validation",
                        "created_at": "2026-06-04T00:00:02Z",
                    }
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
