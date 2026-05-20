//! Skill-specific agentic stage helpers.

mod diff;
mod input;
mod layout;
mod materializer;
mod parser;
mod patch_plan;
mod report;

pub use diff::SkillBankDiff;
pub use input::SkillBankProposalInput;
pub use layout::SkillWorkspaceLayout;
pub use materializer::SkillBankMaterializer;
pub use parser::SkillBankWorkspaceProposalParser;
pub use patch_plan::{
    SkillLineRange, SkillPatchEditKind, SkillPatchFileRef, SkillPatchPlan, SkillPatchPlanEdit,
    SkillPatchPlanError, SkillPatchRange, SkillPatchSupport,
};
pub use report::{
    SkillBankChangeReport, SkillDescriptionChange, SkillFileChange, SkillFileChangeKind,
    SkillRenameReport,
};
