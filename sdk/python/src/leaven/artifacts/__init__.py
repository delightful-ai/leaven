"""Built-in artifact types — what gets optimized.

Common artifacts live here as first-class top-level types. Less common or
paper-specific artifacts live under `leaven.x.*` adapter namespaces.

The boundary: if an artifact ships with Leaven core because >1 paper or
common workflow uses it, it's here. Otherwise it lives in an adapter
namespace owned by its semantics.
"""

from .agent_kit import (
    AGENT_KIT_ARTIFACT_SCHEMA,
    AGENT_KIT_ARTIFACT_TYPE,
    AgentKitArtifact,
    AgentKitChange,
    AgentKitSkill,
)
from .directory import DirectoryArtifact, directory
from .prompt import PromptArtifact, PromptTemplateChange
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
    "AGENT_KIT_ARTIFACT_SCHEMA",
    "AGENT_KIT_ARTIFACT_TYPE",
    "AgentKitArtifact",
    "AgentKitChange",
    "AgentKitSkill",
    "DirectoryArtifact",
    "PromptArtifact",
    "PromptTemplateChange",
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
