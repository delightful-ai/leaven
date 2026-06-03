"""Role-scoped context bindings for the command-runner worker."""

from __future__ import annotations

import json
import sys

from .._seam import LmCompleteRequest
from .._stage_runtime import CallbackRolloutContext


class JsonRpcCallbackClient:
    """Callback-backed effect client over the active command-runner pipe."""

    def request(self, request: dict) -> dict:
        """Send one callback request and return the public-seam result object."""
        print(json.dumps(request, sort_keys=True), flush=True)
        line = sys.stdin.readline()
        if not line:
            raise RuntimeError("stage host closed before answering callback request")
        response = json.loads(line)
        if "error" in response:
            raise RuntimeError(f"stage callback failed: {response['error']}")
        return response["result"]

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
        content = self.request(request)["primary"]["message"]["content"]
        return "".join(part["text"] for part in content if part.get("kind") == "text")


def rollout_context(
    *,
    candidate_id: str,
    stage_call_id: str,
) -> CallbackRolloutContext:
    """Build the context passed to a registered runner stage."""
    callback = JsonRpcCallbackClient()
    return CallbackRolloutContext(
        callback,
        candidate_id=candidate_id,
        stage_call_id=stage_call_id,
        agent_callback=callback,
    )


def _plan_id(request_id: str) -> str:
    return "plan_" + _id_fragment(request_id)


def _idempotency_key(request_id: str) -> str:
    return "lm_" + _id_fragment(request_id)


def _id_fragment(value: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in value)


__all__ = ["JsonRpcCallbackClient", "rollout_context"]
