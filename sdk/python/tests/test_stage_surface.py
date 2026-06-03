from __future__ import annotations

import json
import subprocess
import sys

import leaven as lv
from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam import CommandRunnerStageConfig, MockRunnerStageConfig, StageRunRequest
from leaven.builders.agent import AgentBuilder
from leaven.builders.lm import LmBuilder


def test_optimize_surface_names_the_four_product_inputs() -> None:
    """Example test: optimize composes seed, environment, optimizer, runtime."""

    task = lv.Task(
        name="ctf-smoke",
        cases=[
            lv.Case(
                id="ctf-001",
                input={"instructions": "Find the flag."},
                target={"flag": "picoCTF{...}"},
                files={"README.md": "Solve the challenge."},
                setup=lv.setup.bash("chmod +x case/files/challenge"),
            ),
        ],
        sandbox=lv.sandbox.docker(image="python:3.12"),
    )
    seed = lv.artifacts.directory("./agent_harness")
    layout = lv.layouts.case_workspace()
    rollout = lv.Rollout.command(
        argv=["uv", "run", "python", "target/current/run.py"],
        layout=layout,
        output=lv.output.files(["output/result.json"], max_bytes=64_000),
    )
    rubric = lv.Rubric([dummy_reward])
    environment = lv.Environment(
        task=task,
        rollout=rollout,
        rubric=rubric,
    )
    runtime = lv.runtime.local(budget=lv.budget(usd=20))

    run = lv.optimize(
        seed=seed,
        environment=environment,
        optimizer=lv.optimizers.gepa(population_size=4),
        runtime=runtime,
    )

    assert run.seed == seed
    assert run.environment == environment
    assert run.optimizer.name == "gepa"
    assert run.runtime == runtime
    assert environment.task == task
    assert environment.rubric == rubric
    assert rollout.layout == layout


