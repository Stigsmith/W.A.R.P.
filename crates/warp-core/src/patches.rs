//! Game updates that break mods (`knowledge/patches.toml`), and the mods that were
//! kept up to date for the previous one but haven't been updated since the latest.

use serde::{Deserialize, Serialize};

use crate::Error;

const BUILTIN: &str = include_str!("../../../knowledge/patches.toml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePatch {
    /// `9.0`
    pub version: String,
    #[serde(default)]
    pub name: String,
    /// `YYYY-MM-DD`, the day it went live for everyone (UTC).
    pub released: String,
}

impl GamePatch {
    /// Unix seconds at the start of the release day.
    pub fn released_at(&self) -> Result<i64, Error> {
        let bad = || {
            Error::Format(format!(
                "patch {}: released must be YYYY-MM-DD, not {:?}",
                self.version, self.released
            ))
        };
        let parts: Vec<i64> = self
            .released
            .split('-')
            .map(|p| p.parse().map_err(|_| bad()))
            .collect::<Result<_, _>>()?;
        match parts[..] {
            [y, m, d] if (1..=12).contains(&m) && (1..=31).contains(&d) => {
                Ok(days_from_civil(y, m, d) * 86_400)
            }
            _ => Err(bad()),
        }
    }
}

/// A mod kept up to date for `previous` that hasn't been updated since `patch` came out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotUpdated {
    /// The latest update, `9.0`.
    pub patch: String,
    /// Unix seconds: the day `patch` went live.
    pub released: i64,
    /// The update before it, `8.0`.
    pub previous: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePatches {
    /// Newest first.
    #[serde(rename = "patch")]
    pub patches: Vec<GamePatch>,
}

impl GamePatches {
    /// The list shipped with WARP (`knowledge/patches.toml`).
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("built-in patches.toml is valid")
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut list: Self = toml::from_str(text).map_err(|e| Error::Format(e.to_string()))?;
        let mut dated = Vec::with_capacity(list.patches.len());
        for p in list.patches.drain(..) {
            dated.push((p.released_at()?, p));
        }
        dated.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        list.patches = dated.into_iter().map(|(_, p)| p).collect();
        Ok(list)
    }

    /// For a mod whose newest version is from `time_updated` (Unix seconds): was it
    /// updated after the previous game update, but not since the latest one?
    pub fn not_updated(&self, time_updated: i64) -> Option<NotUpdated> {
        let [latest, previous, ..] = &self.patches[..] else {
            return None;
        };
        let (latest_at, previous_at) = (latest.released_at().ok()?, previous.released_at().ok()?);
        (time_updated >= previous_at && time_updated < latest_at).then(|| NotUpdated {
            patch: latest.version.clone(),
            released: latest_at,
            previous: previous.version.clone(),
        })
    }
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn list() -> GamePatches {
        GamePatches::parse(
            r#"
            [[patch]]
            version = "8.0"
            released = "2026-05-21"
            [[patch]]
            version = "9.0"
            released = "2026-09-24"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn release_days_are_utc_midnight() {
        let p = |released: &str| GamePatch {
            version: "x".into(),
            name: String::new(),
            released: released.into(),
        };
        assert_eq!(p("1970-01-01").released_at().unwrap(), 0);
        assert_eq!(p("2000-02-29").released_at().unwrap(), 951_782_400);
        assert_eq!(p("2026-09-24").released_at().unwrap(), 1_790_208_000);
        assert!(p("2026-13-01").released_at().is_err());
        assert!(p("24 Sept 2026").released_at().is_err());
    }

    #[test]
    fn sorted_newest_first() {
        assert_eq!(list().patches[0].version, "9.0");
    }

    #[test]
    fn flags_only_mods_kept_up_to_date_for_the_previous_update() {
        let list = list();
        let v8 = list.patches[1].released_at().unwrap();
        let v9 = list.patches[0].released_at().unwrap();
        // Updated during 8.x, not since 9.0: flagged.
        let flagged = list.not_updated(v9 - 1).unwrap();
        assert_eq!(
            (
                flagged.patch.as_str(),
                flagged.previous.as_str(),
                flagged.released
            ),
            ("9.0", "8.0", v9)
        );
        assert!(list.not_updated(v8).is_some());
        // Updated on 9.0's release day or later: fine.
        assert_eq!(list.not_updated(v9), None);
        assert_eq!(list.not_updated(v9 + 30 * DAY), None);
        // Already older than 8.0: not flagged. Unknown (0): not flagged.
        assert_eq!(list.not_updated(v8 - 1), None);
        assert_eq!(list.not_updated(0), None);
    }

    #[test]
    fn builtin_list_is_valid_and_needs_two_updates() {
        assert!(GamePatches::builtin().patches.len() >= 2);
        assert_eq!(GamePatches::default().not_updated(1_790_000_000), None);
    }
}
