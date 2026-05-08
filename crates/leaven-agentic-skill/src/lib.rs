//! Skill-specific agentic stage helpers.

mod diff;
mod input;
mod layout;
mod materializer;
mod parser;
mod report;

pub use diff::SkillBankDiff;
pub use input::SkillBankProposalInput;
pub use layout::SkillWorkspaceLayout;
pub use materializer::SkillBankMaterializer;
pub use parser::SkillBankWorkspaceProposalParser;
pub use report::{
    SkillBankChangeReport, SkillDescriptionChange, SkillFileChange, SkillFileChangeKind,
    SkillRenameReport,
};
