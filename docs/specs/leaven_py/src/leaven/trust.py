"""Trust profiles — fixed execution-policy + capability-default enum.

The `trust_profile=` declaration bundles execution policy and capability
defaults from a fixed enum. The engine enforces; the Python surface only
declares.

Governing spec: `docs/specs/leaven_python.md` — Runtime / "trust profile
bundles execution policy + capability defaults from a fixed enum".
"""

from __future__ import annotations

from enum import StrEnum

__all__ = ["TrustProfile", "trust"]


class TrustProfile(StrEnum):
    """The fixed V1 trust-profile enum.

    Spec: `trusted_local_operator`, `managed_sandbox`, `package_scorer`,
    `remote_untrusted`.
    """

    trusted_local_operator = "trusted_local_operator"
    managed_sandbox = "managed_sandbox"
    package_scorer = "package_scorer"
    remote_untrusted = "remote_untrusted"


class _Trust:
    """Ergonomic `lv.trust.*` namespace exposing the profile values.

    `lv.trust` is the top-level product noun; `TrustProfile` is the enum type
    behind `trust_profile=` strings/values and is NOT in the top-level
    allow-list.
    """

    trusted_local_operator: TrustProfile = TrustProfile.trusted_local_operator
    managed_sandbox: TrustProfile = TrustProfile.managed_sandbox
    package_scorer: TrustProfile = TrustProfile.package_scorer
    remote_untrusted: TrustProfile = TrustProfile.remote_untrusted


trust = _Trust()
