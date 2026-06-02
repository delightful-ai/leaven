"""Standalone Python runner worker served over the locked Leaven ACP stdio seam.

This is the slice-3 `serve_stage` server loop for the prompt/LM/exact-match path.
It is the worker side of the bidirectional seam example 03 rides:

  1. The engine spawns this script with the locked capability env
     (`LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, `LEAVEN_CAPABILITY_FINGERPRINT`).
  2. The engine dispatches `leaven/stage.run` (a target-free runner stage). The
     worker reads the model-facing input the engine projected into `case_input`.
  3. The worker runs the rollout: it calls `leaven/lm.complete` BACK into the
     engine (the worker is the ACP agent; the engine is the ACP client). The host
     services the callback against its deterministic mock LM and replies.
  4. The worker returns a `stage_run_result` carrying the completion as a text
     `OutputRecord`.

Honest scope (slice 3): only the runner stage and the `leaven/lm.complete`
callback are exercised. The candidate prompt template is host-side optimization
state; the engine renders it against the case and projects the rendered,
model-facing prompt into `case_input["prompt"]`, which the runner sends to the
LM. The worker never sees the case target. Candidate materialization,
`graph.query`, the reward vector, agent, and sandbox are later slices.
"""

from __future__ import annotations

import json
import os
import sys


def _read_message():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)


def _write_message(message):
    sys.stdout.write(json.dumps(message, sort_keys=True))
    sys.stdout.write("\n")
    sys.stdout.flush()


def _lm_complete(prompt, request_id):
    """Worker-initiated `leaven/lm.complete`: bind the prompt and read the reply.

    Returns the assistant text the host's LM produced. The host stamps the
    launched capability fingerprint onto the reply; the worker verifies it.
    """
    fingerprint = os.environ["LEAVEN_CAPABILITY_FINGERPRINT"]
    _write_message(
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
    response = _read_message()
    if response is None:
        raise RuntimeError("host closed the seam before answering leaven/lm.complete")
    result = response["result"]
    assert result["method"] == "leaven/lm.complete", result
    assert result["capability_fingerprint"] == fingerprint, result
    content = result["primary"]["message"]["content"]
    return "".join(part["text"] for part in content if part.get("kind") == "text")


def _run_runner_stage(request, request_id):
    """Run one target-free runner rollout and return its `stage_run_result`."""
    payload = request["params"]["payload"]
    assert payload["role"] == "runner", payload
    assert payload["target_forbidden"] is True, payload
    case_input = payload["case_input"]
    # The engine projected the rendered, model-facing prompt into the case input.
    prompt = case_input["prompt"]

    # The rollout body: call the LM back over the seam and use its text output.
    completion = _lm_complete(prompt, request_id=f"{payload['stage_call_id']}::lm")
    output_text = completion.strip()

    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {
            "schema_version": "leaven.stage_run.v1",
            "message": "stage_run_result",
            "stage": "runner",
            "stage_call_id": payload["stage_call_id"],
            "output": {
                "kind": "text",
                "summary": f"runner output for {payload['case']}",
                "value": output_text,
                "visibility": "optimizer_visible",
                "data_classes": ["candidate.output"],
            },
        },
    }


def serve() -> None:
    """The serve_stage loop: dispatch stage calls until the session terminates."""
    # The locked capability env must be present; the engine injects it at spawn.
    for required in (
        "LEAVEN_CAPABILITY_TOKEN",
        "LEAVEN_ENDPOINT",
        "LEAVEN_CAPABILITY_FINGERPRINT",
    ):
        if required not in os.environ:
            raise RuntimeError(f"missing locked capability env `{required}`")

    while True:
        message = _read_message()
        if message is None:
            return
        if message.get("method") == "leaven/stage.run":
            response = _run_runner_stage(message, message["id"])
            _write_message(response)
        else:
            # Slice 3 serves only the runner stage dispatch.
            raise RuntimeError(f"unexpected inbound method: {message.get('method')!r}")


if __name__ == "__main__":
    serve()
