//! GEPA agentic reflection adapter for skill-bank artifacts.

mod input;
mod reflector;
mod skill_reflector;

pub use input::SkillBankReflectionInput;
pub use reflector::GepaSkillBankAgenticReflector;
pub use skill_reflector::{SkillBankReflectionError, SkillBankReflector};

pub mod prelude {
    pub use crate::{
        GepaSkillBankAgenticReflector, SkillBankReflectionError, SkillBankReflectionInput,
        SkillBankReflector,
    };
}
