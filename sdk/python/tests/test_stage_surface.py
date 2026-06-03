from __future__ import annotations

import leaven as lv
from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven.builders.agent import AgentBuilder


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
