//! The load-order solver.
//!
//! Default order: tier, then role, then Kaedrin name order (see docs/load-order.md).
//! Hard rules (requires / patches / pins) are applied on top. When a rule disagrees
//! with the default, the dependent is raised to sit directly above what it depends
//! on; the parent is never pulled down.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use crate::kaedrin;
use crate::model::{WorkshopId, pack_key};

/// One pack to be placed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderItem {
    pub pack: String,
    pub workshop_id: Option<WorkshopId>,
    pub tier: String,
    pub tier_priority: i32,
    pub role: String,
    pub role_priority: i32,
    pub requires: Vec<WorkshopId>,
    pub patches: Vec<WorkshopId>,
}

/// A user's explicit "this pack above that pack".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pin {
    pub above: String,
    pub below: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Requires,
    Patches,
    Pin,
}

/// A hard rule between two packs in the list: `above` must sit higher than `below`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Rule {
    pub above: String,
    pub below: String,
    pub kind: RuleKind,
}

/// Why a pack is where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    /// Position from tier and role alone.
    Default { tier: String, role: String },
    /// A hard rule this pack satisfies by sitting above `other`.
    Above { other: String, rule: RuleKind },
    /// A hard rule this pack satisfies by sitting below `other`.
    Below { other: String, rule: RuleKind },
    /// Moved above `other` against the default order because of a rule.
    Raised { other: String, rule: RuleKind },
    /// Part of contradictory rules; placed by the default order instead.
    InCycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub pack: String,
    pub workshop_id: Option<WorkshopId>,
    pub tier: String,
    pub role: String,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderResult {
    /// Top of the load order first.
    pub placements: Vec<Placement>,
    /// Groups of rules that contradict each other.
    pub cycles: Vec<Vec<Rule>>,
    /// Pins that name packs not in the list; ignored.
    pub unused_pins: Vec<Pin>,
}

impl OrderResult {
    pub fn packs(&self) -> Vec<&str> {
        self.placements.iter().map(|p| p.pack.as_str()).collect()
    }
}

/// Sort key for the default order. `a < b` means `a` belongs lower in the list.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SoftKey {
    tier: i32,
    role: i32,
    // Earlier in Kaedrin order sits higher, so the name comparison is reversed.
    name: Reverse<Vec<u32>>,
    exact: Reverse<String>,
}

impl SoftKey {
    fn of(item: &OrderItem) -> Self {
        Self {
            tier: item.tier_priority,
            role: item.role_priority,
            name: Reverse(kaedrin::name_key(&item.pack)),
            exact: Reverse(pack_key(&item.pack)),
        }
    }
}

/// Orders the default way, ignoring every hard rule.
pub fn default_order(items: &[OrderItem]) -> Vec<usize> {
    let keys: Vec<SoftKey> = items.iter().map(SoftKey::of).collect();
    let mut idx: Vec<usize> = (0..items.len()).collect();
    idx.sort_by(|&a, &b| keys[b].cmp(&keys[a]));
    idx
}

