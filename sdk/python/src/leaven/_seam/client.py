"""One-shot process client for `leaven seam serve --stdio`."""

import subprocess
import tempfile
from pathlib import Path

from ._wire import (
    JsonRpcId,
    JsonRpcProtocolError,
    JsonRpcRemoteError,
)
from ._wire.codec import decode_method_response, encode_request
from ._wire.results import (
    AgentRunResult,
    AssessmentSubmitResult,
    CaseLoadResult,
    EvaluationRequestResult,
    EventEmitResult,
    LmCompleteResult,
    ProposalApplyResult,
    ProposalSubmitResult,
    SandboxExecResult,
    StageRunDispatchResult,
    WorkspaceCaptureArtifactsResult,
    WorkspaceDigestResult,
    WorkspaceGitDiffResult,
    WorkspaceGitLogResult,
    WorkspaceGitStatusResult,
    WorkspaceListResult,
    WorkspaceMaterializeResult,
    WorkspaceReadFileResult,
    WorkspaceReleaseResult,
    WorkspaceSnapshotResult,
    WorkspaceStatResult,
)
from .config import SeamServiceConfig
from .effect_plans import EvaluationRequestRequest, EventEmitRequest
from .errors import SeamClientError
from .lm_plans import LmCompleteRequest
from .plans import (
    AgentRunRequest,
    AssessmentSubmitRequest,
    CaseLoadRequest,
    ProposalApplyRequest,
    ProposalSubmitRequest,
    SandboxExecRequest,
    SeamJsonRpcRequest,
    StageRunProposeRequest,
    StageRunRequest,
)
from .resolve import resolve_leaven_binary, resolve_repo_root
from .workspace_plans import (
    WorkspaceMaterializeRequest,
    WorkspaceQueryRequest,
    WorkspaceReleaseRequest,
)


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

    def agent_run(self, request: AgentRunRequest, *, timeout_s: int = 240) -> AgentRunResult:
        """Send one `leaven/agent.run` request and return its typed result."""
        return self._typed_request(request, AgentRunResult, timeout_s=timeout_s)

    def assessment_submit(
        self,
        request: AssessmentSubmitRequest,
        *,
        timeout_s: int = 240,
    ) -> AssessmentSubmitResult:
        """Send one `leaven/assessment.submit` request and return its typed result."""
        return self._typed_request(request, AssessmentSubmitResult, timeout_s=timeout_s)

    def evaluation_request(
        self,
        request: EvaluationRequestRequest,
        *,
        timeout_s: int = 240,
    ) -> EvaluationRequestResult:
        """Send one `leaven/evaluation.request` request and return its typed result."""
        return self._typed_request(request, EvaluationRequestResult, timeout_s=timeout_s)

    def event_emit(
        self,
        request: EventEmitRequest,
        *,
        timeout_s: int = 240,
    ) -> EventEmitResult:
        """Send one `leaven/event.emit` request and return its typed result."""
        return self._typed_request(request, EventEmitResult, timeout_s=timeout_s)

    def lm_complete(self, request: LmCompleteRequest, *, timeout_s: int = 240) -> LmCompleteResult:
        """Send one `leaven/lm.complete` request and return its typed result."""
        return self._typed_request(request, LmCompleteResult, timeout_s=timeout_s)

    def proposal_submit(
        self,
        request: ProposalSubmitRequest,
        *,
        timeout_s: int = 240,
    ) -> ProposalSubmitResult:
        """Send one `leaven/proposal.submit_batch` request and return its typed result."""
        return self._typed_request(request, ProposalSubmitResult, timeout_s=timeout_s)

    def proposal_apply(
        self,
        request: ProposalApplyRequest,
        *,
        timeout_s: int = 240,
    ) -> ProposalApplyResult:
        """Send one `leaven/proposal.apply` request and return its typed result."""
        return self._typed_request(request, ProposalApplyResult, timeout_s=timeout_s)

    def sandbox_exec(
        self,
        request: SandboxExecRequest,
        *,
        timeout_s: int = 240,
    ) -> SandboxExecResult:
        """Send one `leaven/sandbox.exec` request and return its typed result."""
        return self._typed_request(request, SandboxExecResult, timeout_s=timeout_s)

    def workspace_materialize(
        self,
        request: WorkspaceMaterializeRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceMaterializeResult:
        """Send one `leaven/workspace.materialize` request and return its typed result."""
        return self._typed_request(request, WorkspaceMaterializeResult, timeout_s=timeout_s)

    def workspace_release(
        self,
        request: WorkspaceReleaseRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceReleaseResult:
        """Send one `leaven/workspace.release` request and return its typed result."""
        return self._typed_request(request, WorkspaceReleaseResult, timeout_s=timeout_s)

    def workspace_read_file(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceReadFileResult:
        """Send one `leaven/workspace.read_file` request and return its typed result."""
        return self._typed_request(request, WorkspaceReadFileResult, timeout_s=timeout_s)

    def workspace_list(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceListResult:
        """Send one `leaven/workspace.list` request and return its typed result."""
        return self._typed_request(request, WorkspaceListResult, timeout_s=timeout_s)

    def workspace_snapshot(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceSnapshotResult:
        """Send one `leaven/workspace.snapshot` request and return its typed result."""
        return self._typed_request(request, WorkspaceSnapshotResult, timeout_s=timeout_s)

    def workspace_stat(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceStatResult:
        """Send one `leaven/workspace.stat` request and return its typed result."""
        return self._typed_request(request, WorkspaceStatResult, timeout_s=timeout_s)

    def workspace_digest(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceDigestResult:
        """Send one `leaven/workspace.digest` request and return its typed result."""
        return self._typed_request(request, WorkspaceDigestResult, timeout_s=timeout_s)

    def workspace_git_log(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceGitLogResult:
        """Send one `leaven/workspace.git_log` request and return its typed result."""
        return self._typed_request(request, WorkspaceGitLogResult, timeout_s=timeout_s)

    def workspace_git_diff(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceGitDiffResult:
        """Send one `leaven/workspace.git_diff` request and return its typed result."""
        return self._typed_request(request, WorkspaceGitDiffResult, timeout_s=timeout_s)

    def workspace_git_status(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceGitStatusResult:
        """Send one `leaven/workspace.git_status` request and return its typed result."""
        return self._typed_request(request, WorkspaceGitStatusResult, timeout_s=timeout_s)

    def workspace_capture_artifacts(
        self,
        request: WorkspaceQueryRequest,
        *,
        timeout_s: int = 240,
    ) -> WorkspaceCaptureArtifactsResult:
        """Send one `leaven/workspace.capture_artifacts` request and return its typed result."""
        return self._typed_request(request, WorkspaceCaptureArtifactsResult, timeout_s=timeout_s)

    def case_load(self, request: CaseLoadRequest, *, timeout_s: int = 240) -> CaseLoadResult:
        """Send one case read request and return its typed result."""
        return self._typed_request(request, CaseLoadResult, timeout_s=timeout_s)

    def stage_run(self, request: StageRunRequest, *, timeout_s: int = 240) -> StageRunDispatchResult:
        """Send one runner `leaven/stage.run` request and return its typed result."""
        return self._typed_request(request, StageRunDispatchResult, timeout_s=timeout_s)

    def stage_propose(
        self,
        request: StageRunProposeRequest,
        *,
        timeout_s: int = 240,
    ) -> StageRunDispatchResult:
        """Send one proposer `leaven/stage.run` request and return its typed result."""
        return self._typed_request(request, StageRunDispatchResult, timeout_s=timeout_s)

    def _request_bytes(self, request: SeamJsonRpcRequest, *, timeout_s: int) -> bytes:
        with tempfile.TemporaryDirectory(prefix="leaven-seam-client-") as tmp:
            config_path = Path(tmp) / "seam-config.json"
            config_path.write_text(
                self._config.to_json_bytes().decode(),
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
        request: SeamJsonRpcRequest,
        result_type: type[T],
        *,
        timeout_s: int,
    ) -> T:
        body = self._request_bytes(request, timeout_s=timeout_s)
        try:
            result = decode_method_response(body, request.method)
        except JsonRpcRemoteError as error:
            raise SeamClientError(f"seam returned JSON-RPC error: {error.error}") from error
        except JsonRpcProtocolError as error:
            raise SeamClientError(f"seam returned invalid JSON-RPC: {error}") from error
        if not isinstance(result, result_type):
            raise SeamClientError(f"seam returned {type(result).__name__} for {request.method}")
        return result

    def _run_process(
        self,
        config_path: Path,
        request: SeamJsonRpcRequest,
        timeout_s: int,
    ) -> subprocess.CompletedProcess[str]:
        line = encode_request(
            method=request.method,
            request_id=_request_id(request.request_id),
            params=request.to_params(),
        ).decode()
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


def _request_id(request_id: str) -> JsonRpcId:
    if isinstance(request_id, bool):
        raise SeamClientError("seam request id must be a string, integer, or null")
    if isinstance(request_id, str | int):
        return request_id
    raise SeamClientError("seam request id must be a string, integer, or null")


__all__ = ["SeamClient"]
