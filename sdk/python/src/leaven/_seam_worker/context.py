"""Role-scoped context bindings for the command-runner worker."""

from __future__ import annotations

import json
import sys

from .._seam import LmCompleteRequest
from .._stage_runtime import CallbackRolloutContext


class JsonRpcLmCallback:
    """Callback-backed LM effect over the active command-runner pipe."""

    async def lm_complete(self, prompt: str, *, request_id: str) -> str:
        """Send one `leaven/lm.complete` callback request and read the response."""
        request = LmCompleteRequest(
            request_id=request_id,
            plan_id=_plan_id(request_id),
            idempotency_key=_idempotency_key(request_id),
            messages=[
                {
                    "role": "user",
                    "content": [{"kind": "text", "text": prompt}],
                }
            ],
            model="mock",
            input_classes=["public"],
        ).to_json_rpc()
        print(json.dumps(request, sort_keys=True), flush=True)
        line = sys.stdin.readline()
        if not line:
            raise RuntimeError("stage host closed before answering leaven/lm.complete")
        response = json.loads(line)
        if "error" in response:
            raise RuntimeError(f"leaven/lm.complete callback failed: {response['error']}")
        content = response["result"]["primary"]["message"]["content"]
        return "".join(part["text"] for part in content if part.get("kind") == "text")


def rollout_context(
    *,
    candidate_id: str,
    stage_call_id: str,
) -> CallbackRolloutContext:
    """Build the context passed to a registered runner stage."""
    return CallbackRolloutContext(
        JsonRpcLmCallback(),
        candidate_id=candidate_id,
        stage_call_id=stage_call_id,
    )


def _plan_id(request_id: str) -> str:
    return "plan_" + _id_fragment(request_id)


def _idempotency_key(request_id: str) -> str:
    return "lm_" + _id_fragment(request_id)


def _id_fragment(value: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in value)


__all__ = ["JsonRpcLmCallback", "rollout_context"]
