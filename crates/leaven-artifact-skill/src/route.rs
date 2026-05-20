//! Explicit routing overlays over validated skill banks.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::{
    SkillBank, SkillCard, SkillName, SkillRouteKeyError, SkillRoutePoolError,
    SkillRouteRegistryError,
};

/// A caller-defined skill pool label used for retrieval-time membership.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SkillRoutePool(String);

impl SkillRoutePool {
    /// Validates and constructs a route pool label.
    ///
    /// # Errors
    ///
    /// Returns [`SkillRoutePoolError`] when the label is empty or contains NUL.
    pub fn new(value: impl Into<String>) -> Result<Self, SkillRoutePoolError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(SkillRoutePoolError::Empty);
        }
        if trimmed.contains('\0') {
            return Err(SkillRoutePoolError::Nul);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated pool label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillRoutePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SkillRoutePool {
    type Err = SkillRoutePoolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SkillRoutePool {
    type Error = SkillRoutePoolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A caller-defined retrieval key for a skill route entry.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SkillRouteKey(String);

impl SkillRouteKey {
    /// Validates and constructs a route key.
    ///
    /// # Errors
    ///
    /// Returns [`SkillRouteKeyError`] when the key is empty or contains NUL.
    pub fn new(value: impl Into<String>) -> Result<Self, SkillRouteKeyError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(SkillRouteKeyError::Empty);
        }
        if value.contains('\0') {
            return Err(SkillRouteKeyError::Nul);
        }
        Ok(Self(value))
    }

    /// Returns the validated route key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SkillRouteKey {
    type Err = SkillRouteKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SkillRouteKey {
    type Error = SkillRouteKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Caller-supplied route membership for one skill in a bank.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillRouteSpec {
    skill: SkillName,
    pool: SkillRoutePool,
    route_key: SkillRouteKey,
}

impl SkillRouteSpec {
    /// Builds a route spec for one validated skill name.
    pub fn new(skill: SkillName, pool: SkillRoutePool, route_key: SkillRouteKey) -> Self {
        Self {
            skill,
            pool,
            route_key,
        }
    }

    /// Returns the target skill name.
    pub fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Returns the caller-defined pool.
    pub fn pool(&self) -> &SkillRoutePool {
        &self.pool
    }

    /// Returns the caller-defined retrieval key.
    pub fn route_key(&self) -> &SkillRouteKey {
        &self.route_key
    }
}

/// A validated skill card plus retrieval pool/key membership.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillRouteEntry {
    card: SkillCard,
    pool: SkillRoutePool,
    route_key: SkillRouteKey,
}

impl SkillRouteEntry {
    /// Builds one route entry from a projected skill card.
    pub fn new(card: SkillCard, pool: SkillRoutePool, route_key: SkillRouteKey) -> Self {
        Self {
            card,
            pool,
            route_key,
        }
    }

    /// Returns the routed skill name.
    pub fn skill(&self) -> &SkillName {
        self.card.name()
    }

    /// Returns the projected skill card.
    pub fn card(&self) -> &SkillCard {
        &self.card
    }

    /// Returns the caller-defined pool.
    pub fn pool(&self) -> &SkillRoutePool {
        &self.pool
    }

    /// Returns the caller-defined retrieval key.
    pub fn route_key(&self) -> &SkillRouteKey {
        &self.route_key
    }
}

/// A validated routing overlay over a skill bank.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillRouteRegistry {
    entries: BTreeMap<SkillName, SkillRouteEntry>,
}

impl SkillRouteRegistry {
    /// Builds a registry from caller-supplied route specs over a skill bank.
    ///
    /// # Errors
    ///
    /// Returns [`SkillRouteRegistryError`] when a spec references an unknown
    /// skill or supplies duplicate membership for one skill.
    pub fn from_specs(
        bank: &SkillBank,
        specs: impl IntoIterator<Item = SkillRouteSpec>,
    ) -> Result<Self, SkillRouteRegistryError> {
        let mut entries = BTreeMap::new();
        for spec in specs {
            let SkillRouteSpec {
                skill,
                pool,
                route_key,
            } = spec;
            let folder = bank
                .get(&skill)
                .ok_or_else(|| SkillRouteRegistryError::UnknownSkill {
                    skill: skill.clone(),
                })?;
            if entries.contains_key(&skill) {
                return Err(SkillRouteRegistryError::DuplicateSkill { skill });
            }
            let card = SkillCard::from_folder(folder);
            let entry = SkillRouteEntry::new(card, pool, route_key);
            entries.insert(skill, entry);
        }
        Ok(Self { entries })
    }

    /// Returns all route entries in stable skill-name order.
    pub fn entries(&self) -> Vec<&SkillRouteEntry> {
        self.entries.values().collect()
    }

    /// Returns route entries in one pool in stable skill-name order.
    pub fn by_pool(&self, pool: &SkillRoutePool) -> Vec<&SkillRouteEntry> {
        self.entries
            .values()
            .filter(|entry| entry.pool() == pool)
            .collect()
    }

    /// Returns one routed skill entry.
    pub fn get(&self, skill: &SkillName) -> Option<&SkillRouteEntry> {
        self.entries.get(skill)
    }

    /// Returns true when the registry has no route entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