/// Produces the load order. Duplicate packs (same name, any case) are kept once.
pub fn solve(items: &[OrderItem], pins: &[Pin]) -> OrderResult {
    // Deduplicate by pack name, keeping the first occurrence.
    let mut seen = BTreeSet::new();
    let items: Vec<&OrderItem> = items
        .iter()
        .filter(|i| seen.insert(pack_key(&i.pack)))
        .collect();
    let n = items.len();
    let by_pack: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (pack_key(&it.pack), i))
        .collect();
    let mut by_mod: HashMap<WorkshopId, Vec<usize>> = HashMap::new();
    for (i, it) in items.iter().enumerate() {
        if let Some(id) = it.workshop_id {
            by_mod.entry(id).or_default().push(i);
        }
    }

    // Hard rules as edges above -> below.
    let mut edges: BTreeSet<(usize, usize, RuleKind)> = BTreeSet::new();
    for (a, it) in items.iter().enumerate() {
        for (targets, kind) in [
            (&it.requires, RuleKind::Requires),
            (&it.patches, RuleKind::Patches),
        ] {
            for target in targets {
                for &b in by_mod.get(target).into_iter().flatten() {
                    if b != a {
                        edges.insert((a, b, kind));
                    }
                }
            }
        }
    }
    let mut unused_pins = Vec::new();
    for pin in pins {
        match (
            by_pack.get(&pack_key(&pin.above)),
            by_pack.get(&pack_key(&pin.below)),
        ) {
            (Some(&a), Some(&b)) if a != b => {
                edges.insert((a, b, RuleKind::Pin));
            }
            _ => unused_pins.push(pin.clone()),
        }
    }

    let keys: Vec<SoftKey> = items.iter().map(|it| SoftKey::of(it)).collect();
    // For the bottom-up pass: a node can be placed once everything that must sit below it is placed.
    let mut waiting_on = vec![0usize; n];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b, _) in &edges {
        waiting_on[a] += 1;
        dependents[b].push(a);
    }

    // Build from the bottom: always place the lowest-priority node that is free to go.
    let mut heap: BinaryHeap<Reverse<(SoftKey, usize)>> = (0..n)
        .filter(|&i| waiting_on[i] == 0)
        .map(|i| Reverse((keys[i].clone(), i)))
        .collect();
    let mut placed = vec![false; n];
    let mut bottom_up = Vec::with_capacity(n);
    let mut in_cycle = vec![false; n];
    let mut cycles = Vec::new();

    while bottom_up.len() < n {
        let next = match heap.pop() {
            Some(Reverse((_, i))) if !placed[i] => i,
            Some(_) => continue,
            None => {
                // Everything left is blocked by contradictory rules. Report the cycle and
                // break it at its lowest member (by default order); nodes merely waiting
                // on the cycle keep their rules.
                let remaining: Vec<usize> = (0..n).filter(|&i| !placed[i]).collect();
                let members: Vec<usize> = match find_cycle(&remaining, &edges, &placed) {
                    Some(cycle) => {
                        cycles.push(
                            cycle
                                .iter()
                                .map(|&(a, b, kind)| Rule {
                                    above: items[a].pack.clone(),
                                    below: items[b].pack.clone(),
                                    kind,
                                })
                                .collect(),
                        );
                        cycle.iter().map(|&(a, _, _)| a).collect()
                    }
                    None => remaining,
                };
                for &m in &members {
                    in_cycle[m] = true;
                }
                *members
                    .iter()
                    .min_by(|&&a, &&b| keys[a].cmp(&keys[b]))
                    .expect("nodes remain")
            }
        };
        placed[next] = true;
        bottom_up.push(next);
        for &a in &dependents[next] {
            waiting_on[a] = waiting_on[a].saturating_sub(1);
            if waiting_on[a] == 0 && !placed[a] {
                heap.push(Reverse((keys[a].clone(), a)));
            }
        }
    }

    let position: Vec<usize> = {
        let mut pos = vec![0; n];
        for (p, &i) in bottom_up.iter().rev().enumerate() {
            pos[i] = p;
        }
        pos
    };

    let placements = bottom_up
        .iter()
        .rev()
        .map(|&i| {
            let it = items[i];
            let mut reasons = vec![Reason::Default {
                tier: it.tier.clone(),
                role: it.role.clone(),
            }];
            for &(a, b, kind) in &edges {
                if a == i && position[a] < position[b] {
                    reasons.push(Reason::Above {
                        other: items[b].pack.clone(),
                        rule: kind,
                    });
                    if keys[a] < keys[b] {
                        reasons.push(Reason::Raised {
                            other: items[b].pack.clone(),
                            rule: kind,
                        });
                    }
                } else if b == i && position[a] < position[b] {
                    reasons.push(Reason::Below {
                        other: items[a].pack.clone(),
                        rule: kind,
                    });
                }
            }
            if in_cycle[i] {
                reasons.push(Reason::InCycle);
            }
            Placement {
                pack: it.pack.clone(),
                workshop_id: it.workshop_id,
                tier: it.tier.clone(),
                role: it.role.clone(),
                reasons,
            }
        })
        .collect();

    OrderResult {
        placements,
        cycles,
        unused_pins,
    }
}

