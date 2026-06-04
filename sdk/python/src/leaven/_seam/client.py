"""One-shot process client for `leaven seam serve --stdio`."""

import json
import subprocess
import tempfile
from pathlib import Path

from msgspec import UNSET, UnsetType

from ._wire import (
    JsonObject,
    JsonRpcId,
    JsonRpcProtocolError,
    JsonRpcRemoteError,
    require_locked_method,
)
from ._wire.codec import decode_response, encode_request
from .config import SeamServiceConfig
from .errors import SeamClientError
from .resolve import resolve_leaven_binary, resolve_repo_root
from .results import AgentRunResult, CaseLoadResult, LmCompleteResult, ProposalSubmitResult


class SeamClient:
    """One-shot client for the line-delimited public seam process."""

    def __init__(
        self,
        *,
        config: SeamServiceConfig,
        leaven_bin: Path | None = None,
        repo_root: Path | None = None,
    ) -> None:
        self._config = config
        self._leaven_bin = leaven_bin or resolve_leaven_binary()
        self._repo_root = repo_root or resolve_repo_root()

    def agent_run(self, request: JsonObject, *, timeout_s: int = 240) -> AgentRunResult:
        """Send one `leaven/agent.run` request and return its typed result."""
        return self._typed_request(request, AgentRunResult, timeout_s=timeout_s)

    def lm_complete(self, request: JsonObject, *, timeout_s: int = 240) -> LmCompleteResult:
        """Send one `leaven/lm.complete` request and return its typed result."""
        return self._typed_request(request, LmCompleteResult, timeout_s=timeout_s)

    def proposal_submit(self, request: JsonObject, *, timeout_s: int = 240) -> ProposalSubmitResult:
        """Send one `leaven/proposal.submit_batch` request and return its typed result."""
        return self._typed_request(request, ProposalSubmitResult, timeout_s=timeout_s)

    def case_load(self, request: JsonObject, *, timeout_s: int = 240) -> CaseLoadResult:
        """Send one case read request and return its typed result."""
        return self._typed_request(request, CaseLoadResult, timeout_s=timeout_s)

    def _request_bytes(self, request: JsonObject, *, timeout_s: int) -> bytes:
        with tempfile.TemporaryDirectory(prefix="leaven-seam-client-") as tmp:
            config_path = Path(tmp) / "seam-config.json"
            config_path.write_text(
                json.dumps(self._config.to_json(), sort_keys=True),
                encoding="utf-8",
            )
            process = self._run_process(config_path, request, timeout_s)

        if process.returncode != 0:
            raise SeamClientError(
                "leaven seam serve failed\n"
                f"status: {process.returncode}\nstdout:\n{process.stdout}\n"
                f"stderr:\n{process.stderr}"
            )
        return process.stdout.encode()

    def _typed_request[T](
        self,
        request: JsonObject,
        result_type: type[T],
        *,
        timeout_s: int,
    ) -> T:
        body = self._request_bytes(request, timeout_s=timeout_s)
        try:
            return decode_response(body, result_type)
        except JsonRpcRemoteError as error:
            raise SeamClientError(f"seam returned JSON-RPC error: {error.error}") from error
        except JsonRpcProtocolError as error:
            raise SeamClientError(f"seam returned invalid JSON-RPC: {error}") from error

    def _run_process(
        self,
        config_path: Path,
        request: JsonObject,
        timeout_s: int,
    ) -> subprocess.CompletedProcess[str]:
        method = request.get("method")
        if not isinstance(method, str):
            raise SeamClientError("seam request must carry a string method")
        try:
            locked_method = require_locked_method(method)
        except ValueError as error:
            raise SeamClientError(str(error)) from error
        request_id = _request_id(request)
        params = request.get("params", UNSET)
        line = encode_request(method=locked_method, request_id=request_id, params=params).decode()
        return subprocess.run(
            [
                str(self._leaven_bin),
                "seam",
                "serve",
                "--stdio",
                "--root",
                str(self._repo_root),
                "--config",
                str(config_path),
            ],
            input=line + "\n",
            text=True,
            capture_output=True,
            timeout=timeout_s,
            check=False,
        )


def _request_id(request: JsonObject) -> JsonRpcId | UnsetType:
    if "id" not in request:
        return UNSET
    request_id = request["id"]
    if isinstance(request_id, bool):
        raise SeamClientError("seam request id must be a string, integer, or null")
    if request_id is None or isinstance(request_id, str | int):
        return request_id
    raise SeamClientError("seam request id must be a string, integer, or null")


__all__ = ["SeamClient"]
