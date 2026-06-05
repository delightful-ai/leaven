import json

import msgspec

import leaven as lv
from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam import SandboxExecRequest
from leaven._seam._wire.calls import OutputFiles
from leaven._seam._wire.payloads import Cost
from leaven._seam._wire.refs import BlobRef as WireBlobRef
from leaven._seam._wire.results import SandboxExecPrimary, SandboxExecResult
from leaven.builders.sandbox import SandboxBuilder
from leaven.json_value import JsonObject, JsonValue


async def test_sandbox_builder_exec_lowers_files_output_contract_through_seam() -> None:
    """Scenario: sandbox exec routes typed command and file capture through seam."""

    client = FakeSandboxSeamClient()
    sandbox = SandboxBuilder._for_seam(
        client,
        idempotency_prefix="sandbox-builder-test",
        plan_id="plansandboxbuilder001",
    )
    result = await sandbox.exec(
        workspace=WorkspaceHandle(
            workspace_id="ws_sandbox_builder",
            candidate_id="cand_sandbox",
            lifetime="manual",
            receipt=CallReceipt(receipt_id="wrec_workspace"),
        ),
        argv=["python", "-c", "print('ok')"],
        env={"LEAVEN_CASE": "case_1"},
        cwd="work",
        timeout_s=2.1,
        output=lv.output.files(["reports/out.txt"], max_bytes=1024),
        stream_policy="blob_refs_only",
        input_classes=[lv.data_class.WORKSPACE_FILE],
        forbidden_input_classes=[lv.data_class.WORKSPACE_SECRET],
    )

    assert client.request_value.method == "leaven/sandbox.exec"
    assert isinstance(client.request_value.output, OutputFiles)
    assert client.request_value.output.paths == ["reports/out.txt"]
    assert client.request_value.output.max_bytes == 1024
    params = _params_object(client.request_value.to_params())
    assert params["plan_id"] == "plansandboxbuilder001"
    assert params["return"] == ["sandbox_exec"]
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    assert op["kind"] == "call"
    assert op["idempotency_key"] == "sandbox-builder-test-exec"
    call = _json_object(op["call"])
    assert call == {
        "kind": "sandbox_exec",
        "workspace": "ws_sandbox_builder",
        "argv": ["python", "-c", "print('ok')"],
        "timeout_s": 3,
        "output": {"kind": "files", "paths": ["reports/out.txt"], "max_bytes": 1024},
        "input_classes": [lv.data_class.WORKSPACE_FILE],
        "cwd": "work",
        "env": {"LEAVEN_CASE": "case_1"},
        "stream_policy": "blob_refs_only",
        "forbidden_input_classes": [lv.data_class.WORKSPACE_SECRET],
    }
    assert result.exit_code == 0
    assert result.stdout_ref.blob_id == "blob_stdout"
    assert result.stderr_ref.blob_id == "blob_stderr"
    assert result.files is not None
    assert result.files["reports/out.txt"].sha256 == "c" * 64
    assert result.cost_usd == 0.00001
    assert result.receipt.receipt_id == "execrec_sandbox"
    assert [blob.blob_id for blob in result.receipt.blob_refs] == [
        "blob_stdout",
        "blob_stderr",
        "blob_file",
    ]


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    if not isinstance(value, list):
        raise TypeError("expected JSON array")
    return value


def _json_object(value: JsonValue) -> JsonObject:
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value


class FakeSandboxSeamClient:
    def __init__(self) -> None:
        self.request_value = SandboxExecRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            workspace="unset",
            argv=["true"],
            timeout_s=1,
            output=OutputFiles(paths=[]),
        )

    def sandbox_exec(self, request: SandboxExecRequest) -> SandboxExecResult:
        self.request_value = request
        return SandboxExecResult(
            method="leaven/sandbox.exec",
            primary=SandboxExecPrimary(
                kind="sandbox_exec",
                status="completed",
                cost=Cost(usd_micro=10, sandbox_calls=1),
                graph_revision="rev_sandbox_builder",
                data_classes=[lv.data_class.WORKSPACE_FILE],
                replayability="fully_managed",
                receipt="execrec_sandbox",
                exit_code=0,
                stdout_ref=WireBlobRef(
                    id="blob_stdout",
                    sha256="a" * 64,
                    bytes=3,
                    data_classes=[lv.data_class.WORKSPACE_FILE],
                ),
                stderr_ref=WireBlobRef(
                    id="blob_stderr",
                    sha256="b" * 64,
                    bytes=0,
                    data_classes=[lv.data_class.WORKSPACE_FILE],
                ),
                files={
                    "reports/out.txt": WireBlobRef(
                        id="blob_file",
                        sha256="c" * 64,
                        bytes=12,
                        data_classes=[lv.data_class.WORKSPACE_FILE],
                    )
                },
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=[lv.data_class.WORKSPACE_FILE],
        )


__all__ = []
