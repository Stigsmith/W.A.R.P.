//! The conflict map: which packs ship the same files, who wins, and whether that's a problem.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::model::pack_key;
use crate::pack_index::{ContentKind, PackIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Cosmetic: models, textures, UI images. The top pack's look wins.
    Low,
    /// Scripts, text or map data replaced wholesale. Often deliberate, sometimes breaks a feature.
    Medium,
    /// A whole DB table file or a campaign start position is discarded. Content silently disappears.
    High,
}

impl ContentKind {
    pub fn severity(self) -> Severity {
        match self {
            ContentKind::DbTable | ContentKind::Startpos => Severity::High,
            ContentKind::Script | ContentKind::Text | ContentKind::Map => Severity::Medium,
            ContentKind::Ui | ContentKind::Art | ContentKind::Other => Severity::Low,
        }
    }
}

/// One pack overriding another's files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairConflict {
    pub winner: String,
    pub loser: String,
    pub severity: Severity,
    /// The winner is meant to override the loser (it patches or requires it).
    pub intended: bool,
    pub total: usize,
    pub by_kind: BTreeMap<ContentKind, usize>,
    /// The overridden files, most serious first (capped).
    pub files: Vec<String>,
}

/// A pack most or all of whose files are overridden by packs above it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shadowed {
    pub pack: String,
    pub files: usize,
    pub overridden: usize,
    /// The packs doing the overriding, top first.
    pub by: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictReport {
    /// Problems first: unintended, then by severity and size.
    pub pairs: Vec<PairConflict>,
    pub shadowed: Vec<Shadowed>,
    /// Packs in the order that have no index (not installed, or unreadable).
    pub not_indexed: Vec<String>,
}

const MAX_FILES_PER_PAIR: usize = 300;
/// Report a pack as shadowed once at least this share of its files is overridden.
const SHADOWED_SHARE: f64 = 0.5;

