//! Token accounting over validated skill artifacts.

use std::collections::BTreeMap;

use crate::{SkillBank, SkillFile, SkillName, SkillPath};

/// Tokenizer adapter used to measure skill text.
///
/// Implementations own tokenizer-specific behavior, such as `cl100k_base`.
/// This crate owns only the skill-structure accounting law: descriptions and
/// `SKILL.md` bodies are always-loaded context, while direct `references/*.md`
/// files are optional progressive-disclosure context.
pub trait SkillTokenizer {
    /// Stable tokenizer identifier recorded in token reports.
    fn tokenizer_id(&self) -> &str;

    /// Counts tokens in one UTF-8 text payload.
    fn count_tokens(&self, text: &str) -> u64;
}

/// Token profile for a whole [`SkillBank`].
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillTokenProfile {
    tokenizer_id: String,
    skills: BTreeMap<SkillName, SkillTokenBreakdown>,
    total_always_loaded_tokens: u64,
    total_reference_tokens: u64,
    total_context_tokens: u64,
}

impl SkillTokenProfile {
    /// Measures description, body, and direct reference-module token counts.
    ///
    /// # Errors
    ///
    /// Returns [`SkillTokenProfileError`] when a direct `references/*.md` module
    /// is not UTF-8 or token totals overflow `u64`.
    pub fn measure(
        bank: &SkillBank,
        tokenizer: &(impl SkillTokenizer + ?Sized),
    ) -> Result<Self, SkillTokenProfileError> {
        let mut skills = BTreeMap::new();
        let mut total_always_loaded_tokens = 0_u64;
        let mut total_reference_tokens = 0_u64;
        let mut total_context_tokens = 0_u64;

        for (name, folder) in bank.folders() {
            let description_tokens = tokenizer.count_tokens(folder.manifest().description.as_str());
            let body_tokens = tokenizer.count_tokens(folder.body().as_str());
            let mut reference_tokens = BTreeMap::new();
            for (path, file) in folder.entries() {
                if is_direct_reference_markdown(path) {
                    let text = reference_text(name, path, file)?;
                    reference_tokens.insert(path.clone(), tokenizer.count_tokens(text));
                }
            }
            let breakdown =
                SkillTokenBreakdown::new(name, description_tokens, body_tokens, reference_tokens)?;
            total_always_loaded_tokens = checked_add(
                name,
                "bank always-loaded tokens",
                total_always_loaded_tokens,
                breakdown.always_loaded_tokens,
            )?;
            total_reference_tokens = checked_add(
                name,
                "bank reference tokens",
                total_reference_tokens,
                breakdown.reference_tokens_total,
            )?;
            total_context_tokens = checked_add(
                name,
                "bank context tokens",
                total_context_tokens,
                breakdown.context_tokens,
            )?;
            skills.insert(name.clone(), breakdown);
        }

        Ok(Self {
            tokenizer_id: tokenizer.tokenizer_id().to_owned(),
            skills,
            total_always_loaded_tokens,
            total_reference_tokens,
            total_context_tokens,
        })
    }

    /// Tokenizer identifier used for this profile.
    #[must_use]
    pub fn tokenizer_id(&self) -> &str {
        &self.tokenizer_id
    }

    /// Per-skill token breakdowns in stable skill-name order.
    #[must_use]
    pub fn skills(&self) -> &BTreeMap<SkillName, SkillTokenBreakdown> {
        &self.skills
    }

    /// Token breakdown for one skill.
    #[must_use]
    pub fn skill(&self, name: &SkillName) -> Option<&SkillTokenBreakdown> {
        self.skills.get(name)
    }

    /// Total description + `SKILL.md` body tokens across all skills.
    #[must_use]
    pub const fn total_always_loaded_tokens(&self) -> u64 {
        self.total_always_loaded_tokens
    }

    /// Total direct reference-module tokens across all skills.
    #[must_use]
    pub const fn total_reference_tokens(&self) -> u64 {
        self.total_reference_tokens
    }

    /// Total always-loaded + direct reference-module context tokens.
    #[must_use]
    pub const fn total_context_tokens(&self) -> u64 {
        self.total_context_tokens
    }

    /// Compares this profile with a later profile measured by the same tokenizer.
    ///
    /// # Errors
    ///
    /// Returns [`SkillTokenProfileError::TokenizerMismatch`] when the two
    /// profiles were measured with different tokenizer identifiers.
    pub fn compare(
        &self,
        after: &Self,
    ) -> Result<SkillTokenProfileComparison, SkillTokenProfileError> {
        if self.tokenizer_id != after.tokenizer_id {
            return Err(SkillTokenProfileError::TokenizerMismatch {
                before: self.tokenizer_id.clone(),
                after: after.tokenizer_id.clone(),
            });
        }
        Ok(SkillTokenProfileComparison {
            before_always_loaded: self.total_always_loaded_tokens,
            after_always_loaded: after.total_always_loaded_tokens,
            before_context: self.total_context_tokens,
            after_context: after.total_context_tokens,
        })
    }
}

/// Token breakdown for one skill folder.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillTokenBreakdown {
    description_tokens: u64,
    body_tokens: u64,
    reference_tokens: BTreeMap<SkillPath, u64>,
    always_loaded_tokens: u64,
    reference_tokens_total: u64,
    context_tokens: u64,
}

