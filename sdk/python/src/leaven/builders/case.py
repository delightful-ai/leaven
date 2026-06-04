"""`cx.case.*` — case loading and queries."""

import asyncio
from collections.abc import Sequence
from typing import Any, Literal, Protocol, cast

from .._seam import CaseLoadRequest
from ..case import Case

CaseField = Literal["input", "target", "metadata", "files", "setup", "sandbox", "split"]


class CaseBuilder:
    """Case loader bound to a context.

    Returned `Case` values are ordinary user-facing records. The engine still
    records read receipts internally for audit/replay.
    """

    def __init__(
        self,
        *,
        _client: "_SeamRequester | None" = None,
        _idempotency_prefix: str = "case-builder",
        _plan_id: str = "planpythoncasebuilder001",
        _run_id: str = "run_python_case_builder",
    ) -> None:
        self._client = _client
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id
        self._run_id = _run_id
        self._seq = 0

    @classmethod
    def _for_seam(
        cls,
        client: "_SeamRequester",
        *,
        idempotency_prefix: str = "case-builder",
        plan_id: str = "planpythoncasebuilder001",
        run_id: str = "run_python_case_builder",
    ) -> "CaseBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
            _run_id=run_id,
        )

    async def load(
        self,
        case_id: str,
        *,
        include: Sequence[CaseField] = ("input", "metadata"),
    ) -> Case:
        """Load a case by id.

        `include` controls projection. By default `target` is NOT included
        (target-safe read for runners/reflectors). Evaluators/scorers/judges
        include `target` explicitly when needed; the seam enforces that the
        capability authorizes it.
        """
        if self._client is None:
            raise NotImplementedError(
                "CaseBuilder.load needs an engine-bound public-seam client; "
                "use the cx.case instance supplied to a running evaluator or judge"
            )

        request = CaseLoadRequest(
            request_id=f"{self._idempotency_prefix}-case-{self._seq}",
            plan_id=self._plan_id,
            case_id=case_id,
            include=include,
            run_id=self._run_id,
        )
        self._seq += 1
        result = await asyncio.to_thread(self._client.request, request.to_json_rpc())
        return _case_from_result(result)

    async def load_batch(
        self,
        case_ids: Sequence[str],
        *,
        include: Sequence[CaseField] = ("input", "metadata"),
    ) -> list[Case]:
        """Load multiple cases in one round-trip."""
        return [await self.load(case_id, include=include) for case_id in case_ids]


class _SeamRequester(Protocol):
    """Small private protocol CaseBuilder needs from the seam client."""

    def request(self, request: dict[str, Any]) -> dict[str, Any]: ...


def _case_from_result(result: dict[str, Any]) -> Case:
    primary = result["primary"]
    if not isinstance(primary, dict):
        raise TypeError("case query result primary must be an object")
    record = cast("dict[str, Any]", primary)
    return Case(
        id=str(record["case"]),
        input=_object_field(record, "input"),
        target=_optional_object_field(record, "target"),
        metadata=_object_field(record, "metadata"),
    )


def _object_field(record: dict[str, Any], field: str) -> dict[str, Any]:
    value = record.get(field, {})
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise TypeError(f"case query result {field} must be an object")
    return cast("dict[str, Any]", value)


def _optional_object_field(record: dict[str, Any], field: str) -> dict[str, Any] | None:
    if field not in record or record[field] is None:
        return None
    return _object_field(record, field)


__all__ = ["CaseBuilder", "CaseField"]
