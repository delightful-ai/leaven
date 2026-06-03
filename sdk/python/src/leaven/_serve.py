"""The Python ACP AGENT loop that drives `leaven serve --stdio`.

This is the worker side of the locked Leaven ACP bidirectional seam, the Python
generalization of the proven Rust bridge worker
(`crates/leaven-acp-stage-bridge/worker/serve_stage_runner.py`). The
directionality is the crux and is fixed by the governing spec (the engine is the
ACP client, the worker is the ACP agent):

  - `optimize().run()` SPAWNS `leaven serve --stdio` as a child subprocess and
    injects the locked capability env. The CHILD is the ACP client: it owns the
    tiny real GEPA accept loop, the deterministic host mock LM, and INITIATES
    `leaven/stage.run` dispatches over the seam.
  - This module is the ACP agent: it SERVES `leaven/stage.run` by running the
    user's `@lv.runner` rollout, and INITIATES `leaven/lm.complete` BACK to the
    child (serviced by the child's host mock LM). It returns the rollout output
    as a `stage_run_result`.

The transport is the child's inherited stdio: the child's stdout carries its
JSON-RPC dispatches to us and our `leaven/lm.complete` replies; the child's
stdin carries our `leaven/lm.complete` requests and our `stage_run_result`
responses. The optimize plan travels by `--plan` file and the `Optimized` result
by `--out` file so stdin/stdout stay a pure JSON-RPC channel.

Honest scope (the first SDK product-proof, prompt/LM/exact-match path): the
runner stage and the `leaven/lm.complete` callback are exercised for real over
the live seam; the LM is the child's deterministic mock (no spend, no network);
the reward (exact match) and reflector run host-side in `leaven serve` and are
named declaratively in the plan. The reward vector, agent, and sandbox are later
slices.
"""

from __future__ import annotations

import asyncio
import json
import os
import subprocess
import tempfile
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from ._receipts import CallReceipt
from .artifacts.prompt import PromptArtifact
from .builders.lm import LmBuilder, LmMessage, LmResponse
from .case import InputCaseView
from .contexts import RolloutContext
from .decorators import RegisteredStage

# The locked capability env the child requires (the ACP profile auth block).
_CAPABILITY_TOKEN_ENV = "LEAVEN_CAPABILITY_TOKEN"
_CAPABILITY_ENDPOINT_ENV = "LEAVEN_ENDPOINT"
_CAPABILITY_FINGERPRINT_ENV = "LEAVEN_CAPABILITY_FINGERPRINT"

# Deterministic capability material for the local no-spend path. The child stamps
# the fingerprint onto each `leaven/lm.complete` reply; we verify it.
_LOCAL_TOKEN = "leaven-local-stdio"
_LOCAL_ENDPOINT = "stdio://leaven/optimize"
_LOCAL_FINGERPRINT = "fp_cap_sha256_leaven_py_optimize"


class ServeError(RuntimeError):
    """Raised when the `leaven serve --stdio` child cannot be driven to completion."""


def resolve_leaven_binary() -> Path:
    """Locate the `leaven` binary the SDK spawns via `leaven serve --stdio`.

    Resolution order, mirroring how the Rust integration tests / p9 resolve
    binaries: an explicit `LEAVEN_BIN` env override, then `target/debug/leaven`
    (then `target/release/leaven`) under the discovered repo root.
    """
    override = os.environ.get("LEAVEN_BIN")
    if override:
        binary = Path(override)
        if not binary.is_file():
            raise ServeError(f"LEAVEN_BIN={override!r} is not an executable file")
        return binary

    root = resolve_repo_root()
    for profile in ("debug", "release"):
        candidate = root / "target" / profile / "leaven"
        if candidate.is_file():
            return candidate
    raise ServeError(
        "could not find the `leaven` binary; build it with "
        "`cargo build -p leaven-cli` or set LEAVEN_BIN to its path "
        f"(looked under {root / 'target'})"
    )


def resolve_repo_root() -> Path:
    """Walk up from this package to the Leaven Cargo workspace root.

    The locked public-seam package loads from the repo root; the executable
    inventory guard `crates/leaven/tests/topology_contract.rs` marks it. An
    explicit `LEAVEN_REPO_ROOT` env override wins for out-of-tree installs.
    """
    override = os.environ.get("LEAVEN_REPO_ROOT")
    if override:
        root = Path(override)
        if not (root / "Cargo.toml").is_file():
            raise ServeError(f"LEAVEN_REPO_ROOT={override!r} has no Cargo.toml")
        return root

    marker = Path("crates/leaven/tests/topology_contract.rs")
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").is_file() and (parent / marker).is_file():
            return parent
    raise ServeError(
        "could not locate the Leaven Cargo workspace root from "
        f"{Path(__file__).resolve()}; set LEAVEN_REPO_ROOT to the repo root"
    )


