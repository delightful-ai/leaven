"""One-shot process client for `leaven seam serve --stdio`."""

import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from .config import SeamServiceConfig
from .errors import SeamClientError
from .resolve import resolve_leaven_binary, resolve_repo_root


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

    def request(self, request: dict[str, Any], *, timeout_s: int = 240) -> dict[str, Any]:
        """Send one JSON-RPC request and return the `result` object."""
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
        response = json.loads(process.stdout)
        if "error" in response:
            raise SeamClientError(f"seam returned JSON-RPC error: {response['error']}")
        return response["result"]

    def _run_process(
        self,
        config_path: Path,
        request: dict[str, Any],
        timeout_s: int,
    ) -> subprocess.CompletedProcess[str]:
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
            input=json.dumps(request, sort_keys=True) + "\n",
            text=True,
            capture_output=True,
            timeout=timeout_s,
            check=False,
        )


__all__ = ["SeamClient"]
