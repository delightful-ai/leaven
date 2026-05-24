"""Built-in artifact types — what gets optimized.

Common artifacts live here as first-class top-level types. Less common or
paper-specific artifacts live under `leaven.x.*` adapter namespaces.

The boundary: if an artifact ships with Leaven core because >1 paper or
common workflow uses it, it's here. Otherwise it lives in an adapter
namespace owned by its semantics.
"""

from __future__ import annotations

from .prompt import PromptArtifact
from .skill_bank import SkillBank

__all__ = ["PromptArtifact", "SkillBank"]
