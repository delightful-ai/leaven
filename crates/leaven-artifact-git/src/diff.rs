#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitDiff {
    summary: GitDiffSummary,
}

impl GitDiff {
    #[must_use]
    pub const fn new(summary: GitDiffSummary) -> Self {
        Self { summary }
    }

    #[must_use]
    pub const fn summary(&self) -> &GitDiffSummary {
        &self.summary
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitDiffSummary {
    pub files_changed: u32,
    pub refs_changed: u32,
}
