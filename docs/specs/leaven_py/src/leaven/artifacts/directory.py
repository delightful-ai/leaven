"""DirectoryArtifact — a mutable behavior package backed by a directory.

This adapter is for harness/code packages where the artifact projection is a
tree of files. The rollout stage, not the artifact, decides how to execute it.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class DirectoryArtifact(BaseModel):
    """A directory-shaped artifact seed."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str
    mutable_root: str = "."
    candidate_id: str | None = None


def directory(path: str, *, mutable_root: str = ".") -> DirectoryArtifact:
    """Declare a directory-shaped mutable artifact."""
    return DirectoryArtifact(path=path, mutable_root=mutable_root)


__all__ = ["DirectoryArtifact", "directory"]
