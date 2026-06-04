"""Tests for `leaven._seam.client` request validation."""

from pathlib import Path

import pytest

from leaven._seam import SeamClient, SeamClientError, SeamExecutionContext, SeamServiceConfig


def test_client_rejects_unknown_method_before_subprocess() -> None:
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint="fp_cap_sha256_test",
                policy_fingerprint="fp_policy_sha256_test",
                base_revision="run_base",
            )
        ),
        leaven_bin=Path("/does/not/exist/leaven"),
        repo_root=Path("/does/not/exist/repo"),
    )

    with pytest.raises(SeamClientError, match="unknown locked Leaven public-seam method"):
        client.agent_run(
            {
                "jsonrpc": "2.0",
                "method": "leaven/human.review",
                "id": "req_1",
                "params": {"schema_version": "leaven.plan.v1"},
            }
        )