/// Analyses a load order (pack names, top wins). `intended(winner, loser)` says whether
/// the winner is supposed to override the loser.
pub fn analyze(
    order: &[String],
    indexes: &HashMap<String, PackIndex>,
    intended: &dyn Fn(&str, &str) -> bool,
) -> ConflictReport {
    let mut report = ConflictReport::default();
    let mut present: Vec<(&String, &PackIndex)> = Vec::new();
    for name in order {
        match indexes.get(&pack_key(name)) {
            Some(idx) => present.push((name, idx)),
            None => report.not_indexed.push(name.clone()),
        }
    }

    // path -> positions (in `present`) of the packs shipping it, top first.
    let mut owners: HashMap<&str, Vec<usize>> = HashMap::new();
    for (pos, (_, idx)) in present.iter().enumerate() {
        for f in &idx.files {
            owners.entry(f.as_str()).or_default().push(pos);
        }
    }

    let mut pairs: HashMap<(usize, usize), Vec<&str>> = HashMap::new();
    let mut overridden: Vec<usize> = vec![0; present.len()];
    let mut overridden_by: Vec<BTreeMap<usize, ()>> = vec![BTreeMap::new(); present.len()];
    for (path, packs) in &owners {
        let Some((&winner, losers)) = packs.split_first() else {
            continue;
        };
        for &loser in losers {
            pairs.entry((winner, loser)).or_default().push(path);
            overridden[loser] += 1;
            overridden_by[loser].insert(winner, ());
        }
    }

    report.pairs = pairs
        .into_iter()
        .map(|((w, l), mut files)| {
            files.sort_by(|a, b| {
                ContentKind::of(b)
                    .severity()
                    .cmp(&ContentKind::of(a).severity())
                    .then(a.cmp(b))
            });
            let mut by_kind = BTreeMap::new();
            for f in &files {
                *by_kind.entry(ContentKind::of(f)).or_insert(0) += 1;
            }
            let (winner, loser) = (present[w].0.clone(), present[l].0.clone());
            PairConflict {
                severity: by_kind
                    .keys()
                    .map(|k| k.severity())
                    .max()
                    .unwrap_or(Severity::Low),
                intended: intended(&winner, &loser),
                total: files.len(),
                files: files
                    .into_iter()
                    .take(MAX_FILES_PER_PAIR)
                    .map(str::to_owned)
                    .collect(),
                by_kind,
                winner,
                loser,
            }
        })
        .collect();
    report.pairs.sort_by(|a, b| {
        a.intended
            .cmp(&b.intended)
            .then(b.severity.cmp(&a.severity))
            .then(b.total.cmp(&a.total))
            .then(a.winner.cmp(&b.winner))
            .then(a.loser.cmp(&b.loser))
    });

    report.shadowed = present
        .iter()
        .enumerate()
        .filter(|(pos, (_, idx))| {
            !idx.files.is_empty()
                && overridden[*pos] as f64 >= idx.files.len() as f64 * SHADOWED_SHARE
        })
        .map(|(pos, (name, idx))| Shadowed {
            pack: (*name).clone(),
            files: idx.files.len(),
            overridden: overridden[pos],
            by: overridden_by[pos]
                .keys()
                .map(|&w| present[w].0.clone())
                .collect(),
        })
        .collect();
    // Most overridden (by share of files) first.
    report
        .shadowed
        .sort_by_key(|s| std::cmp::Reverse(s.overridden * 1000 / s.files));
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(name: &str, files: &[&str]) -> (String, PackIndex) {
        (
            pack_key(name),
            PackIndex {
                name: name.into(),
                size: 0,
                modified: 0,
                kind: "mod".into(),
                dependencies: vec![],
                files: files.iter().map(|f| f.to_string()).collect(),
            },
        )
    }

    #[test]
    fn top_pack_wins_and_severity_follows_content() {
        let indexes: HashMap<_, _> = [
            idx("patch.pack", &["db/units_tables/data__", "ui/a.png"]),
            idx(
                "overhaul.pack",
                &["db/units_tables/data__", "ui/a.png", "script/x.lua"],
            ),
            idx("reskin.pack", &["variantmeshes/a.dds"]),
            idx("reskin2.pack", &["variantmeshes/a.dds"]),
        ]
        .into();
        let order: Vec<String> = ["patch.pack", "overhaul.pack", "reskin.pack", "reskin2.pack"]
            .map(String::from)
            .to_vec();
        let r = analyze(&order, &indexes, &|w, l| {
            w == "patch.pack" && l == "overhaul.pack"
        });

        assert_eq!(r.pairs.len(), 2);
        let reskin = &r.pairs[0];
        assert_eq!(
            (reskin.winner.as_str(), reskin.loser.as_str()),
            ("reskin.pack", "reskin2.pack")
        );
        assert_eq!(reskin.severity, Severity::Low);
        assert!(!reskin.intended, "unintended conflicts come first");

        let patch = &r.pairs[1];
        assert!(patch.intended);
        assert_eq!(patch.severity, Severity::High);
        assert_eq!(patch.total, 2);
        assert_eq!(
            patch.files[0], "db/units_tables/data__",
            "most serious file first"
        );

        // reskin2 is fully overridden; overhaul only 2 of 3.
        assert_eq!(r.shadowed.len(), 2);
        assert_eq!(r.shadowed[0].pack, "reskin2.pack");
        assert_eq!(r.shadowed[0].by, ["reskin.pack"]);
    }

    #[test]
    fn packs_without_index_are_listed() {
        let indexes: HashMap<_, _> = [idx("a.pack", &["x"])].into();
        let r = analyze(
            &["a.pack".into(), "ghost.pack".into()],
            &indexes,
            &|_, _| false,
        );
        assert_eq!(r.not_indexed, ["ghost.pack"]);
        assert!(r.pairs.is_empty());
    }
}
