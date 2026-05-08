//! Codex app-server runtime configuration.

use leaven_kernel::{Fingerprint, FingerprintBuilder};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodexAppServerConfig {
    pub initialize: CodexAppServerInitializeConfig,
    pub thread: CodexAppServerThreadConfig,
    pub turn: CodexAppServerTurnConfig,
    pub approval_mode: CodexAppServerApprovalMode,
    pub retain_raw_events: CodexRawEventPolicy,
}

impl CodexAppServerConfig {
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder.update("leaven-agent-codex-app-server/v1");
        self.initialize.feed_fingerprint(&mut builder);
        self.thread.feed_fingerprint(&mut builder);
        self.turn.feed_fingerprint(&mut builder);
        builder.update(self.approval_mode.as_wire());
        builder.update(self.retain_raw_events.as_wire());
        builder.finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppServerInitializeConfig {
    pub client_name: String,
    pub client_title: Option<String>,
    pub experimental_api: bool,
    pub opt_out_notification_methods: Option<Vec<String>>,
}

impl Default for CodexAppServerInitializeConfig {
    fn default() -> Self {
        Self {
            client_name: "leaven-agent-codex-app-server".to_owned(),
            client_title: Some("Leaven Codex App-Server Runtime".to_owned()),
            experimental_api: true,
            opt_out_notification_methods: None,
        }
    }
}

impl CodexAppServerInitializeConfig {
    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.update("initialize");
        builder.update(self.client_name.as_bytes());
        feed_option(builder, self.client_title.as_deref());
        builder.update(if self.experimental_api {
            "exp=1"
        } else {
            "exp=0"
        });
        if let Some(methods) = &self.opt_out_notification_methods {
            for method in methods {
                builder.update("\0");
                builder.update(method.as_bytes());
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppServerThreadConfig {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub service_tier: Option<String>,
    pub sandbox: Option<CodexSandboxMode>,
    pub approval_policy: Option<CodexApprovalPolicy>,
    pub approvals_reviewer: Option<CodexApprovalsReviewer>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub ephemeral: bool,
    pub service_name: Option<String>,
}

impl Default for CodexAppServerThreadConfig {
    fn default() -> Self {
        Self {
            model: Some("gpt-5.4-mini".to_owned()),
            model_provider: None,
            service_tier: None,
            sandbox: Some(CodexSandboxMode::WorkspaceWrite),
            approval_policy: Some(CodexApprovalPolicy::Never),
            approvals_reviewer: Some(CodexApprovalsReviewer::User),
            base_instructions: None,
            developer_instructions: None,
            ephemeral: true,
            service_name: None,
        }
    }
}

impl CodexAppServerThreadConfig {
    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.update("thread");
        feed_option(builder, self.model.as_deref());
        feed_option(builder, self.model_provider.as_deref());
        feed_option(builder, self.service_tier.as_deref());
        feed_option(builder, self.sandbox.map(CodexSandboxMode::as_wire));
        feed_option(
            builder,
            self.approval_policy.map(CodexApprovalPolicy::as_wire),
        );
        feed_option(
            builder,
            self.approvals_reviewer.map(CodexApprovalsReviewer::as_wire),
        );
        feed_option(builder, self.base_instructions.as_deref());
        feed_option(builder, self.developer_instructions.as_deref());
        builder.update(if self.ephemeral {
            "ephemeral=1"
        } else {
            "ephemeral=0"
        });
        feed_option(builder, self.service_name.as_deref());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppServerTurnConfig {
    pub model: Option<String>,
    pub service_tier: Option<String>,
    pub effort: Option<CodexReasoningEffort>,
    pub summary: Option<CodexReasoningSummary>,
    pub approval_policy: Option<CodexApprovalPolicy>,
    pub approvals_reviewer: Option<CodexApprovalsReviewer>,
}

impl Default for CodexAppServerTurnConfig {
    fn default() -> Self {
        Self {
            model: None,
            service_tier: None,
            effort: Some(CodexReasoningEffort::Low),
            summary: None,
            approval_policy: None,
            approvals_reviewer: None,
        }
    }
}

impl CodexAppServerTurnConfig {
    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.update("turn");
        feed_option(builder, self.model.as_deref());
        feed_option(builder, self.service_tier.as_deref());
        feed_option(builder, self.effort.map(CodexReasoningEffort::as_wire));
        feed_option(builder, self.summary.map(CodexReasoningSummary::as_wire));
        feed_option(
            builder,
            self.approval_policy.map(CodexApprovalPolicy::as_wire),
        );
        feed_option(
            builder,
            self.approvals_reviewer.map(CodexApprovalsReviewer::as_wire),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CodexAppServerApprovalMode {
    #[default]
    Error,
    Accept,
    Decline,
    Cancel,
}

impl CodexAppServerApprovalMode {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Accept => "accept",
            Self::Decline => "decline",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexApprovalPolicy {
    UnlessTrusted,
    OnFailure,
    OnRequest,
    Never,
}

impl CodexApprovalPolicy {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::UnlessTrusted => "untrusted",
            Self::OnFailure => "on-failure",
            Self::OnRequest => "on-request",
            Self::Never => "never",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexApprovalsReviewer {
    User,
    AutoReview,
}

impl CodexApprovalsReviewer {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::AutoReview => "auto-review",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexSandboxMode {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl CodexReasoningEffort {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

impl CodexReasoningSummary {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Concise => "concise",
            Self::Detailed => "detailed",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CodexRawEventPolicy {
    Drop,
    #[default]
    Retain,
}

impl CodexRawEventPolicy {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Drop => "drop",
            Self::Retain => "retain",
        }
    }

    #[must_use]
    pub const fn retains(self) -> bool {
        matches!(self, Self::Retain)
    }
}

fn feed_option(builder: &mut FingerprintBuilder, value: Option<&str>) {
    match value {
        Some(value) => {
            builder.update("some");
            builder.update(value.as_bytes());
        }
        None => {
            builder.update("none");
        }
    }
}

#[cfg(feature = "app-server")]
pub(crate) mod protocol_map {
    use codex_app_server_protocol::{ApprovalsReviewer, AskForApproval, SandboxMode};
    use codex_protocol::config_types::ReasoningSummary;
    use codex_protocol::openai_models::ReasoningEffort;

    use super::{
        CodexAppServerApprovalMode, CodexApprovalPolicy, CodexApprovalsReviewer,
        CodexReasoningEffort, CodexReasoningSummary, CodexSandboxMode,
    };

    impl From<CodexAppServerApprovalMode> for crate::client::ApprovalMode {
        fn from(value: CodexAppServerApprovalMode) -> Self {
            match value {
                CodexAppServerApprovalMode::Error => Self::Error,
                CodexAppServerApprovalMode::Accept => Self::Accept,
                CodexAppServerApprovalMode::Decline => Self::Decline,
                CodexAppServerApprovalMode::Cancel => Self::Cancel,
            }
        }
    }

    impl From<CodexApprovalPolicy> for AskForApproval {
        fn from(value: CodexApprovalPolicy) -> Self {
            match value {
                CodexApprovalPolicy::UnlessTrusted => Self::UnlessTrusted,
                CodexApprovalPolicy::OnFailure => Self::OnFailure,
                CodexApprovalPolicy::OnRequest => Self::OnRequest,
                CodexApprovalPolicy::Never => Self::Never,
            }
        }
    }

    impl From<CodexApprovalsReviewer> for ApprovalsReviewer {
        fn from(value: CodexApprovalsReviewer) -> Self {
            match value {
                CodexApprovalsReviewer::User => Self::User,
                CodexApprovalsReviewer::AutoReview => Self::AutoReview,
            }
        }
    }

    impl From<CodexSandboxMode> for SandboxMode {
        fn from(value: CodexSandboxMode) -> Self {
            match value {
                CodexSandboxMode::ReadOnly => Self::ReadOnly,
                CodexSandboxMode::WorkspaceWrite => Self::WorkspaceWrite,
                CodexSandboxMode::DangerFullAccess => Self::DangerFullAccess,
            }
        }
    }

    impl From<CodexReasoningEffort> for ReasoningEffort {
        fn from(value: CodexReasoningEffort) -> Self {
            match value {
                CodexReasoningEffort::None => Self::None,
                CodexReasoningEffort::Minimal => Self::Minimal,
                CodexReasoningEffort::Low => Self::Low,
                CodexReasoningEffort::Medium => Self::Medium,
                CodexReasoningEffort::High => Self::High,
                CodexReasoningEffort::XHigh => Self::XHigh,
            }
        }
    }

    impl From<CodexReasoningSummary> for ReasoningSummary {
        fn from(value: CodexReasoningSummary) -> Self {
            match value {
                CodexReasoningSummary::Auto => Self::Auto,
                CodexReasoningSummary::Concise => Self::Concise,
                CodexReasoningSummary::Detailed => Self::Detailed,
                CodexReasoningSummary::None => Self::None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_when_model_changes() {
        let before = CodexAppServerConfig::default().fingerprint();
        let mut config = CodexAppServerConfig::default();
        config.thread.model = Some("gpt-5.5".to_owned());

        assert_ne!(before, config.fingerprint());
    }
}