/// Finds one cycle among the unplaced nodes, as a list of edges.
fn find_cycle(
    remaining: &[usize],
    edges: &BTreeSet<(usize, usize, RuleKind)>,
    placed: &[bool],
) -> Option<Vec<(usize, usize, RuleKind)>> {
    let mut out: HashMap<usize, Vec<(usize, RuleKind)>> = HashMap::new();
    for &(a, b, k) in edges {
        if !placed[a] && !placed[b] {
            out.entry(a).or_default().push((b, k));
        }
    }
    // Walk edges from any remaining node; every remaining node is blocked, so
    // following outgoing edges must revisit a node.
    let start = *remaining.iter().find(|i| out.contains_key(i))?;
    let mut path: Vec<(usize, usize, RuleKind)> = Vec::new();
    let mut seen_at: HashMap<usize, usize> = HashMap::new();
    let mut node = start;
    loop {
        if let Some(&at) = seen_at.get(&node) {
            return Some(path[at..].to_vec());
        }
        seen_at.insert(node, path.len());
        let &(next, kind) = out.get(&node)?.first()?;
        path.push((node, next, kind));
        node = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn item(pack: &str, id: u64, tier: i32, role: i32) -> OrderItem {
        OrderItem {
            pack: pack.into(),
            workshop_id: Some(WorkshopId(id)),
            tier: format!("t{tier}"),
            tier_priority: tier,
            role: format!("r{role}"),
            role_priority: role,
            requires: vec![],
            patches: vec![],
        }
    }

    #[test]
    fn default_is_tier_then_role_then_name() {
        let items = vec![
            item("core.pack", 1, 0, 20),
            item("ui_b.pack", 2, 90, 20),
            item("!ui_a.pack", 3, 90, 20),
            item("ui_sub.pack", 4, 90, 40),
        ];
        let r = solve(&items, &[]);
        assert_eq!(
            r.packs(),
            ["ui_sub.pack", "!ui_a.pack", "ui_b.pack", "core.pack"]
        );
    }

    #[test]
    fn dependent_is_raised_directly_above_parent() {
        // A low-tier patch that patches a UI mod rises to sit just above it.
        let mut patch = item("patch.pack", 1, 0, 40);
        patch.patches = vec![WorkshopId(2)];
        let items = vec![
            patch,
            item("ui.pack", 2, 90, 20),
            item("battle.pack", 3, 70, 20),
            item("top.pack", 4, 110, 20),
        ];
        let r = solve(&items, &[]);
        assert_eq!(
            r.packs(),
            ["top.pack", "patch.pack", "ui.pack", "battle.pack"]
        );
        assert!(r.placements[1].reasons.contains(&Reason::Raised {
            other: "ui.pack".into(),
            rule: RuleKind::Patches
        }));
        assert!(r.placements[2].reasons.contains(&Reason::Below {
            other: "patch.pack".into(),
            rule: RuleKind::Patches
        }));
    }

    #[test]
    fn pins_apply_and_unknown_pins_are_reported() {
        let items = vec![item("a.pack", 1, 90, 20), item("b.pack", 2, 0, 20)];
        let pins = vec![
            Pin {
                above: "B.pack".into(),
                below: "a.pack".into(),
            },
            Pin {
                above: "ghost.pack".into(),
                below: "a.pack".into(),
            },
        ];
        let r = solve(&items, &pins);
        assert_eq!(r.packs(), ["b.pack", "a.pack"]);
        assert_eq!(r.unused_pins.len(), 1);
    }

    #[test]
    fn cycles_are_reported_and_everything_is_still_placed() {
        let mut a = item("a.pack", 1, 50, 20);
        let mut b = item("b.pack", 2, 50, 20);
        a.requires = vec![WorkshopId(2)];
        b.requires = vec![WorkshopId(1)];
        let r = solve(&[a, b, item("c.pack", 3, 0, 20)], &[]);
        assert_eq!(r.placements.len(), 3);
        assert_eq!(r.cycles.len(), 1);
        assert_eq!(r.cycles[0].len(), 2);
        assert!(
            r.placements
                .iter()
                .any(|p| p.reasons.contains(&Reason::InCycle))
        );
    }

    #[test]
    fn duplicate_packs_are_placed_once() {
        let r = solve(&[item("a.pack", 1, 0, 20), item("A.pack", 2, 90, 20)], &[]);
        assert_eq!(r.packs(), ["a.pack"]);
    }

    fn arb_items() -> impl Strategy<Value = (Vec<OrderItem>, Vec<(usize, usize)>)> {
        (2usize..25).prop_flat_map(|n| {
            let items = proptest::collection::vec((0i32..5, 0i32..3), n).prop_map(|v| {
                v.into_iter()
                    .enumerate()
                    .map(|(i, (t, r))| item(&format!("p{i:02}.pack"), i as u64, t * 10, r * 10))
                    .collect::<Vec<_>>()
            });
            let edges = proptest::collection::vec((0..n, 0..n), 0..n);
            (items, edges)
        })
    }

    proptest! {
        /// Edges only ever point from a higher index to a lower one, so the rules
        /// are acyclic; every one of them must hold in the result.
        #[test]
        fn acyclic_rules_always_hold((mut items, raw) in arb_items()) {
            for (x, y) in raw {
                let (hi, lo) = (x.max(y), x.min(y));
                if hi != lo {
                    items[hi].requires.push(WorkshopId(lo as u64));
                }
            }
            let r = solve(&items, &[]);
            prop_assert!(r.cycles.is_empty());
            prop_assert_eq!(r.placements.len(), items.len());
            let pos: HashMap<&str, usize> = r.packs().into_iter().enumerate().map(|(i, p)| (p, i)).collect();
            for it in &items {
                for dep in &it.requires {
                    let dep_pack = &items[dep.0 as usize].pack;
                    prop_assert!(pos[it.pack.as_str()] < pos[dep_pack.as_str()], "{} must sit above {}", it.pack, dep_pack);
                }
            }
        }

        #[test]
        fn output_is_deterministic_and_order_independent((items, _) in arb_items()) {
            let a = solve(&items, &[]);
            let mut shuffled = items.clone();
            shuffled.reverse();
            let b = solve(&shuffled, &[]);
            prop_assert_eq!(a.packs(), b.packs());
        }

        #[test]
        fn without_rules_it_equals_the_default_order((items, _) in arb_items()) {
            let r = solve(&items, &[]);
            let expected: Vec<&str> = default_order(&items).into_iter().map(|i| items[i].pack.as_str()).collect();
            prop_assert_eq!(r.packs(), expected);
        }
    }
}