async def run_optimization(
    *,
    seed: PromptArtifact,
    cases: list[dict[str, Any]],
    runner: RegisteredStage[Any, Any],
    run_id: str,
    minibatch: int,
    max_iterations: int,
    reward_name: str,
    reflect_name: str,
) -> dict[str, Any]:
    """Spawn `leaven serve --stdio`, drive it as the ACP agent, return its result.

    `cases` are plan rows `{case_id, input, target}`. `runner` is the user's
    `@lv.runner` stage that this process runs to serve each `leaven/stage.run`.
    `reward_name`/`reflect_name` select the host-side reward/reflector the child
    runs by name (the slice-3 scalar exact-match path). Returns the parsed
    `Optimized` result JSON the child wrote to its `--out` file.

    Runs inside the caller's event loop (`optimize().run()` is `async`); the
    child is driven concurrently via the asyncio subprocess transport.
    """
    binary = resolve_leaven_binary()
    root = resolve_repo_root()
    plan = {
        "run_id": run_id,
        "seed_template": seed.template,
        "cases": cases,
        "minibatch": minibatch,
        "max_iterations": max_iterations,
        "reward": reward_name,
        "reflect": reflect_name,
    }
    return await _drive_child(binary, root, plan, runner)


async def _drive_child(
    binary: Path,
    root: Path,
    plan: Mapping[str, Any],
    runner: RegisteredStage[Any, Any],
) -> dict[str, Any]:
    """Spawn the child, run the agent loop until it closes the seam, read the result."""
    with tempfile.TemporaryDirectory(prefix="leaven-optimize-") as workdir:
        plan_path = Path(workdir) / "plan.json"
        out_path = Path(workdir) / "result.json"
        plan_path.write_text(json.dumps(plan, sort_keys=True))

        env = dict(os.environ)
        env[_CAPABILITY_TOKEN_ENV] = _LOCAL_TOKEN
        env[_CAPABILITY_ENDPOINT_ENV] = _LOCAL_ENDPOINT
        env[_CAPABILITY_FINGERPRINT_ENV] = _LOCAL_FINGERPRINT

        process = await asyncio.create_subprocess_exec(
            str(binary),
            "serve",
            "--stdio",
            "--root",
            str(root),
            "--plan",
            str(plan_path),
            "--out",
            str(out_path),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=None,  # operator diagnostics inherit to our stderr
            env=env,
        )
        agent = _ParentAgent(process, runner)
        await agent.serve_until_eof()

        code = await process.wait()
        if code != 0:
            raise ServeError(f"leaven serve --stdio exited with status {code}")
        if not out_path.is_file():
            raise ServeError("leaven serve --stdio did not write an Optimized result")
        return json.loads(out_path.read_text())


class _ParentAgent:
    """The ACP agent over the child's inherited stdio.

    The child (`leaven serve`) is the ACP client; this agent serves the
    `leaven/stage.run` dispatches it initiates and calls `leaven/lm.complete`
    back into the child's host LM, exactly like the proven `serve_stage_runner.py`.
    """

    def __init__(
        self,
        process: asyncio.subprocess.Process,
        runner: RegisteredStage[Any, Any],
    ) -> None:
        if process.stdin is None or process.stdout is None:
            raise ServeError("child process is missing stdin/stdout pipes")
        self._stdin = process.stdin
        self._stdout = process.stdout
        self._runner = runner
        self._fingerprint = _LOCAL_FINGERPRINT

    async def serve_until_eof(self) -> None:
        """Serve runner-stage dispatches until the child closes the seam."""
        while (message := await self._read_message()) is not None:
            method = message.get("method")
            if method != "leaven/stage.run":
                raise ServeError(f"unexpected inbound method from child: {method!r}")
            await self._serve_runner_stage(message)

    async def _serve_runner_stage(self, request: Mapping[str, Any]) -> None:
        """Run one target-free runner rollout and return its `stage_run_result`."""
        payload = request["params"]["payload"]
        if payload.get("role") != "runner":
            raise ServeError(f"stage.run payload is not a runner role: {payload!r}")
        if payload.get("target_forbidden") is not True:
            raise ServeError("runner stage payload must be target-free")
        stage_call_id = payload["stage_call_id"]

        output = await self._run_user_runner(payload, stage_call_id)

        await self._write_message(
            {
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "schema_version": "leaven.stage_run.v1",
                    "message": "stage_run_result",
                    "stage": "runner",
                    "stage_call_id": stage_call_id,
                    "output": {
                        "kind": "text",
                        "summary": f"runner output for {payload['case']}",
                        "value": output,
                        "visibility": "optimizer_visible",
                        "data_classes": ["candidate.output"],
                    },
                },
            }
        )

    async def _run_user_runner(self, payload: Mapping[str, Any], stage_call_id: str) -> str:
        """Invoke the user's `@lv.runner` body for one case over the live seam.

        The engine rendered the candidate template host-side and projected the
        model-facing prompt into `case_input["prompt"]`. We reconstruct the
        `PromptArtifact` and `InputCaseView` the user's `run(prompt, case, cx)`
        signature expects: the artifact template IS the rendered prompt, so the
        user's `prompt.template.format(**case.input)` is idempotent (the
        placeholders are already substituted) and yields the same prompt the
        engine intended. The runner never sees the case target.
        """
        case_input = dict(payload["case_input"])
        rendered_prompt = case_input["prompt"]
        prompt = PromptArtifact(template=rendered_prompt)
        # Expose the original (target-free) case input, minus the rendered-prompt
        # projection key, as the structural InputCaseView the runner reads.
        view_input = {k: v for k, v in case_input.items() if k != "prompt"}
        case = InputCaseView(id=payload["case"], input=view_input)
        cx = _SeamRolloutContext(self, payload["candidate"], stage_call_id)

        result = await self._runner.func(prompt, case, cx)
        output = result if isinstance(result, str) else str(result)
        return output.strip()

    async def lm_complete(self, prompt: str, *, request_id: str) -> str:
        """Worker-initiated `leaven/lm.complete`: bind the prompt, read the reply.

        This is the bidirectional bit. We send the rendered prompt to the child as
        a `leaven/lm.complete` request (Plan IR binding it as a `prompt` literal
        op, matching the host's lowering), and the child's deterministic host LM
        answers on its stdout. The child stamps the launched capability
        fingerprint onto the reply; we verify it.
        """
        await self._write_message(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "leaven/lm.complete",
                "params": {
                    "schema_version": "leaven.plan.v1",
                    "plan_id": "plan_runner_lm_complete",
                    "consistency": {"kind": "latest_at_start"},
                    "mode": {"kind": "dry_run"},
                    "ops": [
                        {
                            "kind": "let",
                            "name": "prompt",
                            "expr": {
                                "kind": "literal",
                                "value": prompt,
                                "data_classes": ["public"],
                            },
                        }
                    ],
                    "return": ["prompt"],
                    "commit": {"kind": "no_graph_writes"},
                },
            }
        )
        reply = await self._read_message()
        if reply is None:
            raise ServeError("child closed the seam before answering leaven/lm.complete")
        result = reply["result"]
        if result.get("method") != "leaven/lm.complete":
            raise ServeError(f"unexpected lm.complete reply: {reply!r}")
        if result.get("capability_fingerprint") != self._fingerprint:
            raise ServeError(
                "leaven/lm.complete reply carries a foreign capability fingerprint: "
                f"{result.get('capability_fingerprint')!r}"
            )
        content = result["primary"]["message"]["content"]
        return "".join(part["text"] for part in content if part.get("kind") == "text")

    async def _read_message(self) -> dict[str, Any] | None:
        line = await self._stdout.readline()
        if not line:
            return None
        return json.loads(line)

    async def _write_message(self, message: Mapping[str, Any]) -> None:
        self._stdin.write((json.dumps(message, sort_keys=True) + "\n").encode())
        await self._stdin.drain()


