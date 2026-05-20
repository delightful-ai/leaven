//! Skill-specific agentic stage helpers.

mod diff;
mod input;
mod layout;
mod materializer;
mod merge_tree;
mod parser;
mod patch_plan;
mod report;

pub use diff::SkillBankDiff;
pub use input::SkillBankProposalInput;
pub use layout::SkillWorkspaceLayout;
pub use materializer::SkillBankMaterializer;
pub use merge_tree::{
    SkillPatchMergeBatch, SkillPatchMergeDecision, SkillPatchMergeInput, SkillPatchMergeLevel,
    SkillPatchMergeTree, SkillPatchMergeTreeError, SkillPatchPlanId, SkillPatchPlanRecord,
};
pub use parser::SkillBankWorkspaceProposalParser;
pub use patch_plan::{
    SkillLineRange, SkillPatchEditKind, SkillPatchFileRef, SkillPatchPlan, SkillPatchPlanEdit,
    SkillPatchPlanError, SkillPatchRange, SkillPatchSupport, SkillReferencePath,
};
pub use report::{
    SkillBankChangeReport, SkillDescriptionChange, SkillFileChange, SkillFileChangeKind,
    SkillRenameReport,
};
