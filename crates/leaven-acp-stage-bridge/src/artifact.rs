//! The prompt artifact optimized by the example-03 bridge loop.

/// A prompt template the optimizer rewrites.
///
/// The runner formats `{question}` (and any other case-input keys) into the
/// template before dispatching the rollout. This is the bridge's local artifact
/// for the prompt/LM/exact-match path; richer artifact semantics belong in the
/// owning artifact crates, not here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptArtifact {
    template: String,
}

impl PromptArtifact {
    /// Creates a prompt artifact from a template string.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
        }
    }

    /// The prompt template.
    pub fn template(&self) -> &str {
        &self.template
    }

    /// Renders the template against one case input map.
    ///
    /// Each `{key}` placeholder is replaced by the matching case-input value.
    /// Unmatched placeholders are left intact so a malformed template is visible
    /// in the dispatched prompt rather than silently dropped.
    pub fn render(&self, case_input: &[(String, String)]) -> String {
        let mut rendered = self.template.clone();
        for (key, value) in case_input {
            rendered = rendered.replace(&format!("{{{key}}}"), value);
        }
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_substitutes_case_input_placeholders() {
        let artifact = PromptArtifact::new("Expression: {question}\nAnswer:");
        let rendered = artifact.render(&[("question".to_owned(), "2 + 3".to_owned())]);
        assert_eq!(rendered, "Expression: 2 + 3\nAnswer:");
    }

    #[test]
    fn render_leaves_unmatched_placeholders_visible() {
        let artifact = PromptArtifact::new("{missing} stays");
        assert_eq!(artifact.render(&[]), "{missing} stays");
    }
}
