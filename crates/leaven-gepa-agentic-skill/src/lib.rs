//! GEPA agentic reflection adapter for skill-bank artifacts.

mod input;
mod materializer;
mod parser;
mod reflector;
mod renderer;

pub use input::SkillBankGepaReflectionInput;
pub use materializer::SkillBankGepaReflectionMaterializer;
pub use parser::SkillBankGepaReflectionParser;
pub use reflector::GepaSkillBankAgenticReflector;
pub use renderer::GepaSkillBankReflectionRenderer;

pub mod prelude {
    pub use crate::{
        GepaSkillBankAgenticReflector, GepaSkillBankReflectionRenderer,
        SkillBankGepaReflectionInput, SkillBankGepaReflectionMaterializer,
        SkillBankGepaReflectionParser,
    };
}
