"""Example 10 — live-gated Python client for `leaven/agent.run`.

This is a proof of the public seam substrate, not the ergonomic `cx.agent.run`
SDK surface. It spawns `leaven seam serve --stdio --config`, sends one locked
Plan IR `leaven/agent.run` request, and checks that the Rust child materializes
a workspace, runs the configured Codex CLI adapter, and returns an
`agent_session` with receipts and transcript refs.

Run only when live Codex spend is intended:

    LEAVEN_LIVE_CODEX=1 uv run python examples/10_live_codex_seam.py

Set `LEAVEN_BIN` or `LEAVEN_CODEX_BIN` to override binary discovery.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any


def main() -> None:
    if os.environ.get("LEAVEN_LIVE_CODEX") != "1":
        print("skipped: set LEAVEN_LIVE_CODEX=1 to run the live Codex seam proof")
        return

    leaven = _resolve_leaven_binary()
    codex = _resolve_codex_binary()
    root = _resolve_repo_root()

    with tempfile.TemporaryDirectory(prefix="leaven-py-live-codex-") as tmp:
        config_path = Path(tmp) / "seam-config.json"
        config_path.write_text(json.dumps(_service_config(codex), sort_keys=True), encoding="utf-8")

        request = _agent_run_request()
        process = subprocess.run(
            [
                str(leaven),
                "seam",
                "serve",
                "--stdio",
                "--root",
                str(root),
                "--config",
                str(config_path),
            ],
            input=json.dumps(request, sort_keys=True) + "\n",
            text=True,
            capture_output=True,
            timeout=240,
            check=False,
        )

    if process.returncode != 0:
        raise RuntimeError(
            "leaven seam serve failed\n"
            f"status: {process.returncode}\nstdout:\n{process.stdout}\nstderr:\n{process.stderr}"
        )

    response = json.loads(process.stdout)
    if "error" in response:
        raise RuntimeError(f"seam returned JSON-RPC error: {response['error']}")

    result = response["result"]
    primary = result["primary"]
    receipts = result["receipts"]
    assert primary["kind"] == "agent_session"
    assert primary["status"] == "completed"
    assert [receipt["call_kind"] for receipt in receipts] == ["workspace_materialize", "agent_run"]
    assert primary["transcript_ref"]["bytes"] > 100

    codex_command = primary["commands"][1]["argv"]
    print("agent status:     ", primary["status"])
    print("codex model:      ", codex_command[codex_command.index("--model") + 1])
    print("transcript bytes: ", primary["transcript_ref"]["bytes"])
    print("receipts:         ", ", ".join(receipt["receipt"] for receipt in receipts))


def _resolve_repo_root() -> Path:
    marker = Path("crates/leaven/tests/topology_contract.rs")
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").is_file() and (parent / marker).is_file():
            return parent
    raise RuntimeError("could not locate Leaven repo root")


def _resolve_leaven_binary() -> Path:
    override = os.environ.get("LEAVEN_BIN")
    if override:
        return _existing_file(override, "LEAVEN_BIN")
    root = _resolve_repo_root()
    for profile in ("debug", "release"):
        candidate = root / "target" / profile / "leaven"
        if candidate.is_file():
            return candidate
    raise RuntimeError("build the CLI first with `cargo build -p leaven-cli`, or set LEAVEN_BIN")


def _resolve_codex_binary() -> str:
    override = os.environ.get("LEAVEN_CODEX_BIN")
    if override:
        return str(_existing_file(override, "LEAVEN_CODEX_BIN"))
    found = shutil.which("codex")
    if found:
        return found
    raise RuntimeError("could not find `codex`; set LEAVEN_CODEX_BIN")


def _existing_file(path: str, env_name: str) -> Path:
    candidate = Path(path)
    if not candidate.is_file():
        raise RuntimeError(f"{env_name}={path!r} is not a file")
    return candidate


def _service_config(codex_bin: str) -> dict[str, Any]:
    return {
        "context": {
            "capability_fingerprint": "fp_cap_sha256_py_live_codex",
            "policy_fingerprint": "fp_policy_sha256_py_live_codex",
            "base_revision": "rev_py_live_codex_base",
            "started_at": "2026-06-02T00:00:00Z",
            "completed_at": "2026-06-02T00:00:01Z",
        },
        "capability": {
            "schema_version": "leaven.capability.v1",
            "jti": "jti_py_live_codex",
            "capability_fingerprint": "fp_cap_sha256_py_live_codex",
            "policy_fingerprint": "fp_policy_sha256_py_live_codex",
            "subject_fingerprint": "fp_subject_sha256_py_live_codex",
            "issuer": {"kind": "run_engine", "id": "engine_local"},
            "subject": {
                "kind": "stage_call",
                "run": "run_py_live_codex",
                "stage_call_id": "sc_py_live_codex",
                "role": "scorer",
            },
            "audience": ["leaven.acp.worker"],
            "issued_at": "2026-06-02T00:00:00Z",
            "expires_at": "2026-06-02T00:20:00Z",
            "expiry_behavior": "drain_inflight_no_new_ops",
            "token_binding": {"kind": "opaque_lookup", "token_id": "ltok_py_live_codex"},
            "revocation": {"mode": "issuer_epoch", "revocation_epoch": 9, "check": "on_every_request"},
            "renewal": {"mode": "renew_before_expiry", "max_extensions": 0, "max_total_lifetime_s": 1200},
            "grants": [
                {
                    "action": "workspace.materialize",
                    "resource": {"candidate_ids": ["cand_pylivecodex"]},
                    "constraints": {"workspace_ops": ["materialize"]},
                },
                {
                    "action": "agent.run",
                    "resource": {"workspace_ids": ["ws_pylivecodex_materialized"]},
                    "constraints": {"allowed_input_classes": ["public"]},
                    "limits": {"timeout_s": 180, "max_usd_micro": 5_000_000},
                },
            ],
            "budgets": {},
            "execution_policy": {
                "profile": "managed_sandbox",
                "network": "leaven_endpoint_only",
                "subprocess": "deny_except_sandbox_exec",
                "filesystem": "workspace_handles_only",
                "byo_effects": "forbidden",
            },
            "delegation": {
                "may_delegate": False,
                "max_depth": 0,
                "must_attenuate": True,
                "allowed_actions": [],
            },
        },
        "workspace": {
            "seed_files": {
                "README.md": "Live Codex proof workspace for Leaven Python public seam.\n"
            }
        },
        "agent": {
            "kind": "codex_cli",
            "codex_bin": codex_bin,
            "model": "gpt-5.4-mini",
            "timeout_s": 180,
            "codex_home": None,
            "bypass_approvals_and_sandbox": False,
        },
        "lm": {"kind": "mock", "responses": [{"text": "unused", "input_tokens": 1, "output_tokens": 1}]},
    }


def _agent_run_request() -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": "py-live-codex-agent-1",
        "method": "leaven/agent.run",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "planpylivecodex001",
            "consistency": {"kind": "latest_at_start"},
            "mode": {"kind": "execute"},
            "ops": [
                {
                    "kind": "call",
                    "name": "workspace",
                    "idempotency_key": "py-live-codex-0001",
                    "call": {
                        "kind": "workspace_materialize",
                        "candidate": "cand_pylivecodex",
                        "surface": "program",
                        "mode": "copy_on_write",
                        "lifetime": "manual_release",
                    },
                },
                {
                    "kind": "call",
                    "name": "completion",
                    "deps": ["workspace"],
                    "idempotency_key": "py-live-codex-0002",
                    "call": {
                        "kind": "agent_run",
                        "runtime": "codex-cli",
                        "workspace": "ws_pylivecodex_materialized",
                        "instructions": {
                            "system": (
                                "You are running inside a temporary Leaven proof workspace. "
                                "Do not edit files or run tools unless absolutely necessary."
                            ),
                            "task": (
                                "Return exactly this sentence as the final answer: "
                                "Leaven Python live Codex seam proof succeeded."
                            ),
                        },
                        "tool_policy": {"allow_shell": False},
                        "output": {"kind": "final_message", "max_bytes": 512},
                        "limits": {"timeout_s": 180, "max_turns": 1, "max_usd_micro": 5_000_000},
                        "input_classes": ["public"],
                    },
                },
            ],
            "return": ["workspace", "completion"],
            "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"},
        },
    }


if __name__ == "__main__":
    main()