@lv.reward
async def dummy_reward(output: object, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    """The reward can inspect scorer-role case data."""
    _ = (output, case, cx)
    return 1.0


async def test_agent_builder_run_uses_bound_public_seam_client() -> None:
    """AgentBuilder.run lowers to the durable public-seam agent.run route."""

    client = FakeSeamClient()
    agent = AgentBuilder._for_seam(
        client,
        candidate_id="cand_agent_builder",
        idempotency_prefix="agent-builder-test",
        plan_id="planagentbuilder001",
    )

    session = await agent.run(
        workspace=WorkspaceHandle(
            workspace_id="ws_agent_builder_materialized",
            candidate_id="cand_agent_builder",
            lifetime="manual",
            receipt=CallReceipt(receipt_id="wrec_agent_builder"),
        ),
        instructions=lv.AgentInstructions(task="Return a short answer.", system="Stay brief."),
        runtime="codex-cli",
        output=lv.output.text(max_chars=256),
        allowed_commands=["codex"],
    )

    assert client.request_value["method"] == "leaven/agent.run"
    params = client.request_value["params"]
    assert params["plan_id"] == "planagentbuilder001"
    assert params["ops"][0]["call"]["kind"] == "workspace_materialize"
    assert params["ops"][1]["call"]["kind"] == "agent_run"
    assert params["ops"][1]["call"]["workspace"] == "ws_agent_builder_materialized"
    assert params["ops"][1]["call"]["instructions"]["task"] == "Return a short answer."
    assert params["ops"][1]["call"]["tool_policy"]["allowed_commands"] == ["codex"]
    assert params["ops"][1]["call"]["output"] == {"kind": "final_message", "max_bytes": 256}
    assert session.transcript_ref == "blob_agent_builder_transcript"
    assert session.receipt.receipt_id == "agentrec_completion"
    assert session.cost_usd == 0.25


async def test_lm_builder_complete_uses_bound_public_seam_client() -> None:
    """LmBuilder.complete lowers to the durable public-seam lm.complete route."""

    client = FakeLmSeamClient()
    lm = LmBuilder._for_seam(
        client,
        idempotency_prefix="lm-builder-test",
        plan_id="planlmbuilder001",
        model="gpt-4.1-mini",
    )

    response = await lm.complete(
        prompt="Say ok.",
        model_role="reflector",
        temperature=0.2,
        max_tokens=12,
        stop=["DONE"],
        input_classes=["public"],
    )

    assert client.request_value["method"] == "leaven/lm.complete"
    params = client.request_value["params"]
    assert params["plan_id"] == "planlmbuilder001"
    assert params["return"] == ["completion"]
    op = params["ops"][0]
    assert op["call"]["kind"] == "lm_complete"
    assert op["call"]["purpose"] == "python.sdk"
    assert op["call"]["model"] == "gpt-4.1-mini"
    assert op["call"]["model_role"] == "reflector"
    assert op["call"]["messages"][0]["content"][0]["text"] == "Say ok."
    assert op["call"]["sampling"] == {
        "temperature": 0.2,
        "max_output_tokens": 12,
        "stop": ["DONE"],
    }
    assert op["call"]["input_classes"] == ["public"]
    assert response.text == "ok"
    assert response.usage == {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}
    assert response.cost_usd == 0.000042
    assert response.model == "gpt-4.1-mini"
    assert response.receipt.receipt_id == "lmrec_completion"


def test_stage_run_request_names_locked_runner_dispatch_shape() -> None:
    """Private stage-run request construction stays on the locked durable seam."""

    request = StageRunRequest(
        request_id="stage-builder-test",
        run_id="run_stage_builder",
        stage_call_id="sc_stage_builder",
        candidate="cand_stage_builder",
        case="case_stage_builder",
        case_input={"question": "2 + 2"},
    ).to_json_rpc()

    assert request["method"] == "leaven/stage.run"
    params = request["params"]
    assert params["schema_version"] == "leaven.stage_run.v1"
    assert params["message"] == "stage_run_request"
    assert params["stage"] == "runner"
    assert params["payload"] == {
        "schema_version": "leaven.stage_payloads.v1",
        "role": "runner",
        "run": "run_stage_builder",
        "stage_call_id": "sc_stage_builder",
        "candidate": "cand_stage_builder",
        "case": "case_stage_builder",
        "case_input": {"question": "2 + 2"},
        "target_forbidden": True,
    }
    assert MockRunnerStageConfig(text="ok", summary="runner").to_json() == {
        "kind": "mock_runner",
        "text": "ok",
        "summary": "runner",
    }
    assert CommandRunnerStageConfig(argv=("python", "-m", "worker")).to_json() == {
        "kind": "command_runner",
        "argv": ["python", "-m", "worker"],
    }


def test_checked_in_stage_worker_dispatches_registered_runner(tmp_path) -> None:
    """Scenario: command worker imports a registered runner and returns stage result."""

    module = tmp_path / "worker_stage.py"
    module.write_text(
        """
import leaven as lv

@lv.runner
async def run(prompt, case, cx):
    reply = await cx.lm.complete(prompt=prompt.template)
    return f"{case.input['question']} => {reply.text}"
""".lstrip(),
        encoding="utf-8",
    )
    request = StageRunRequest(
        request_id="stage-worker-test",
        run_id="run_stage_worker",
        stage_call_id="sc_stage_worker",
        candidate="cand_stage_worker",
        case="case_stage_worker",
        case_input={"question": "2 + 2", "prompt": "Answer the question."},
    ).to_json_rpc()

    process = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "leaven._seam_worker",
            "--module-file",
            str(module),
            "--stage-id",
            "worker_stage.run",
            "--stage-name",
            "run",
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
    assert callback["method"] == "leaven/lm.complete"
    assert callback["params"]["ops"][0]["call"]["messages"][0]["content"][0]["text"] == (
        "Answer the question."
    )
    process.stdin.write(
        json.dumps(
            {
                "jsonrpc": "2.0",
                "id": callback["id"],
                "result": {
                    "method": "leaven/lm.complete",
                    "primary": {
                        "kind": "lm_response",
                        "message": {
                            "role": "assistant",
                            "content": [{"kind": "text", "text": "4"}],
                        },
                        "receipt": "lmrec_worker_test",
                    },
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
    assert response["result"]["stage_call_id"] == "sc_stage_worker"
    assert response["result"]["output"]["value"] == "2 + 2 => 4"


def test_checked_in_stage_worker_can_callback_agent_run(tmp_path) -> None:
    """Scenario: registered runner can call `cx.agent.run` over the active seam."""

    module = tmp_path / "agent_stage.py"
    module.write_text(
        """
import leaven as lv

@lv.runner
async def run(prompt, case, cx):
    session = await cx.agent.run(
        workspace=cx.rollout_workspace,
        instructions=lv.AgentInstructions(task=f"answer {case.input['question']}"),
        output=lv.output.text(max_chars=128),
    )
    return session.receipt.receipt_id
""".lstrip(),
        encoding="utf-8",
    )
    request = StageRunRequest(
        request_id="stage-worker-agent-test",
        run_id="run_stage_worker_agent",
        stage_call_id="sc_stage_worker_agent",
        candidate="cand_stage_worker_agent",
        case="case_stage_worker_agent",
        case_input={"question": "2 + 2", "prompt": "Answer the question."},
    ).to_json_rpc()

    process = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "leaven._seam_worker",
            "--module-file",
            str(module),
            "--stage-id",
            "agent_stage.run",
            "--stage-name",
            "run",
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
    assert callback["method"] == "leaven/agent.run"
    params = callback["params"]
    assert params["ops"][0]["call"]["candidate"] == "cand_stage_worker_agent"
    assert params["ops"][1]["call"]["workspace"] == "ws_stage_worker_agent_materialized"
    assert params["ops"][1]["call"]["instructions"]["task"] == "answer 2 + 2"
    process.stdin.write(
        json.dumps(
            {
                "jsonrpc": "2.0",
                "id": callback["id"],
                "result": {
                    "method": "leaven/agent.run",
                    "primary": {
                        "kind": "agent_session",
                        "status": "completed",
                        "receipt": "agentrec_worker_test",
                        "transcript_ref": {
                            "kind": "blob_ref",
                            "id": "blob_worker_agent_transcript",
                            "sha256": "a" * 64,
                            "bytes": 8,
                            "data_classes": ["transcript.raw"],
                        },
                        "commands": [],
                        "cost": {"usd_micro": 0},
                    },
                    "receipts": [{"receipt": "agentrec_worker_test", "call_kind": "agent_run"}],
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
    assert response["result"]["stage_call_id"] == "sc_stage_worker_agent"
    assert response["result"]["output"]["value"] == "agentrec_worker_test"


class FakeSeamClient:
    def __init__(self) -> None:
        self.request_value: dict = {}

    def request(self, request: dict) -> dict:
        self.request_value = request
        return {
            "method": "leaven/agent.run",
            "primary": {
                "kind": "agent_session",
                "status": "completed",
                "receipt": "agentrec_completion",
                "transcript_ref": {
                    "kind": "blob_ref",
                    "id": "blob_agent_builder_transcript",
                    "sha256": "a" * 64,
                    "bytes": 128,
                    "data_classes": ["transcript.raw"],
                },
                "commands": [
                    {
                        "argv": ["codex", "exec"],
                        "status": "completed",
                        "receipt": "agentrec_completion",
                    }
                ],
                "cost": {"usd_micro": 250_000},
            },
            "receipts": [{"receipt": "agentrec_completion", "call_kind": "agent_run"}],
        }


class FakeLmSeamClient:
    def __init__(self) -> None:
        self.request_value: dict = {}

    def request(self, request: dict) -> dict:
        self.request_value = request
        return {
            "method": "leaven/lm.complete",
            "primary": {
                "kind": "lm_response",
                "message": {
                    "role": "assistant",
                    "content": [{"kind": "text", "text": "ok"}],
                },
                "receipt": "lmrec_completion",
                "cost": {"usd_micro": 42, "input_tokens": 3, "output_tokens": 2},
            },
            "receipts": [{"receipt": "lmrec_completion", "call_kind": "lm_complete"}],
        }
