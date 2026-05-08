//! Command-backed agent runtime substrate.

pub mod config;
pub mod error;
pub mod parser;
pub mod runtime;

pub use config::{
    CommandAgentConfig, CommandPromptMode, CommandSessionLayout, CommandTemplate,
    CommandTemplateArg,
};
pub use error::CommandAgentError;
pub use parser::{CommandSessionParser, StdoutSessionParser};
pub use runtime::CommandAgentRuntime;

pub mod prelude {
    pub use crate::{
        CommandAgentConfig, CommandAgentError, CommandAgentRuntime, CommandPromptMode,
        CommandSessionLayout, CommandSessionParser, CommandTemplate, CommandTemplateArg,
        StdoutSessionParser,
    };
}
