//! Codex provider-family facade.
//!
//! This crate intentionally does not own a single `CodexRuntime`. Codex has
//! multiple operational surfaces; concrete adapters live in narrower crates.

#[cfg(feature = "app-server")]
pub mod app_server {
    pub use leaven_agent_codex_app_server::*;
}

#[cfg(feature = "cli")]
pub mod cli {
    pub use leaven_agent_codex_cli::*;
}
