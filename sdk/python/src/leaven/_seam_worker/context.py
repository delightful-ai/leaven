"""Role-scoped context bindings for the command-runner worker."""

import sys
from collections.abc import Sequence

import msgspec
from msgspec import UNSET, Struct, UnsetType

from .._seam import (
    AgentRunRequest,
    CaseLoadRequest,
    LmCompleteRequest,
    ProposalSubmitRequest,
    SeamJsonRpcRequest,
)
from .._seam._wire.calls import LmContentText, LmMessage
from .._seam._wire.codec import decode_method_response, encode_request
from .._seam._wire.errors import JsonRpcProtocolError, JsonRpcRemoteError
from .._seam._wire.jsonrpc import JsonRpcResponseEnvelope
from .._seam._wire.payloads import StageEffectReceipt, StageProposalReceipt
from .._seam._wire.refs import WireJsonLiteralDepth8
from .._seam._wire.results import AgentRunResult, LmCompleteResult, ProposalSubmitResult
from .._stage_runtime import CallbackProposeContext, CallbackRolloutContext
from .callbacks import CallbackReceiptLog, CallbackResult


class _CaseReadPrimary(Struct, frozen=True):
    """Host worker-callback case-read primary (a subset of `case_record`)."""

    kind: str
    case: str
    data_classes: list[str]
    input: WireJsonLiteralDepth8 | UnsetType = UNSET
    target: WireJsonLiteralDepth8 | UnsetType = UNSET
    metadata: WireJsonLiteralDepth8 | UnsetType = UNSET


class CaseReadResponse(Struct, frozen=True):
    """Host worker-callback case-read result; the scorer reads `primary`.

    This is the simplified shape the optimize host returns for a worker
    `leaven/case.*` callback during scorer dispatch, distinct from the full
    public `CaseLoadResult` returned by a top-level client case read. The host
    also serves read receipts alongside `primary`; the scorer does not consume
    them, and msgspec tolerates the extra field, so it is not declared here
    rather than widening a callback-response field to `object`.
    """

    primary: _CaseReadPrimary


_CASE_READ_RESPONSE_DECODER = msgspec.json.Decoder(JsonRpcResponseEnvelope)


class JsonRpcCallbackClient:
    """Callback-backed effect client over the active command-runner pipe."""

    def __init__(self, *, lm_model: str) -> None:
        self._lm_model = lm_model
        self._receipts = CallbackReceiptLog()

    def _request_result[T: CallbackResult](
        self,
        request: SeamJsonRpcRequest,
        result_type: type[T],
    ) -> T:
        """Send one callback request and return the public-seam result object."""
        print(
            encode_request(
                method=request.method,
                request_id=request.request_id,
                params=request.to_params(),
            ).decode(),
            flush=True,
        )
        line = sys.stdin.readline()
        if not line:
            raise RuntimeError("stage host closed before answering callback request")
        try:
            result = decode_method_response(line.encode(), request.method)
        except JsonRpcRemoteError as error:
            raise RuntimeError(f"stage callback failed: {error.error}") from error
        except JsonRpcProtocolError as error:
            raise RuntimeError(f"stage callback returned invalid JSON-RPC: {error}") from error
        if not isinstance(result, result_type):
            raise TypeError(f"stage callback returned {type(result).__name__} for {request.method}")
        self._receipts.record_result(result)
        return result

    def agent_run(self, request: AgentRunRequest) -> AgentRunResult:
        """Send one `leaven/agent.run` callback and decode the typed result."""
        return self._request_result(request, AgentRunResult)

    def proposal_submit(self, request: ProposalSubmitRequest) -> ProposalSubmitResult:
        """Send one `leaven/proposal.submit_batch` callback and decode the typed result."""
        return self._request_result(request, ProposalSubmitResult)

    def case_read(self, request: CaseLoadRequest) -> CaseReadResponse:
        """Send one `leaven/case.*` callback and decode the host case-read result.

        Used by scorer-stage dispatch to read the case target/input/metadata the
        optimize host serves with read receipts. The response is the host's
        simplified worker-callback case-read shape, not the full client
        `CaseLoadResult`.
        """
        print(
            encode_request(
                method=request.method,
                request_id=request.request_id,
                params=request.to_params(),
            ).decode(),
            flush=True,
        )
        line = sys.stdin.readline()
        if not line:
            raise RuntimeError("stage host closed before answering case read callback")
        envelope = _CASE_READ_RESPONSE_DECODER.decode(line.encode())
        if envelope.error is not UNSET:
            raise RuntimeError(f"case read callback was refused: {envelope.error}")
        if envelope.result is UNSET:
            raise RuntimeError("case read callback response carried neither result nor error")
        return msgspec.json.decode(bytes(envelope.result), type=CaseReadResponse)

    def effect_receipts(self) -> list[StageEffectReceipt]:
        """Return effect receipts observed while running the current stage."""
        return self._receipts.effect_receipts()

    def proposal_receipts(self) -> list[StageProposalReceipt]:
        """Return proposal write receipts observed while running the current stage."""
        return self._receipts.proposal_receipts()

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
    ) -> LmCompleteResult:
        """Send one `leaven/lm.complete` callback request and read the response."""
        request = LmCompleteRequest(
            request_id=request_id,
            plan_id=_plan_id(request_id),
            idempotency_key=_idempotency_key(request_id),
            messages=[LmMessage(role="user", content=[LmContentText(text=prompt)])],
            model=model,
            model_role=model_role,
            temperature=temperature,
            max_tokens=max_tokens,
            stop=stop,
            input_classes=input_classes or ["public"],
        )
        return self._request_result(request, LmCompleteResult)


def rollout_context(
    *,
    candidate_id: str,
    stage_call_id: str,
    capability_fingerprint: str,
    lm_model: str,
    callback: JsonRpcCallbackClient | None = None,
) -> CallbackRolloutContext:
    """Build the context passed to a registered runner stage."""
    callback = callback or JsonRpcCallbackClient(lm_model=lm_model)
    return CallbackRolloutContext(
        callback,
        candidate_id=candidate_id,
        stage_call_id=stage_call_id,
        capability_fingerprint=capability_fingerprint,
        lm_model=lm_model,
        agent_callback=callback,
    )


def propose_context(
    *,
    parent_candidate_id: str,
    stage_call_id: str,
    capability_fingerprint: str,
    lm_model: str,
    callback: JsonRpcCallbackClient | None = None,
) -> CallbackProposeContext:
    """Build the context passed to a registered proposer stage."""
    callback = callback or JsonRpcCallbackClient(lm_model=lm_model)
    return CallbackProposeContext(
        callback,
        parent_candidate_id=parent_candidate_id,
        stage_call_id=stage_call_id,
        capability_fingerprint=capability_fingerprint,
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
