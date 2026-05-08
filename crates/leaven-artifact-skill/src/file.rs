//! Files inside skill folders.

/// File permissions relevant to agent skill materialization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillFilePermissions {
    /// Whether the file should be executable when materialized.
    pub executable: bool,
}

/// One file inside a skill folder.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillFile {
    bytes: Vec<u8>,
    permissions: SkillFilePermissions,
}

impl SkillFile {
    /// Constructs a non-executable file.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            permissions: SkillFilePermissions::default(),
        }
    }

    /// Constructs a file with explicit permissions.
    pub fn with_permissions(bytes: impl Into<Vec<u8>>, permissions: SkillFilePermissions) -> Self {
        Self {
            bytes: bytes.into(),
            permissions,
        }
    }

    /// Constructs a UTF-8 file from a string.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(text.into().into_bytes())
    }

    /// Returns the file bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the file and returns its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Returns the file permissions.
    pub fn permissions(&self) -> SkillFilePermissions {
        self.permissions
    }

    pub(crate) fn set_executable(&mut self, executable: bool) {
        self.permissions.executable = executable;
    }
}
