use serde::{Deserialize, Serialize};

/// Role attached to a text message in a canonical LM conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System/developer instruction text.
    System,
    /// User-authored turn text.
    User,
    /// Assistant-authored turn text.
    Assistant,
}

/// One text message in a canonical LM conversation.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Message {
    role: Role,
    content: String,
}

impl Message {
    /// Builds a message with an explicit role.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Builds a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Builds a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Builds an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Returns the message role.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the message text.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
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
