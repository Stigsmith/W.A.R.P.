//! Diffs two real Kaedrin profiles (Solo Chaos v3.2 -> v3.3).
//! Expected values were checked against a plain line diff of the two files.

use warp_core::kaedrin;
use warp_core::mp::{self, ShareEntry, ShareList};

fn load(name: &str) -> ShareList {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    ShareList {
        name: name.into(),
        entries: kaedrin::read_profile(&text)
            .into_iter()
            .map(|p| ShareEntry::new(p, None, 0))
            .collect(),
    }
}

#[test]
fn v3_2_to_v3_3_adds_five_mods() {
    let (a, b) = (
        load("profile_Solo Chaos v3.2.txt"),
        load("profile_Solo Chaos v3.3.txt"),
    );
    assert_eq!((a.entries.len(), b.entries.len()), (145, 150));
    let d = mp::diff(&a, &b);
    assert_eq!(d.common, 145);
    assert!(d.only_in_a.is_empty());
    assert!(d.moves.is_empty());
    let mut added: Vec<&str> = d.only_in_b.iter().map(|e| e.pack.as_str()).collect();
    added.sort_unstable();
    assert_eq!(
        added,
        [
            "@beautiful_slaanesh.pack",
            "DANUnitExpansion.pack",
            "MfGreenskinSkills.pack",
            "The Marauding Hordes.pack",
            "salem_chaos_ogre_upgrade_vanilla.pack"
        ]
    );
}

#[test]
fn reversed_order_needs_n_minus_one_moves() {
    let a = load("profile_Solo Chaos v3.3.txt");
    let mut b = a.clone();
    b.entries.reverse();
    let d = mp::diff(&a, &b);
    assert_eq!(d.moves.len(), a.entries.len() - 1);
}
