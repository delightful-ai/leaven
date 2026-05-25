//! `OpenAI` Responses API adapter for Leaven's provider-neutral LM trait.
//!
//! This crate lowers [`leaven_lm::LmRequest`] values into `OpenAI` wire requests,
//! parses `OpenAI` Responses API payloads back into [`leaven_lm::LmResponse`],
//! and owns local `OpenAI` transport policy such as retry, timeout, and
//! concurrency throttling.

mod client;
mod config;

pub use client::OpenAiLm;
pub use config::{OpenAiConfig, OpenAiRetryPolicy, OpenAiThrottlePolicy};
