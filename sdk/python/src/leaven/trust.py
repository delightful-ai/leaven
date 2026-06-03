"""Trust profiles — execution policy + capability defaults bundled together.

Pass to `@lv.runner(trust_profile=...)`, `@lv.evaluator(...)`, and
`lv.runtime(trust_profile=...)`. The engine lowers the profile into the
locked capability document; Python authors declare, Rust enforces.

Spec: `docs/specs/leaven_python.md` · seam: public-seam-v1 trust bundles.

V1 is a fixed four-value enum. New profiles require a spec revision.
"""

from __future__ import annotations

from enum import StrEnum


class TrustProfile(StrEnum):
    """Named trust bundles for stages and runtimes."""

    TRUSTED_LOCAL_OPERATOR = "trusted_local_operator"
    """Operator machine — local sandbox backends allowed; broadest effects."""

    MANAGED_SANDBOX = "managed_sandbox"
    """Default for paper repros — engine-managed sandbox, gated capabilities."""

    PACKAGE_SCORER = "package_scorer"
    """Third-party scorer packages — score-only, no arbitrary workspace/agent access."""

    REMOTE_UNTRUSTED = "remote_untrusted"
    """Remote public-seam workers — strictest checks, minimal implicit authority."""


# String-friendly aliases for the convention `trust_profile="managed_sandbox"`.
TRUSTED_LOCAL_OPERATOR = TrustProfile.TRUSTED_LOCAL_OPERATOR
MANAGED_SANDBOX = TrustProfile.MANAGED_SANDBOX
PACKAGE_SCORER = TrustProfile.PACKAGE_SCORER
REMOTE_UNTRUSTED = TrustProfile.REMOTE_UNTRUSTED

__all__ = [
    "MANAGED_SANDBOX",
    "PACKAGE_SCORER",
    "REMOTE_UNTRUSTED",
    "TRUSTED_LOCAL_OPERATOR",
    "TrustProfile",
]
