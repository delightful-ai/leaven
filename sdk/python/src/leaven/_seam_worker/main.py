"""Command-line entrypoint for the private command-runner worker."""

from __future__ import annotations

import argparse
import asyncio
from pathlib import Path

from .loader import load_stage_from_file
from .protocol import read_request, write_error, write_result
from .runner import run_runner_stage


def main(argv: list[str] | None = None) -> int:
    """Run one stage.run request from stdin and write one JSON-RPC response."""
    args = _parser().parse_args(argv)
    request_id: object = None
    try:
        stage = load_stage_from_file(
            args.module_file,
            stage_id=args.stage_id,
            stage_name=args.stage_name,
        )
        request = read_request()
        request_id = request.get("id")
        result = asyncio.run(run_runner_stage(stage, request["params"], lm_text=args.lm_text))
        write_result(request, result)
        return 0
    except Exception as error:
        write_error(request_id, str(error))
        return 1


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m leaven._seam_worker")
    parser.add_argument("--module-file", required=True, type=Path)
    parser.add_argument("--stage-id", required=True)
    parser.add_argument("--stage-name", required=True)
    parser.add_argument("--lm-text", required=True)
    return parser


__all__ = ["main"]
