import json
import subprocess
import sys
from pathlib import Path

from _pytest.monkeypatch import MonkeyPatch

import leaven as lv
from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam import (
    AgentRunRequest,
    CommandRunnerStageConfig,
    LmCompleteRequest,
    MockRunnerStageConfig,
    OpenAiLmRuntimeConfig,
    StageRunRequest,
)
from leaven._seam._wire.codec import encode_request
from leaven._seam._wire.payloads import BlobRef as WireBlobRef
from leaven._seam._wire.payloads import Cost
from leaven._seam._wire.results import (
    AgentCommandRecord,
    AgentRunResult,
    AgentSessionPrimary,
    LmCompleteResult,
    LmContentPart,
    LmMessageRecord,
    LmResponsePrimary,
)
from leaven._seam_optimize.driver import _agent_config, _lm_config, _lm_model
from leaven._seam_optimize.rewards import evaluate_reward_vector
from leaven._seam_optimize.status import unsupported_facts_for_runtime
from leaven._seam_optimize.types import SeamOptimizeReport, SeamStageAssessment
from leaven.artifacts.prompt import PromptArtifact
from leaven.assessment import RewardAssessment
from leaven.builders.agent import AgentBuilder
from leaven.builders.lm import LmBuilder
from leaven.json_value import JsonObject, JsonValue
from leaven.optimize import _to_optimized
from leaven.run_status import UnsupportedRunFact, project_cost_usage


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

    assert client.request_value.method == "leaven/agent.run"
    params = client.request_value.to_params()
    assert params["plan_id"] == "planagentbuilder001"
    ops = _json_array(params["ops"])
    workspace_op = _json_object(ops[0])
    agent_op = _json_object(ops[1])
    workspace_call = _json_object(workspace_op["call"])
    agent_call = _json_object(agent_op["call"])
    instructions = _json_object(agent_call["instructions"])
    tool_policy = _json_object(agent_call["tool_policy"])
    assert workspace_call["kind"] == "workspace_materialize"
    assert agent_call["kind"] == "agent_run"
    assert agent_call["workspace"] == "ws_agent_builder_materialized"
    assert instructions["task"] == "Return a short answer."
    assert tool_policy["allowed_commands"] == ["codex"]
    assert agent_call["output"] == {"kind": "final_message", "max_bytes": 256}
    assert session.transcript_ref == "blob_agent_builder_transcript"
    assert session.transcript is not None
    assert session.transcript.blob_id == "blob_agent_builder_transcript"
    assert session.transcript.bytes == 128
    assert session.receipt.receipt_id == "agentrec_completion"
    assert session.receipt.blob_refs == [session.transcript]
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

    assert client.request_value.method == "leaven/lm.complete"
    params = client.request_value.to_params()
    assert params["plan_id"] == "planlmbuilder001"
    assert params["return"] == ["completion"]
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    call = _json_object(op["call"])
    messages = _json_array(call["messages"])
    message = _json_object(messages[0])
    content = _json_array(message["content"])
    content_part = _json_object(content[0])
    assert call["kind"] == "lm_complete"
    assert call["purpose"] == "python.sdk"
    assert call["model"] == "gpt-4.1-mini"
    assert call["model_role"] == "reflector"
    assert content_part["text"] == "Say ok."
    assert call["sampling"] == {
        "temperature": 0.2,
        "max_output_tokens": 12,
        "stop": ["DONE"],
    }
    assert call["input_classes"] == ["public"]
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
    )

    assert request.method == "leaven/stage.run"
    params = request.to_params()
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


def test_openai_runtime_lowers_to_private_seam_service_config() -> None:
    """Scenario: runtime OpenAI config reaches the Rust service as provider config."""

    runtime = lv.runtime(
        workspace=lv.workspace.local(),
        lm=lv.lm.openai(
            model="gpt-4.1-mini",
            api_key_env="LEAVEN_TEST_OPENAI_KEY",
            base_url="http://127.0.0.1:12345/v1/responses",
            timeout_s=7,
            max_retries=0,
        ),
    )

    lm_config = _lm_config(runtime, fallback_text="unused")

    assert _lm_model(runtime) == "gpt-4.1-mini"
    assert isinstance(lm_config, OpenAiLmRuntimeConfig)
    assert lm_config.to_json() == {
        "kind": "open_ai",
        "api_key_env": "LEAVEN_TEST_OPENAI_KEY",
        "base_url": "http://127.0.0.1:12345/v1/responses",
        "timeout_s": 7,
        "max_retries": 0,
    }


