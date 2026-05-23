use serde::{Deserialize, Serialize};

/// Role attached to a text message in a canonical LM conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System/developer instruction text.
    System,
    /// Developer instruction text.
    Developer,
    /// User-authored turn text.
    User,
    /// Assistant-authored turn text.
    Assistant,
    /// Tool-authored result text.
    Tool,
}

/// One content part in a canonical LM conversation message.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageContentPart {
    /// Plain text content.
    Text {
        /// Text body.
        text: String,
    },
    /// Result returned from a prior tool call.
    ToolResult {
        /// Provider/model tool-call identifier being answered.
        tool_call_id: String,
        /// Tool result body.
        content: String,
    },
}

/// One text message in a canonical LM conversation.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Message {
    role: Role,
    content: Vec<MessageContentPart>,
    tool_call_id: Option<String>,
    name: Option<String>,
}

impl Message {
    /// Builds a message with an explicit role.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![MessageContentPart::Text {
                text: content.into(),
            }],
            tool_call_id: None,
            name: None,
        }
    }

    /// Builds a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Builds a developer message.
    pub fn developer(content: impl Into<String>) -> Self {
        Self::new(Role::Developer, content)
    }

    /// Builds a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Builds an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Builds a tool-result message.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        let tool_call_id = tool_call_id.into();
        Self {
            role: Role::Tool,
            content: vec![MessageContentPart::ToolResult {
                tool_call_id: tool_call_id.clone(),
                content: content.into(),
            }],
            tool_call_id: Some(tool_call_id),
            name: None,
        }
    }

    /// Attaches a provider/model tool-call identifier to this message.
    #[must_use]
    pub fn with_tool_call_id(mut self, tool_call_id: impl Into<String>) -> Self {
        self.tool_call_id = Some(tool_call_id.into());
        self
    }

    /// Attaches a provider/model-visible message name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Returns the message role.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the first text or tool-result content part, or an empty string
    /// for empty extension-only messages.
    #[must_use]
    pub fn content(&self) -> &str {
        self.content.first().map_or("", |part| match part {
            MessageContentPart::Text { text } => text.as_str(),
            MessageContentPart::ToolResult { content, .. } => content.as_str(),
        })
    }

    /// Returns the first text part when present.
    #[must_use]
    pub fn text_content(&self) -> Option<&str> {
        self.content.iter().find_map(|part| match part {
            MessageContentPart::Text { text } => Some(text.as_str()),
            MessageContentPart::ToolResult { .. } => None,
        })
    }

    /// Returns all structured content parts.
    #[must_use]
    pub fn content_parts(&self) -> &[MessageContentPart] {
        &self.content
    }

    /// Returns the attached tool-call id.
    #[must_use]
    pub fn tool_call_id(&self) -> Option<&str> {
        self.tool_call_id.as_deref()
    }

    /// Returns the attached message name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Ordered canonical text conversation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Messages(Vec<Message>);

impl Messages {
    /// Creates an empty message list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a single-user-turn message list.
    pub fn from_user(content: impl Into<String>) -> Self {
        Self::new().with_user(content)
    }

    /// Appends a message and returns `self`.
    #[must_use]
    pub fn with_message(mut self, message: Message) -> Self {
        self.0.push(message);
        self
    }

    /// Appends a system message and returns `self`.
    #[must_use]
    pub fn with_system(self, content: impl Into<String>) -> Self {
        self.with_message(Message::system(content))
    }

    /// Appends a developer message and returns `self`.
    #[must_use]
    pub fn with_developer(self, content: impl Into<String>) -> Self {
        self.with_message(Message::developer(content))
    }

    /// Appends a user message and returns `self`.
    #[must_use]
    pub fn with_user(self, content: impl Into<String>) -> Self {
        self.with_message(Message::user(content))
    }

    /// Appends an assistant message and returns `self`.
    #[must_use]
    pub fn with_assistant(self, content: impl Into<String>) -> Self {
        self.with_message(Message::assistant(content))
    }

    /// Appends a tool-result message and returns `self`.
    #[must_use]
    pub fn with_tool_result(
        self,
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.with_message(Message::tool_result(tool_call_id, content))
    }

    /// Appends a message in place.
    pub fn push(&mut self, message: Message) {
        self.0.push(message);
    }

    /// Iterates over messages in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.0.iter()
    }

    /// Returns messages as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Message] {
        &self.0
    }

    /// Returns the number of messages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true when there are no messages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the suffix beginning at `start`, or an empty slice if `start`
    /// is beyond the message length.
    #[must_use]
    pub fn suffix_from(&self, start: usize) -> &[Message] {
        self.0.get(start..).unwrap_or(&[])
    }
}
