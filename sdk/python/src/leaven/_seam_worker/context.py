"""Role-scoped context bindings for the command-runner worker."""

import json
import sys
from collections.abc import Sequence

from .._seam import LmCompleteRequest
from .._stage_runtime import CallbackProposeContext, CallbackRolloutContext
from .callbacks import CallbackReceiptLog


class JsonRpcCallbackClient:
    """Callback-backed effect client over the active command-runner pipe."""

    def __init__(self, *, lm_model: str) -> None:
        self._lm_model = lm_model
        self._receipts = CallbackReceiptLog()

    def request(self, request: dict) -> dict:
        """Send one callback request and return the public-seam result object."""
        print(json.dumps(request, sort_keys=True), flush=True)
        line = sys.stdin.readline()
        if not line:
            raise RuntimeError("stage host closed before answering callback request")
        response = json.loads(line)
        if "error" in response:
            raise RuntimeError(f"stage callback failed: {response['error']}")
        result = response["result"]
        method = request.get("method")
        if isinstance(method, str) and isinstance(result, dict):
            self._receipts.record_result(method=method, result=result)
        return result

    def effect_receipts_json(self) -> list[dict[str, object]]:
        """Return effect receipts observed while running the current stage."""
        return self._receipts.effect_receipts_json()

    def proposal_receipts_json(self) -> list[dict[str, object]]:
        """Return proposal write receipts observed while running the current stage."""
        return self._receipts.proposal_receipts_json()

    async def lm_complete(
        self,
        prompt: str,
        *,
        request_id: str,
        model: str,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        input_classes: Sequence[str] | None = None,
    ) -> dict:
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
            model=model,
            model_role=model_role,
            temperature=temperature,
            max_tokens=max_tokens,
            stop=stop,
            input_classes=input_classes or ["public"],
        ).to_json_rpc()
        return self.request(request)


def rollout_context(
    *,
    candidate_id: str,
    stage_call_id: str,
    lm_model: str,
    callback: JsonRpcCallbackClient | None = None,
) -> CallbackRolloutContext:
    """Build the context passed to a registered runner stage."""
    callback = callback or JsonRpcCallbackClient(lm_model=lm_model)
    return CallbackRolloutContext(
        callback,
        candidate_id=candidate_id,
        stage_call_id=stage_call_id,
        lm_model=lm_model,
        agent_callback=callback,
    )


def propose_context(
    *,
    parent_candidate_id: str,
    stage_call_id: str,
    lm_model: str,
    callback: JsonRpcCallbackClient | None = None,
) -> CallbackProposeContext:
    """Build the context passed to a registered proposer stage."""
    callback = callback or JsonRpcCallbackClient(lm_model=lm_model)
    return CallbackProposeContext(
        callback,
        parent_candidate_id=parent_candidate_id,
        stage_call_id=stage_call_id,
        lm_model=lm_model,
        agent_callback=callback,
        proposal_callback=callback,
    )


def _plan_id(request_id: str) -> str:
    return "plan_" + _id_fragment(request_id)


def _idempotency_key(request_id: str) -> str:
    return "lm_" + _id_fragment(request_id)


def _id_fragment(value: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in value)


__all__ = ["JsonRpcCallbackClient", "propose_context", "rollout_context"]