def test_checked_in_stage_worker_dispatches_registered_runner(tmp_path: Path) -> None:
    """Scenario: command worker imports a registered runner and returns stage result."""

    module = tmp_path / "worker_stage.py"
    module.write_text(
        """
import leaven as lv

@lv.runner
async def run(prompt, case, cx):
    reply = await cx.lm.complete(prompt=prompt.template, max_tokens=12)
    return f"{case.input['question']} => {reply.text} / {reply.usage['total_tokens']}"
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
    )

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
            "--lm-model",
            "gpt-test",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    process.stdin.write(_json_rpc_line(request) + "\n")
    process.stdin.flush()

    callback = json.loads(process.stdout.readline())
    assert callback["method"] == "leaven/lm.complete"
    assert callback["params"]["ops"][0]["call"]["model"] == "gpt-test"
    assert callback["params"]["ops"][0]["call"]["sampling"]["max_output_tokens"] == 12
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
                        "cost": {"input_tokens": 3, "output_tokens": 2, "lm_calls": 1},
                        "receipt": "lmrec_worker_test",
                    },
                    "receipts": [],
                    "redactions": [],
                    "capability_fingerprint": "fp_cap_worker_test",
                    "policy_fingerprint": "fp_policy_worker_test",
                    "data_classes": ["public"],
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
    assert response["result"]["output"]["value"] == "2 + 2 => 4 / 5"


def test_checked_in_stage_worker_can_callback_agent_run(tmp_path: Path) -> None:
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
    )

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
    process.stdin.write(_json_rpc_line(request) + "\n")
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
                    "receipts": [
                        {
                            "kind": "call",
                            "receipt": "agentrec_worker_test",
                            "status": "succeeded",
                            "result_hash": "fp_result_worker_agent",
                            "call_kind": "agent_run",
                        }
                    ],
                    "redactions": [],
                    "capability_fingerprint": "fp_cap_worker_test",
                    "policy_fingerprint": "fp_policy_worker_test",
                    "data_classes": ["public"],
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
    assert response["result"]["effect_receipts"] == [
        {
            "method": "leaven/agent.run",
            "receipt": "agentrec_worker_test",
            "call_kind": "agent_run",
            "cost": {"usd_micro": 0},
            "blob_refs": [
                {
                    "kind": "blob_ref",
                    "id": "blob_worker_agent_transcript",
                    "sha256": "a" * 64,
                    "bytes": 8,
                    "data_classes": ["transcript.raw"],
                }
            ],
        }
    ]


def test_optimize_runtime_codex_agent_config_lowers_to_seam(monkeypatch: MonkeyPatch) -> None:
    """Example: Python runtime agent config becomes service Codex CLI config."""

    monkeypatch.setenv("LEAVEN_TEST_CODEX_BIN", "/tmp/leaven-test-codex")
    runtime = lv.runtime(
        workspace=lv.workspace.local(),
        lm=lv.lm.mock(responses=["unused"]),
        agent=lv.agent.codex(
            model="gpt-5.4-mini",
            transport="cli",
            approval_mode="interactive",
            bin_path_env="LEAVEN_TEST_CODEX_BIN",
            timeout_s=17,
        ),
    )

    config = _agent_config(runtime)

    assert config is not None
    assert config.to_json() == {
        "kind": "codex_cli",
        "codex_bin": "/tmp/leaven-test-codex",
        "model": "gpt-5.4-mini",
        "timeout_s": 17,
        "codex_home": None,
        "bypass_approvals_and_sandbox": False,
    }


def test_run_status_projection_hides_unsupported_dependency_totals() -> None:
    """Example: unsupported provider facts prevent fabricated zero-cost summaries."""

    unsupported = (
        UnsupportedRunFact(
            surface="run.cost",
            dependency="codex_cli",
            reason="provider_cost_not_reported",
            detail="Codex CLI did not report cost.",
        ),
        UnsupportedRunFact(
            surface="run.usage",
            dependency="codex_cli",
            reason="provider_usage_not_reported",
            detail="Codex CLI did not report tokens.",
        ),
    )

    projection = project_cost_usage(
        default_cost_usd=0.0,
        default_lm_tokens=0,
        unsupported=unsupported,
    )

    assert projection.cost_status == "unsupported_dependency"
    assert projection.usage_status == "unsupported_dependency"
    assert projection.total_cost_usd is None
    assert projection.total_lm_tokens is None


def test_optimize_summary_names_codex_cost_and_inspection_gaps() -> None:
    """Scenario: seam optimize summary names unsupported deps instead of zeroing them."""

    runtime = lv.runtime(
        workspace=lv.workspace.local(),
        lm=lv.lm.mock(responses=["unused"]),
        agent=lv.agent.codex(model="gpt-5.4-mini", transport="cli"),
    )
    unsupported = unsupported_facts_for_runtime(runtime)
    result = _to_optimized(
        PromptArtifact(template="answer {question}"),
        SeamOptimizeReport(seed_score=1.0, best_score=1.0, assessments=[], unsupported=unsupported),
        "codex-status-test",
        "2026-06-03T00:00:00+00:00",
        1,
    )

    assert result.summary.total_cost_usd is None
    assert result.summary.total_lm_tokens is None
    assert result.summary.cost_status == "unsupported_dependency"
    assert result.summary.usage_status == "unsupported_dependency"
    assert {
        (fact.surface, fact.dependency, fact.reason) for fact in result.summary.unsupported
    } == {
        ("run.cost", "codex_cli", "provider_cost_not_reported"),
        ("run.usage", "codex_cli", "provider_usage_not_reported"),
        ("run.inspection", "python_seam_optimize", "blob_readback_not_implemented"),
    }
    assert "blob ref metadata" in result.summary.unsupported[0].detail


async def test_reward_vector_executes_all_registered_rewards() -> None:
    """Example: Python rubric rewards produce per-axis rows and aggregate score."""

    @lv.reward(weight=2.0, id="correct")
    async def correct(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
        _ = cx
        return 1.0 if output == (case.target or {})["answer"] else 0.0

    @lv.reward(weight=1.0, id="concise")
    async def concise(
        output: str, case: lv.ScoringCaseView, cx: lv.RubricContext
    ) -> lv.RewardValue:
        _ = (case, cx)
        return lv.RewardValue(value=0.5, feedback=f"{len(output)} chars")

    score, rewards = await evaluate_reward_vector(
        rubric=lv.Rubric([correct, concise]),
        output="42",
        case={
            "case_id": "case_reward_vector",
            "input": {"question": "6 * 7?"},
            "target": {"answer": "42"},
            "metadata": {"source": "unit"},
        },
    )

    assert score.value == 0.8333333333333334
    assert score.feedback == "concise: 2 chars"
    assert [(reward.id, reward.value, reward.weight) for reward in rewards] == [
        ("correct", 1.0, 2.0),
        ("concise", 0.5, 1.0),
    ]


def test_optimized_result_exposes_assessments_rewards_and_lineage() -> None:
    """Scenario: completed result inspection is not stdout-only."""

    result = _to_optimized(
        PromptArtifact(template="answer {question}"),
        SeamOptimizeReport(
            seed_score=0.75,
            best_score=0.75,
            assessments=[
                SeamStageAssessment(
                    case_id="case_inspect_001",
                    case_input={"question": "6 * 7?"},
                    case_target={"answer": "42"},
                    case_metadata={"source": "unit"},
                    case_split="test",
                    output="42",
                    score=lv.Score(value=0.75, feedback="weighted"),
                    rewards=[
                        RewardAssessment(id="correct", value=1.0, weight=2.0),
                        RewardAssessment(
                            id="concise",
                            value=0.25,
                            weight=1.0,
                            feedback="short enough",
                        ),
                    ],
                )
            ],
        ),
        "inspection-test",
        "2026-06-03T00:00:00+00:00",
        1,
    )

    assessment = result.assessment("case_inspect_001")

    assert result.lineage(result.best.id) == [result.best]
    assert list(result.test_assessments()) == [assessment]
    assert assessment.case.target == {"answer": "42"}
    assert assessment.score.value == 0.75
    assert [(reward.id, reward.value) for reward in assessment.rewards] == [
        ("correct", 1.0),
        ("concise", 0.25),
    ]


def _json_object(value: JsonValue) -> JsonObject:
    assert isinstance(value, dict)
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    assert isinstance(value, list)
    return value


def _json_rpc_line(request: StageRunRequest) -> str:
    return encode_request(
        method=request.method,
        request_id=request.request_id,
        params=request.to_params(),
    ).decode()


class FakeSeamClient:
    def __init__(self) -> None:
        self.request_value = AgentRunRequest(
            request_id="unset",
            plan_id="unset",
            candidate="unset",
            workspace="unset",
            instructions={"task": "unset"},
            idempotency_prefix="unset",
        )

    def agent_run(self, request: AgentRunRequest) -> AgentRunResult:
        self.request_value = request
        return AgentRunResult(
            method="leaven/agent.run",
            primary=AgentSessionPrimary(
                kind="agent_session",
                status="completed",
                receipt="agentrec_completion",
                transcript_ref=WireBlobRef(
                    id="blob_agent_builder_transcript",
                    sha256="a" * 64,
                    bytes=128,
                    data_classes=["transcript.raw"],
                ),
                commands=[
                    AgentCommandRecord(
                        argv=["codex", "exec"],
                        status="completed",
                        receipt="agentrec_completion",
                    )
                ],
                cost=Cost(usd_micro=250_000),
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )


class FakeLmSeamClient:
    def __init__(self) -> None:
        self.request_value = LmCompleteRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            messages=[{"role": "user", "content": [{"kind": "text", "text": "unset"}]}],
            model="unset",
        )

    def lm_complete(self, request: LmCompleteRequest) -> LmCompleteResult:
        self.request_value = request
        return LmCompleteResult(
            method="leaven/lm.complete",
            primary=LmResponsePrimary(
                kind="lm_response",
                message=LmMessageRecord(
                    role="assistant",
                    content=[LmContentPart(kind="text", text="ok")],
                ),
                receipt="lmrec_completion",
                cost=Cost(usd_micro=42, input_tokens=3, output_tokens=2),
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )
