"""Built-in artifact types — what gets optimized.

Common artifacts live here as first-class top-level types. Less common or
paper-specific artifacts live under `leaven.x.*` adapter namespaces.

The boundary: if an artifact ships with Leaven core because >1 paper or
common workflow uses it, it's here. Otherwise it lives in an adapter
namespace owned by its semantics.
"""

from .directory import DirectoryArtifact, directory
from .prompt import PromptArtifact
from .skill_bank import (
    SkillBank,
    SkillBankAtomicChange,
    SkillBankChange,
    SkillBankChangeFile,
    SkillBankCreateSkillChange,
    SkillBankFolder,
    SkillBankRemoveFileChange,
    SkillBankRemoveSkillChange,
    SkillBankRenameFileChange,
    SkillBankRenameSkillChange,
    SkillBankReplaceSkillChange,
    SkillBankSetExecutableChange,
    SkillBankWriteFileChange,
)

__all__ = [
    "DirectoryArtifact",
    "PromptArtifact",
    "SkillBank",
    "SkillBankAtomicChange",
    "SkillBankChange",
    "SkillBankChangeFile",
    "SkillBankCreateSkillChange",
    "SkillBankFolder",
    "SkillBankRemoveFileChange",
    "SkillBankRemoveSkillChange",
    "SkillBankRenameFileChange",
    "SkillBankRenameSkillChange",
    "SkillBankReplaceSkillChange",
    "SkillBankSetExecutableChange",
    "SkillBankWriteFileChange",
    "directory",
]
