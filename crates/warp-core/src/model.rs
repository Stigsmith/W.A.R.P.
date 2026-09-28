//! Core data types shared across WARP.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A Steam Workshop item id (Steam calls it `PublishedFileId`).
///
/// Serialized as a string: ids are u64 and JSON consumers should never see them
/// as floating-point numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkshopId(pub u64);

impl fmt::Display for WorkshopId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for WorkshopId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.trim().parse().map(WorkshopId)
    }
}

impl Serialize for WorkshopId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WorkshopId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = WorkshopId;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a workshop id as a string or integer")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<WorkshopId, E> {
                Ok(WorkshopId(v))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<WorkshopId, E> {
                v.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Steam metadata for a Workshop item. A cache of what Steam says, never edited by the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModInfo {
    pub id: WorkshopId,
    pub title: String,
    pub description: String,
    pub steam_tags: Vec<String>,
    /// Size in bytes.
    pub file_size: u64,
    /// Unix seconds.
    pub time_created: i64,
    /// Unix seconds. Changes whenever the author uploads a new version.
    pub time_updated: i64,
    pub preview_url: String,
    pub subscriptions: u64,
    pub favorited: u64,
    pub views: u64,
    /// False when Steam reports the item as removed or hidden.
    pub available: bool,
}

impl ModInfo {
    /// A placeholder for an id Steam hasn't told us about yet.
    pub fn unknown(id: WorkshopId) -> Self {
        Self {
            id,
            title: String::new(),
            description: String::new(),
            steam_tags: Vec::new(),
            file_size: 0,
            time_created: 0,
            time_updated: 0,
            preview_url: String::new(),
            subscriptions: 0,
            favorited: 0,
            views: 0,
            available: true,
        }
    }

    pub fn workshop_url(&self) -> String {
        workshop_url(self.id)
    }
}

pub fn workshop_url(id: WorkshopId) -> String {
    format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}")
}

/// Opens the Workshop page inside the Steam client, where the Subscribe button works.
pub fn steam_client_url(id: WorkshopId) -> String {
    format!("steam://url/CommunityFilePage/{id}")
}

/// `true` for names that look like a `.pack` file. Case-insensitive.
pub fn is_pack_name(name: &str) -> bool {
    name.len() > 5 && name[name.len() - 5..].eq_ignore_ascii_case(".pack")
}

/// The identity used to match packs across lists: the file name, lowercased.
pub fn pack_key(name: &str) -> String {
    name.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workshop_id_round_trips_as_string() {
        let id = WorkshopId(2_968_554_980);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"2968554980\"");
        assert_eq!(serde_json::from_str::<WorkshopId>(&json).unwrap(), id);
        assert_eq!(
            serde_json::from_str::<WorkshopId>("2968554980").unwrap(),
            id
        );
    }

    #[test]
    fn pack_names() {
        assert!(is_pack_name("!abc.PACK"));
        assert!(!is_pack_name(".pack"));
        assert!(!is_pack_name("abc.png"));
        assert_eq!(pack_key("  Skarbrand.pack "), "skarbrand.pack");
    }
}
