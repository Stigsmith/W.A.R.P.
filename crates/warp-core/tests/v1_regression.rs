//! Checks the solver against W.A.R.P. v1's proven ordering on a real v1 workbook.
//!
//! Needs the workbook, which isn't part of the repo:
//!     WARP_V1_WORKBOOK="C:\path\to\WARP Database.zip" cargo test -p warp-core --test v1_regression -- --nocapture
//! Without it the test is skipped.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use warp_core::import_v1::{self, V1Mod};
use warp_core::knowledge::KnowledgeBase;
use warp_core::library::Library;
use warp_core::order::Reason;
use warp_core::store::{ProfileDef, Store};

/// The v1 profile this checks: Solo Chaos v4.
const COMPONENTS: [&str; 4] = ["Base v1", "SFO", "Old World", "Chaos v4"];

/// v1's VBA `CompareKaedrinOrder`, verbatim (including its en dash). Positive means `a` goes first.
fn v1_compare_names(a: &str, b: &str) -> i32 {
    const ORDER: &str = "!#$%&'()+,–;=@0123456789abcdefghijklmnopqrstuvwxyz[]^_`{}~";
    let rank = |c: char| ORDER.chars().position(|o| o == c).map_or(1000, |i| i + 1);
    let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
    for i in 0..a.len().max(b.len()) {
        if i >= a.len() {
            return -1;
        }
        if i >= b.len() {
            return 1;
        }
        let (ra, rb) = (rank(a[i]), rank(b[i]));
        if ra != rb {
            return if ra < rb { 1 } else { -1 };
        }
    }
    0
}

/// v1's sort: Category descending, Subcategory descending, then pack name.
fn v1_order(mods: &[&V1Mod]) -> Vec<String> {
    let mut v: Vec<&V1Mod> = mods.to_vec();
    v.sort_by(|a, b| {
        b.source_category
            .cmp(&a.source_category)
            .then(b.source_subcategory.cmp(&a.source_subcategory))
            .then_with(|| match v1_compare_names(&a.pack, &b.pack) {
                x if x > 0 => Ordering::Less,
                0 => Ordering::Equal,
                _ => Ordering::Greater,
            })
    });
    v.into_iter().map(|m| m.pack.to_lowercase()).collect()
}

#[test]
fn solver_keeps_v1_tier_bands() {
    let Ok(path) = std::env::var("WARP_V1_WORKBOOK") else {
        eprintln!("skipped: set WARP_V1_WORKBOOK to a v1 workbook to run this check");
        return;
    };
    let import = import_v1::read(path.as_ref()).expect("workbook reads");

    let wanted: HashSet<String> = COMPONENTS.iter().map(|c| c.to_lowercase()).collect();
    let selected: Vec<&V1Mod> = import
        .mods
        .iter()
        .filter(|m| !m.pack.is_empty() && m.component.as_ref().is_some_and(|c| wanted.contains(&c.to_lowercase())))
        .collect();
    let old = v1_order(&selected);

    let mut kb = KnowledgeBase::new();
    for m in &import.mods {
        kb.mods.insert(m.info.id, m.knowledge.clone());
    }
    let mut lib = Library::new(Store::open_in_memory().unwrap(), kb);
    lib.import_v1(&import).unwrap();
    let def = ProfileDef { name: "Solo Chaos v4".into(), sets: COMPONENTS.iter().map(|s| s.to_string()).collect(), ..Default::default() };
    let resolved = lib.resolve_profile(&def).unwrap();
    let new: Vec<String> = resolved.order.placements.iter().map(|p| p.pack.to_lowercase()).collect();

    // 1. Same packs.
    let (old_set, new_set): (HashSet<_>, HashSet<_>) = (old.iter().collect(), new.iter().collect());
    assert_eq!(old_set, new_set, "v1 and the solver place different packs");

    // 2. Every cross-tier disagreement is explained by a hard rule.
    let tier: HashMap<String, String> = resolved.order.placements.iter().map(|p| (p.pack.to_lowercase(), p.tier.clone())).collect();
    let ruled: HashSet<String> = resolved
        .order
        .placements
        .iter()
        .filter(|p| p.reasons.iter().any(|r| matches!(r, Reason::Raised { .. } | Reason::Above { .. } | Reason::Below { .. })))
        .map(|p| p.pack.to_lowercase())
        .collect();
    let new_pos: HashMap<&String, usize> = new.iter().enumerate().map(|(i, p)| (p, i)).collect();
    let (mut within_tier, mut cross_tier_ruled, mut unexplained) = (0, 0, Vec::new());
    for i in 0..old.len() {
        for j in i + 1..old.len() {
            let (x, y) = (&old[i], &old[j]);
            if new_pos[x] < new_pos[y] {
                continue; // same relative order
            }
            if tier[x] == tier[y] {
                within_tier += 1;
            } else if ruled.contains(x) || ruled.contains(y) {
                cross_tier_ruled += 1;
            } else {
                unexplained.push(format!("{x} ({}) vs {y} ({})", tier[x], tier[y]));
            }
        }
    }
    eprintln!(
        "{} packs. Pairs ordered differently from v1: {within_tier} within a tier (role/topic ordering), \
         {cross_tier_ruled} across tiers because of a dependency rule",
        new.len()
    );
    assert!(unexplained.is_empty(), "cross-tier changes without a rule:\n{}", unexplained.join("\n"));
}
