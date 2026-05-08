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
            ephemeral: false,
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

    #[test]
    fn default_threads_are_materialized_for_evidence_replay() {
        assert!(!CodexAppServerConfig::default().thread.ephemeral);
    }

    #[test]
    fn wire_values_cover_all_public_config_variants() {
        assert_eq!(CodexAppServerApprovalMode::Error.as_wire(), "error");
        assert_eq!(CodexAppServerApprovalMode::Accept.as_wire(), "accept");
        assert_eq!(CodexAppServerApprovalMode::Decline.as_wire(), "decline");
        assert_eq!(CodexAppServerApprovalMode::Cancel.as_wire(), "cancel");
        assert_eq!(CodexApprovalPolicy::UnlessTrusted.as_wire(), "untrusted");
        assert_eq!(CodexApprovalPolicy::OnFailure.as_wire(), "on-failure");
        assert_eq!(CodexApprovalPolicy::OnRequest.as_wire(), "on-request");
        assert_eq!(CodexApprovalPolicy::Never.as_wire(), "never");
        assert_eq!(CodexApprovalsReviewer::User.as_wire(), "user");
        assert_eq!(CodexApprovalsReviewer::AutoReview.as_wire(), "auto-review");
        assert_eq!(CodexSandboxMode::ReadOnly.as_wire(), "read-only");
        assert_eq!(
            CodexSandboxMode::WorkspaceWrite.as_wire(),
            "workspace-write"
        );
        assert_eq!(
            CodexSandboxMode::DangerFullAccess.as_wire(),
            "danger-full-access"
        );
        assert_eq!(CodexReasoningEffort::None.as_wire(), "none");
        assert_eq!(CodexReasoningEffort::Minimal.as_wire(), "minimal");
        assert_eq!(CodexReasoningEffort::Low.as_wire(), "low");
        assert_eq!(CodexReasoningEffort::Medium.as_wire(), "medium");
        assert_eq!(CodexReasoningEffort::High.as_wire(), "high");
        assert_eq!(CodexReasoningEffort::XHigh.as_wire(), "xhigh");
        assert_eq!(CodexReasoningSummary::Auto.as_wire(), "auto");
        assert_eq!(CodexReasoningSummary::Concise.as_wire(), "concise");
        assert_eq!(CodexReasoningSummary::Detailed.as_wire(), "detailed");
        assert_eq!(CodexReasoningSummary::None.as_wire(), "none");
        assert_eq!(CodexRawEventPolicy::Drop.as_wire(), "drop");
        assert_eq!(CodexRawEventPolicy::Retain.as_wire(), "retain");
        assert!(!CodexRawEventPolicy::Drop.retains());
        assert!(CodexRawEventPolicy::Retain.retains());
    }

    #[test]
    fn fingerprint_includes_optional_and_policy_configuration() {
        let mut configured = CodexAppServerConfig::default();
        configured.initialize.client_title = None;
        configured.initialize.experimental_api = false;
        configured.initialize.opt_out_notification_methods =
            Some(vec!["codex/event".to_owned(), "codex/status".to_owned()]);
        configured.thread.model_provider = Some("openai".to_owned());
        configured.thread.service_tier = Some("default".to_owned());
        configured.thread.sandbox = Some(CodexSandboxMode::DangerFullAccess);
        configured.thread.approval_policy = Some(CodexApprovalPolicy::OnRequest);
        configured.thread.approvals_reviewer = Some(CodexApprovalsReviewer::AutoReview);
        configured.thread.base_instructions = Some("base".to_owned());
        configured.thread.developer_instructions = Some("developer".to_owned());
        configured.thread.ephemeral = true;
        configured.thread.service_name = Some("codex".to_owned());
        configured.turn.model = Some("gpt-5.4-mini".to_owned());
        configured.turn.service_tier = Some("flex".to_owned());
        configured.turn.summary = Some(CodexReasoningSummary::Concise);
        configured.turn.approval_policy = Some(CodexApprovalPolicy::OnFailure);
        configured.turn.approvals_reviewer = Some(CodexApprovalsReviewer::User);
        configured.approval_mode = CodexAppServerApprovalMode::Accept;
        configured.retain_raw_events = CodexRawEventPolicy::Drop;

        assert_ne!(
            CodexAppServerConfig::default().fingerprint(),
            configured.fingerprint()
        );
    }

    #[cfg(feature = "app-server")]
    #[test]
    fn protocol_mappings_cover_all_variants() {
        use codex_app_server_protocol::{ApprovalsReviewer, AskForApproval, SandboxMode};
        use codex_protocol::config_types::ReasoningSummary;
        use codex_protocol::openai_models::ReasoningEffort;

        assert!(matches!(
            crate::client::ApprovalMode::from(CodexAppServerApprovalMode::Error),
            crate::client::ApprovalMode::Error
        ));
        assert!(matches!(
            crate::client::ApprovalMode::from(CodexAppServerApprovalMode::Accept),
            crate::client::ApprovalMode::Accept
        ));
        assert!(matches!(
            crate::client::ApprovalMode::from(CodexAppServerApprovalMode::Decline),
            crate::client::ApprovalMode::Decline
        ));
        assert!(matches!(
            crate::client::ApprovalMode::from(CodexAppServerApprovalMode::Cancel),
            crate::client::ApprovalMode::Cancel
        ));
        assert!(matches!(
            AskForApproval::from(CodexApprovalPolicy::UnlessTrusted),
            AskForApproval::UnlessTrusted
        ));
        assert!(matches!(
            AskForApproval::from(CodexApprovalPolicy::OnFailure),
            AskForApproval::OnFailure
        ));
        assert!(matches!(
            AskForApproval::from(CodexApprovalPolicy::OnRequest),
            AskForApproval::OnRequest
        ));
        assert!(matches!(
            AskForApproval::from(CodexApprovalPolicy::Never),
            AskForApproval::Never
        ));
        assert!(matches!(
            ApprovalsReviewer::from(CodexApprovalsReviewer::User),
            ApprovalsReviewer::User
        ));
        assert!(matches!(
            ApprovalsReviewer::from(CodexApprovalsReviewer::AutoReview),
            ApprovalsReviewer::AutoReview
        ));
        assert!(matches!(
            SandboxMode::from(CodexSandboxMode::ReadOnly),
            SandboxMode::ReadOnly
        ));
        assert!(matches!(
            SandboxMode::from(CodexSandboxMode::WorkspaceWrite),
            SandboxMode::WorkspaceWrite
        ));
        assert!(matches!(
            SandboxMode::from(CodexSandboxMode::DangerFullAccess),
            SandboxMode::DangerFullAccess
        ));
        assert!(matches!(
            ReasoningEffort::from(CodexReasoningEffort::None),
            ReasoningEffort::None
        ));
        assert!(matches!(
            ReasoningEffort::from(CodexReasoningEffort::Minimal),
            ReasoningEffort::Minimal
        ));
        assert!(matches!(
            ReasoningEffort::from(CodexReasoningEffort::Low),
            ReasoningEffort::Low
        ));
        assert!(matches!(
            ReasoningEffort::from(CodexReasoningEffort::Medium),
            ReasoningEffort::Medium
        ));
        assert!(matches!(
            ReasoningEffort::from(CodexReasoningEffort::High),
            ReasoningEffort::High
        ));
        assert!(matches!(
            ReasoningEffort::from(CodexReasoningEffort::XHigh),
            ReasoningEffort::XHigh
        ));
        assert!(matches!(
            ReasoningSummary::from(CodexReasoningSummary::Auto),
            ReasoningSummary::Auto
        ));
        assert!(matches!(
            ReasoningSummary::from(CodexReasoningSummary::Concise),
            ReasoningSummary::Concise
        ));
        assert!(matches!(
            ReasoningSummary::from(CodexReasoningSummary::Detailed),
            ReasoningSummary::Detailed
        ));
        assert!(matches!(
            ReasoningSummary::from(CodexReasoningSummary::None),
            ReasoningSummary::None
        ));
    }
}
