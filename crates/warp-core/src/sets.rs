//! How profiles use sets.
//!
//! A profile keeps its own copy of what each of its sets contained when it took
//! them, so editing a set never changes a profile behind the user's back. When a
//! set changes, the profile gets a notice ([`SetUpdate`]) and the user decides:
//! take the new contents, dismiss this change, or stop being asked. A profile can
//! also follow its sets, taking every change as it happens.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::WorkshopId;
use crate::store::{ModSet, ProfileDef};

/// What a profile does when one of its sets changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetChanges {
    /// Keep the profile as it is and show a notice.
    #[default]
    Ask,
    /// Take every change straight away.
    Follow,
    /// Keep the profile as it is, without a notice.
    Ignore,
}

/// A set that changed since a profile took it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetUpdate {
    pub set: String,
    /// Mods the set gained.
    pub added: Vec<WorkshopId>,
    /// Mods the set lost.
    pub removed: Vec<WorkshopId>,
    /// The set no longer exists; taking the update drops it from the profile.
    pub deleted: bool,
}

pub fn find<'a>(sets: &'a [ModSet], name: &str) -> Option<&'a ModSet> {
    sets.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// The mods a profile gets from one of its sets: its own copy, or the live set
/// for a profile that has none yet.
pub fn members_of<'a>(def: &'a ProfileDef, name: &str, sets: &'a [ModSet]) -> &'a [WorkshopId] {
    match def.set_members.get(name) {
        Some(copy) => copy,
        None => find(sets, name).map_or(&[], |s| &s.members),
    }
}