impl SkillTokenBreakdown {
    fn new(
        name: &SkillName,
        description_tokens: u64,
        body_tokens: u64,
        reference_tokens: BTreeMap<SkillPath, u64>,
    ) -> Result<Self, SkillTokenProfileError> {
        let always_loaded_tokens = checked_add(
            name,
            "always-loaded tokens",
            description_tokens,
            body_tokens,
        )?;
        let mut reference_tokens_total = 0_u64;
        for tokens in reference_tokens.values() {
            reference_tokens_total =
                checked_add(name, "reference tokens", reference_tokens_total, *tokens)?;
        }
        let context_tokens = checked_add(
            name,
            "context tokens",
            always_loaded_tokens,
            reference_tokens_total,
        )?;
        Ok(Self {
            description_tokens,
            body_tokens,
            reference_tokens,
            always_loaded_tokens,
            reference_tokens_total,
            context_tokens,
        })
    }

    /// Frontmatter description tokens.
    #[must_use]
    pub const fn description_tokens(&self) -> u64 {
        self.description_tokens
    }

    /// `SKILL.md` markdown body tokens.
    #[must_use]
    pub const fn body_tokens(&self) -> u64 {
        self.body_tokens
    }

    /// Direct reference-module token counts in stable path order.
    #[must_use]
    pub fn reference_tokens(&self) -> &BTreeMap<SkillPath, u64> {
        &self.reference_tokens
    }

    /// Description + body tokens.
    #[must_use]
    pub const fn always_loaded_tokens(&self) -> u64 {
        self.always_loaded_tokens
    }

    /// Sum of direct reference-module tokens.
    #[must_use]
    pub const fn reference_tokens_total(&self) -> u64 {
        self.reference_tokens_total
    }

    /// Description + body + direct reference-module tokens.
    #[must_use]
    pub const fn context_tokens(&self) -> u64 {
        self.context_tokens
    }
}

/// Before/after token comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillTokenProfileComparison {
    before_always_loaded: u64,
    after_always_loaded: u64,
    before_context: u64,
    after_context: u64,
}

impl SkillTokenProfileComparison {
    /// Always-loaded tokens in the earlier profile.
    #[must_use]
    pub const fn before_always_loaded_tokens(&self) -> u64 {
        self.before_always_loaded
    }

    /// Always-loaded tokens in the later profile.
    #[must_use]
    pub const fn after_always_loaded_tokens(&self) -> u64 {
        self.after_always_loaded
    }

    /// Later minus earlier always-loaded token count.
    #[must_use]
    pub const fn always_loaded_token_change(&self) -> i128 {
        self.after_always_loaded as i128 - self.before_always_loaded as i128
    }

    /// Total context tokens in the earlier profile.
    #[must_use]
    pub const fn before_context_tokens(&self) -> u64 {
        self.before_context
    }

    /// Total context tokens in the later profile.
    #[must_use]
    pub const fn after_context_tokens(&self) -> u64 {
        self.after_context
    }

    /// Later minus earlier total context token count.
    #[must_use]
    pub const fn context_token_change(&self) -> i128 {
        self.after_context as i128 - self.before_context as i128
    }
}

/// Token profiling failure.
#[derive(Debug, thiserror::Error)]
pub enum SkillTokenProfileError {
    /// A direct markdown reference module is not valid UTF-8.
    #[error("reference file {path} in skill {skill} is not valid UTF-8")]
    NonUtf8Reference {
        /// Skill containing the reference.
        skill: SkillName,
        /// Invalid reference path.
        path: SkillPath,
        /// UTF-8 failure.
        #[source]
        source: std::str::Utf8Error,
    },
    /// Summing token counts overflowed.
    #[error("token count overflow while summing {axis} for skill {skill}")]
    TokenCountOverflow {
        /// Skill whose profile was being summed.
        skill: SkillName,
        /// Total axis being summed.
        axis: &'static str,
    },
    /// Two profiles were measured with different tokenizer identifiers.
    #[error("cannot compare token profiles measured with {before} and {after}")]
    TokenizerMismatch {
        /// Tokenizer identifier for the earlier profile.
        before: String,
        /// Tokenizer identifier for the later profile.
        after: String,
    },
}

fn reference_text<'a>(
    skill: &SkillName,
    path: &SkillPath,
    file: &'a SkillFile,
) -> Result<&'a str, SkillTokenProfileError> {
    std::str::from_utf8(file.bytes()).map_err(|source| SkillTokenProfileError::NonUtf8Reference {
        skill: skill.clone(),
        path: path.clone(),
        source,
    })
}

fn is_direct_reference_markdown(path: &SkillPath) -> bool {
    let Some(rest) = path.as_str().strip_prefix("references/") else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".md") else {
        return false;
    };
    !stem.is_empty() && !stem.contains('/')
}

fn checked_add(
    skill: &SkillName,
    axis: &'static str,
    lhs: u64,
    rhs: u64,
) -> Result<u64, SkillTokenProfileError> {
    lhs.checked_add(rhs)
        .ok_or_else(|| SkillTokenProfileError::TokenCountOverflow {
            skill: skill.clone(),
            axis,
        })
}
