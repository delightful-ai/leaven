"""`lv.artifacts.*` — artifact adapters describing the mutable behavior package.

`prompt` / `directory` / `codex_kit` / `skill_bank` / `repo`, plus `unsafe`
(reached as `lv.artifacts.unsafe`, NOT top-level). Each adapter knows its own
identity/fingerprint, projection, typed readback, and mutable-paths contract.

Governing spec: `docs/specs/leaven_python.md` — Artifact adapters / codex_kit.
"""

from __future__ import annotations

from .base import Artifact
from .codex_kit import CodexKitArtifact, codex_kit
from .directory import directory
from .prompt import PromptArtifact, prompt
from .repo import repo
from .skill_bank import skill_bank
from .unsafe import UnsafePath, unsafe

__all__ = [
    "Artifact",
    "CodexKitArtifact",
    "PromptArtifact",
    "UnsafePath",
    "codex_kit",
    "directory",
    "prompt",
    "repo",
    "skill_bank",
    "unsafe",
]
