from __future__ import annotations

import json
import subprocess
import sys


def test_checked_in_stage_worker_can_callback_proposal_submit(tmp_path) -> None:
    """Scenario: registered proposer can submit a ProposalBatch over the active seam."""

    module = tmp_path / "proposer_stage.py"
    module.write_text(
        """
import leaven as lv
from leaven._receipts import CallReceipt, QueryReceipt
from leaven.proposal import ProposalBatch, ProposalEffect

@lv.proposer(stage_id="proposer_stage.propose")
async def propose(req, cx):
    return ProposalBatch(
        effects=[
            ProposalEffect.change_from_agent_session(
                parent_candidate_id=req.parent_candidate_id,
                surface=req.allowed_surfaces[0],
                change_schema=req.allowed_change_schemas[0],
                parser="leaven.agent_session.skill_patch.v1",
                agent_session_receipt=CallReceipt(receipt_id="agentrec_codex"),
            )
        ],
        read_receipts=[QueryReceipt(receipt_id="qrec_reflection")],
    )
""".lstrip(),
        encoding="utf-8",
    )
    request = _proposer_stage_run_request()

    process = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "leaven._seam_worker",
            "--module-file",
            str(module),
            "--stage-id",
            "proposer_stage.propose",
            "--stage-name",
            "propose",
            "--lm-model",
            "mock",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()

    callback = json.loads(process.stdout.readline())
    assert callback["method"] == "leaven/proposal.submit_batch"
    params = callback["params"]
    proposal = params["ops"][0]["write"]["proposals"][0]
    assert proposal["effect"] == {
        "kind": "change_from_agent_session",
        "target": "cand_proposer_worker_parent",
        "surface_fingerprint": "fp_surface_sha256_proposer_worker",
        "change_schema": "fp_schema_sha256_proposer_worker_patch",
        "agent_receipt": "agentrec_codex",
        "parser": "leaven.agent_session.skill_patch.v1",
    }
    assert proposal["read_receipts"] == ["qrec_reflection", "agentrec_codex"]

    process.stdin.write(
        json.dumps(
            {
                "jsonrpc": "2.0",
                "id": callback["id"],
                "result": {
                    "method": "leaven/proposal.submit_batch",
                    "primary": {
                        "kind": "proposal_batch_receipt",
                        "batch_id": "pb_worker",
                        "proposal_ids": ["prop_worker"],
                        "status": "committed",
                        "receipt": "wrec_worker_proposal",
                    },
                    "receipts": [
                        {
                            "receipt": "wrec_worker_proposal",
                            "write_kind": "submit_proposal_batch",
                        }
                    ],
                },
            }
        )
        + "\n"
    )
    process.stdin.flush()
    response = json.loads(process.stdout.readline())
    stdout, stderr = process.communicate(timeout=5)

    assert stdout == ""
    assert process.returncode == 0, stderr
    assert response["result"]["stage"] == "proposer"
    assert response["result"]["stage_call_id"] == "sc_proposer_worker"
    assert response["result"]["output"]["value"] == "wrec_worker_proposal"
    assert response["result"]["effect_receipts"] == []
    assert response["result"]["proposal_receipts"] == [
        {
            "method": "leaven/proposal.submit_batch",
            "receipt": "wrec_worker_proposal",
            "write_kind": "submit_proposal_batch",
            "proposal_ids": ["prop_worker"],
        }
    ]


def _proposer_stage_run_request() -> dict:
    return {
        "jsonrpc": "2.0",
        "id": "proposer-worker-test",
        "method": "leaven/stage.run",
        "params": {
            "schema_version": "leaven.stage_run.v1",
            "message": "stage_run_request",
            "stage": "proposer",
            "payload": {
                "schema_version": "leaven.stage_payloads.v1",
                "role": "proposer",
                "run": "run_proposer_worker",
                "stage_call_id": "sc_proposer_worker",
                "base_revision": "rev_proposer_worker",
                "parent": "cand_proposer_worker_parent",
                "surface_fingerprint": "fp_surface_sha256_proposer_worker",
                "reflection_result": {
                    "schema_version": "leaven.stage_payloads.v1",
                    "role": "reflection_result",
                    "summary": "empty inputs fail",
                    "failure_modes": [
                        {
                            "label": "missing_empty_input_guard",
                            "description": "empty inputs fail",
                            "source_refs": ["cand_proposer_worker_parent"],
                        }
                    ],
                    "surface_suggestions": [],
                    "negative_constraints": [],
                    "positive_constraints": [],
                    "source_refs": ["cand_proposer_worker_parent"],
                    "read_receipts": ["qrec_reflection"],
                    "data_classes": ["optimizer.visible"],
                    "confidence": 0.8,
                },
                "allowed_effects": ["change_from_agent_session"],
                "allowed_change_schemas": ["fp_schema_sha256_proposer_worker_patch"],
                "source_refs": ["cand_proposer_worker_parent"],
                "query_policy_fingerprint": "fp_policy_sha256_proposer_worker",
                "capability_fingerprint": "fp_cap_sha256_proposer_worker",
            },
        },
    }
