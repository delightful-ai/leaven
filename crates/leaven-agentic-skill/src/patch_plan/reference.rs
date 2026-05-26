use std::collections::BTreeSet;
use std::fmt;

use leaven_artifact_skill::SkillPath;

use super::SkillPatchPlanError;

/// A link to a reference file inside one skill folder.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SkillReferencePath(SkillPath);

impl SkillReferencePath {
    /// Builds a validated `references/*.md` path.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPatchPlanError::InvalidReferencePath`] when the path is
    /// not under `references/` or does not end in `.md`.
    pub fn new(path: SkillPath) -> Result<Self, SkillPatchPlanError> {
        if is_reference_path(&path) {
            Ok(Self(path))
        } else {
            Err(SkillPatchPlanError::InvalidReferencePath { path })
        }
    }

    /// Extracts distinct `references/*.md` links from markdown-ish text.
    #[must_use]
    pub fn extract_from_text(text: &str) -> Vec<Self> {
        let mut links = Vec::new();
        let mut seen = BTreeSet::new();
        let mut offset = 0;
        while let Some(relative_start) = text[offset..].find("references/") {
            let start = offset + relative_start;
            if has_reference_prefix_boundary(text, start).is_none() {
                offset = start + "references/".len();
                continue;
            }
            let tail = &text[start..];
            let end = tail
                .char_indices()
                .find_map(|(index, ch)| {
                    if is_reference_link_char(ch) {
                        None
                    } else {
                        Some(index)
                    }
                })
                .unwrap_or(tail.len());
            let candidate = tail[..end].trim_end_matches('.');
            offset = start + end;
            let Ok(path) = SkillPath::new(candidate) else {
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            if let Ok(link) = Self::new(path) {
                links.push(link);
            }
        }
        links
    }

    /// Returns the skill-relative reference path.
    #[must_use]
    pub const fn path(&self) -> &SkillPath {
        &self.0
    }
}

impl fmt::Display for SkillReferencePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn is_reference_link_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_')
}

fn has_reference_prefix_boundary(text: &str, start: usize) -> Option<()> {
    if text[..start].ends_with("./") {
        let before_dot = text[..start - 2].chars().next_back();
        return (!before_dot.is_some_and(is_reference_link_char)).then_some(());
    }
    let previous = text[..start].chars().next_back();
    (!previous.is_some_and(is_reference_link_char)).then_some(())
}

fn is_reference_path(path: &SkillPath) -> bool {
    let Some(reference_name) = path.as_str().strip_prefix("references/") else {
        return false;
    };
    !reference_name.is_empty()
        && reference_name
            .rsplit_once('.')
            .is_some_and(|(stem, extension)| !stem.is_empty() && extension == "md")
}
