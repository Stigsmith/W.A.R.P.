//! Tiers and roles: the vocabulary the load-order solver sorts by.

use serde::{Deserialize, Serialize};

use crate::Error;

const BUILTIN: &str = include_str!("../../../knowledge/taxonomy.toml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tier {
    pub key: String,
    pub name: String,
    /// Higher sits higher in the load order.
    pub priority: i32,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub key: String,
    pub name: String,
    /// Higher sits higher within its tier.
    pub priority: i32,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Taxonomy {
    #[serde(rename = "tier")]
    pub tiers: Vec<Tier>,
    #[serde(rename = "role")]
    pub roles: Vec<Role>,
}

impl Taxonomy {
    pub const DEFAULT_ROLE: &'static str = "content";

    /// The taxonomy shipped with WARP (`knowledge/taxonomy.toml`).
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("built-in taxonomy.toml is valid")
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut taxonomy: Self = toml::from_str(text).map_err(|e| Error::Format(e.to_string()))?;
        taxonomy.tiers.sort_by_key(|t| std::cmp::Reverse(t.priority));
        taxonomy.roles.sort_by_key(|r| std::cmp::Reverse(r.priority));
        if taxonomy.role(Self::DEFAULT_ROLE).is_none() {
            return Err(Error::Format(format!(
                "taxonomy must define the default role '{}'",
                Self::DEFAULT_ROLE
            )));
        }
        Ok(taxonomy)
    }

    pub fn tier(&self, key: &str) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.key == key)
    }

    pub fn role(&self, key: &str) -> Option<&Role> {
        self.roles.iter().find(|r| r.key == key)
    }

    /// The lowest tier: where a mod with no known tier lands, so an unclassified
    /// mod can never override a classified one by accident.
    pub fn fallback_tier(&self) -> &Tier {
        self.tiers.last().expect("taxonomy has at least one tier")
    }

    pub fn default_role(&self) -> &Role {
        self.role(Self::DEFAULT_ROLE).expect("checked in parse")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_parses_and_is_sorted_high_to_low() {
        let t = Taxonomy::builtin();
        assert_eq!(t.tiers.first().unwrap().key, "experimental");
        assert_eq!(t.fallback_tier().key, "core");
        assert_eq!(t.roles.first().unwrap().key, "submod");
        assert_eq!(t.default_role().key, "content");
        let mut keys: Vec<_> = t.tiers.iter().map(|t| &t.key).collect();
        keys.dedup();
        assert_eq!(keys.len(), t.tiers.len(), "tier keys are unique");
    }
}