/// A short, stable name for a set's contents, used to remember a dismissed change.
pub fn fingerprint(set: Option<&ModSet>) -> String {
    let Some(set) = set else {
        return "deleted".into();
    };
    let ids: BTreeSet<u64> = set.members.iter().map(|id| id.0).collect();
    // FNV-1a over the sorted ids.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for id in ids {
        for b in id.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

/// The changes a profile should hear about.
pub fn pending(def: &ProfileDef, sets: &[ModSet]) -> Vec<SetUpdate> {
    if def.set_changes != SetChanges::Ask {
        return Vec::new();
    }
    let mut out = Vec::new();
    for name in &def.sets {
        let Some(copy) = def.set_members.get(name) else {
            continue;
        };
        let live = find(sets, name);
        if def.dismissed.get(name) == Some(&fingerprint(live)) {
            continue;
        }
        let had: BTreeSet<WorkshopId> = copy.iter().copied().collect();
        let has: BTreeSet<WorkshopId> = live
            .map(|s| s.members.iter().copied().collect())
            .unwrap_or_default();
        if live.is_some() && had == has {
            continue;
        }
        out.push(SetUpdate {
            set: name.clone(),
            added: has.difference(&had).copied().collect(),
            removed: had.difference(&has).copied().collect(),
            deleted: live.is_none(),
        });
    }
    out
}

/// Gives the profile a copy of every set it lacks one for, and forgets copies and
/// dismissals of sets it no longer uses. Following profiles take every set fresh.
pub fn fill(def: &mut ProfileDef, sets: &[ModSet]) {
    let names = def.sets.clone();
    for name in &names {
        if def.set_changes == SetChanges::Follow || !def.set_members.contains_key(name) {
            take(def, name, sets);
        }
    }
    def.set_members.retain(|name, _| names.contains(name));
    def.dismissed.retain(|name, _| names.contains(name));
}

/// Takes a set's current contents into the profile; a deleted set leaves it.
/// A set the profile doesn't use is left alone.
pub fn take(def: &mut ProfileDef, name: &str, sets: &[ModSet]) {
    if !def.sets.iter().any(|s| s == name) {
        return;
    }
    def.dismissed.remove(name);
    match find(sets, name) {
        Some(set) => {
            def.set_members.insert(name.to_owned(), set.members.clone());
        }
        // A new profile may name a set that isn't made yet; only a set the
        // profile already had a copy of counts as deleted.
        None if def.set_members.contains_key(name) => {
            def.sets.retain(|s| s != name);
            def.set_members.remove(name);
        }
        None => {
            def.set_members.insert(name.to_owned(), Vec::new());
        }
    }
}

/// Remembers the current state of a set as seen, so its change stops showing.
pub fn dismiss(def: &mut ProfileDef, name: &str, sets: &[ModSet]) {
    if def.sets.iter().any(|s| s == name) {
        def.dismissed
            .insert(name.to_owned(), fingerprint(find(sets, name)));
    }
}

/// Follows a set's rename in a profile. True if the profile uses the set.
pub fn rename(def: &mut ProfileDef, from: &str, to: &str) -> bool {
    let Some(old) = def
        .sets
        .iter()
        .find(|s| s.eq_ignore_ascii_case(from))
        .cloned()
    else {
        return false;
    };
    for s in &mut def.sets {
        if *s == old {
            *s = to.to_owned();
        }
    }
    if let Some(copy) = def.set_members.remove(&old) {
        def.set_members.insert(to.to_owned(), copy);
    }
    if let Some(seen) = def.dismissed.remove(&old) {
        def.dismissed.insert(to.to_owned(), seen);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(name: &str, ids: &[u64]) -> ModSet {
        ModSet {
            name: name.into(),
            members: ids.iter().map(|&i| WorkshopId(i)).collect(),
        }
    }

    fn profile(sets: &[&str]) -> ProfileDef {
        ProfileDef {
            name: "p".into(),
            sets: sets.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_profile_keeps_its_copy_until_told_otherwise() {
        let mut sets = vec![set("Base", &[1, 2])];
        let mut p = profile(&["Base"]);
        fill(&mut p, &sets);
        sets[0] = set("Base", &[2, 3]);

        assert_eq!(
            members_of(&p, "Base", &sets),
            [WorkshopId(1), WorkshopId(2)]
        );
        let notes = pending(&p, &sets);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].added, [WorkshopId(3)]);
        assert_eq!(notes[0].removed, [WorkshopId(1)]);

        take(&mut p, "Base", &sets);
        assert_eq!(
            members_of(&p, "Base", &sets),
            [WorkshopId(2), WorkshopId(3)]
        );
        assert!(pending(&p, &sets).is_empty());
    }

    #[test]
    fn a_dismissed_change_stays_quiet_until_the_set_changes_again() {
        let mut sets = vec![set("Base", &[1])];
        let mut p = profile(&["Base"]);
        fill(&mut p, &sets);
        sets[0] = set("Base", &[1, 2]);
        dismiss(&mut p, "Base", &sets);
        assert!(pending(&p, &sets).is_empty());
        assert_eq!(
            members_of(&p, "Base", &sets),
            [WorkshopId(1)],
            "dismissing doesn't take the change"
        );
        sets[0] = set("Base", &[1, 2, 3]);
        assert_eq!(pending(&p, &sets)[0].added, [WorkshopId(2), WorkshopId(3)]);
    }

    #[test]
    fn following_and_ignoring_profiles_get_no_notices() {
        let mut sets = vec![set("Base", &[1])];
        let mut follow = profile(&["Base"]);
        follow.set_changes = SetChanges::Follow;
        let mut ignore = profile(&["Base"]);
        ignore.set_changes = SetChanges::Ignore;
        fill(&mut follow, &sets);
        fill(&mut ignore, &sets);
        sets[0] = set("Base", &[1, 2]);
        assert!(pending(&follow, &sets).is_empty());
        assert!(pending(&ignore, &sets).is_empty());
        fill(&mut follow, &sets);
        assert_eq!(members_of(&follow, "Base", &sets).len(), 2);
        assert_eq!(members_of(&ignore, "Base", &sets).len(), 1);
    }

    #[test]
    fn a_deleted_set_is_a_notice_and_taking_it_drops_the_set() {
        let sets = vec![set("Base", &[1]), set("Chaos", &[2])];
        let mut p = profile(&["Base", "Chaos"]);
        fill(&mut p, &sets);
        let sets = vec![set("Base", &[1])];
        let notes = pending(&p, &sets);
        assert!(notes[0].deleted);
        assert_eq!(notes[0].removed, [WorkshopId(2)]);
        take(&mut p, "Chaos", &sets);
        assert_eq!(p.sets, ["Base"]);
        assert!(!p.set_members.contains_key("Chaos"));
    }

    #[test]
    fn rename_moves_the_copy_and_fill_drops_unused_ones() {
        let sets = vec![set("Base", &[1])];
        let mut p = profile(&["base"]);
        fill(&mut p, &sets);
        assert!(rename(&mut p, "Base", "Core stuff"));
        assert_eq!(p.sets, ["Core stuff"]);
        assert_eq!(p.set_members["Core stuff"], [WorkshopId(1)]);
        p.sets.clear();
        fill(&mut p, &sets);
        assert!(p.set_members.is_empty());
    }

    #[test]
    fn fingerprint_ignores_order() {
        assert_eq!(
            fingerprint(Some(&set("a", &[1, 2]))),
            fingerprint(Some(&set("b", &[2, 1])))
        );
        assert_ne!(fingerprint(Some(&set("a", &[1]))), fingerprint(None));
    }
}