class _SeamLmBuilder(LmBuilder):
    """A live `cx.lm` bound to the parent agent's `leaven/lm.complete` callback.

    Only the slice-3 prompt path is wired: `complete(prompt=..., ...)` ships the
    prompt over the seam and returns the child host LM's completion. Message
    lists, model/role selection, tools, and structured output are later slices.
    """

    def __init__(self, agent: _ParentAgent, stage_call_id: str) -> None:
        self._agent = agent
        self._stage_call_id = stage_call_id
        self._seq = 0

    async def complete(  # type: ignore[override]
        self,
        *,
        prompt: str | None = None,
        messages: Sequence[LmMessage] | Sequence[dict[str, Any]] | None = None,
        model: str | None = None,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        response_format: Any | None = None,
        tools: Sequence[dict[str, Any]] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> LmResponse:
        if prompt is None:
            raise ServeError("cx.lm.complete requires `prompt=` in this slice")
        request_id = f"{self._stage_call_id}::lm::{self._seq}"
        self._seq += 1
        text = await self._agent.lm_complete(prompt, request_id=request_id)
        return _lm_response(text)


class _SeamRolloutContext(RolloutContext):
    """A live `RolloutContext` for one rollout, bound to the parent agent's seam.

    `cx.lm.complete(...)` routes through the bidirectional seam to the child's
    host LM. The other effect builders (`agent`, `sandbox`, `workspace`, `batch`)
    stay scaffold for this slice — the prompt/LM/exact-match path uses only `lm`.
    """

    def __init__(self, agent: _ParentAgent, candidate_id: str, stage_call_id: str) -> None:
        self.lm = _SeamLmBuilder(agent, stage_call_id)
        self._candidate_id = candidate_id
        self._stage_call_id = stage_call_id

    @property
    def candidate_id(self) -> str:
        return self._candidate_id

    @property
    def stage_id(self) -> str:
        return self._stage_call_id


def _lm_response(text: str) -> LmResponse:
    """Build the `LmResponse` a `cx.lm.complete(...)` returns from the seam reply.

    Slice 3 carries only the completion text over the seam (the deterministic
    mock LM emits no token usage or cost), so the response reports zero usage and
    no cost; the bidirectional seam, not the LM telemetry, is what this slice
    proves.
    """
    return LmResponse(
        text=text,
        finish_reason="stop",
        usage={"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
        cost_usd=0.0,
        model="leaven-serve-mock",
        receipt=CallReceipt(receipt_id="lmrec_leaven_py_optimize"),
    )


__all__ = [
    "ServeError",
    "resolve_leaven_binary",
    "resolve_repo_root",
    "run_optimization",
]
