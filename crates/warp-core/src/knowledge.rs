//! What WARP knows about a mod beyond Steam metadata: its tier, role, tags and
//! relations to other mods.
//!
//! Knowledge comes in layers, highest precedence first:
//! 1. the user's own overrides (stored per machine),
//! 2. the community knowledge base (`knowledge/mods.json`, shipped with WARP),
//! 3. guesses from what's inside the mod's packs and from its Steam metadata.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::model::{ModInfo, WorkshopId};
use crate::pack_index::{Contents, Facet};
use crate::taxonomy::Taxonomy;

/// The knowledge-base file format version this build reads and writes.
pub const KB_FORMAT: u32 = 1;

/// Facts about one mod. Every field is optional so layers can be merged field by field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModKnowledge {
    /// Human-readable title, so the KB file can be reviewed without looking ids up.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factions: Vec<String>,
    /// Units, lords or heroes the mod adds or changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub units: Vec<String>,
    /// Mods this one needs. It will be placed above them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<WorkshopId>,
    /// Mods this one changes (submods, compatibility patches). It will be placed above them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<WorkshopId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incompatible_with: Vec<WorkshopId>,
}

impl ModKnowledge {
    pub fn is_empty(&self) -> bool {
        *self
            == Self {
                title: self.title.clone(),
                ..Self::default()
            }
    }
}

/// The community knowledge base file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub format: u32,
    pub mods: BTreeMap<WorkshopId, ModKnowledge>,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        Self {
            format: KB_FORMAT,
            mods: BTreeMap::new(),
        }
    }

    pub fn parse(json: &str) -> Result<Self, Error> {
        let kb: Self = serde_json::from_str(json).map_err(|e| Error::Format(e.to_string()))?;
        if kb.format > KB_FORMAT {
            return Err(Error::Format(format!(
                "knowledge base format {} is newer than this WARP understands ({KB_FORMAT})",
                kb.format
            )));
        }
        Ok(kb)
    }

    /// Pretty JSON with a trailing newline, stable across runs so diffs stay small.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("knowledge base serializes");
        s.push('\n');
        s
    }
}

/// Where a resolved value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    User,
    Community,
    Heuristic,
    Default,
}

/// The merged view of a mod's knowledge that the rest of WARP works with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    pub tier: String,
    pub tier_source: Source,
    pub role: String,
    pub role_source: Source,
    pub tags: Vec<String>,
    pub factions: Vec<String>,
    pub units: Vec<String>,
    pub requires: Vec<WorkshopId>,
    pub patches: Vec<WorkshopId>,
    pub incompatible_with: Vec<WorkshopId>,
}

/// Merges the layers for one mod. Unknown tier/role keys are ignored rather than trusted.
pub fn resolve(
    taxonomy: &Taxonomy,
    user: Option<&ModKnowledge>,
    community: Option<&ModKnowledge>,
    heuristic: &ModKnowledge,
) -> Resolved {
    let layers = [
        (user, Source::User),
        (community, Source::Community),
        (Some(heuristic), Source::Heuristic),
    ];

    let pick = |get: fn(&ModKnowledge) -> Option<&String>, valid: &dyn Fn(&str) -> bool| {
        layers.iter().find_map(|(layer, source)| {
            layer
                .and_then(get)
                .filter(|v| valid(v))
                .map(|v| (v.clone(), *source))
        })
    };
    let (tier, tier_source) = pick(|k| k.tier.as_ref(), &|v| taxonomy.tier(v).is_some())
        .unwrap_or_else(|| (taxonomy.fallback_tier().key.clone(), Source::Default));
    let (role, role_source) = pick(|k| k.role.as_ref(), &|v| taxonomy.role(v).is_some())
        .unwrap_or_else(|| (taxonomy.default_role().key.clone(), Source::Default));

    // Relations are facts from any source (a pack's own header, the community, the
    // user), so they add up. Descriptive lists come from the highest layer that has one.
    let union = |get: fn(&ModKnowledge) -> &Vec<WorkshopId>| {
        let mut out: Vec<WorkshopId> = Vec::new();
        for id in layers.iter().filter_map(|(l, _)| *l).flat_map(get) {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        out
    };
    let strings = |get: fn(&ModKnowledge) -> &Vec<String>| {
        layers
            .iter()
            .filter_map(|(l, _)| *l)
            .map(get)
            .find(|v| !v.is_empty())
            .cloned()
            .unwrap_or_default()
    };

    Resolved {
        tier,
        tier_source,
        role,
        role_source,
        tags: strings(|k| &k.tags),
        factions: strings(|k| &k.factions),
        units: strings(|k| &k.units),
        requires: union(|k| &k.requires),
        patches: union(|k| &k.patches),
        incompatible_with: union(|k| &k.incompatible_with),
    }
}

/// Guesses tier and role from Steam metadata. Deliberately conservative: a wrong
/// guess is worse than the default.
pub fn heuristic(info: &ModInfo, pack_names: &[String]) -> ModKnowledge {
    let text = format!("{} {}", info.title, pack_names.join(" ")).to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));

    // Titles often say "updated for patch 6.x", so only pack names may use a bare "_patch".
    let role = if has(&[
        "submod",
        "sub-mod",
        "sub mod",
        "compatibility patch",
        "compat patch",
        "_compat",
        "compat_",
        "_patch.pack",
    ]) {
        Some("submod")
    } else if has(&["assets", "resources"]) {
        Some("assets")
    } else if has(&["framework", "mod configuration tool"]) {
        Some("framework")
    } else {
        None
    };

    let tier = steam_tag_tier(info).map(|(_, tier)| tier);

    ModKnowledge {
        tier: tier.map(str::to_owned),
        role: role.map(str::to_owned),
        ..ModKnowledge::default()
    }
}

/// Workshop tags authors pick from a fixed list; the unambiguous ones map to a
/// tier. Returns the tag as the author wrote it, and the tier.
fn steam_tag_tier(info: &ModInfo) -> Option<(&str, &'static str)> {
    const TAG_TIERS: [(&str, &str); 6] = [
        ("ui", "ui"),
        ("units", "units"),
        ("graphical", "graphics"),
        ("overhaul", "overhaul"),
        ("battle", "battle"),
        ("campaign", "campaign"),
    ];
    TAG_TIERS.iter().find_map(|(tag, tier)| {
        info.steam_tags
            .iter()
            .find(|t| t.eq_ignore_ascii_case(tag))
            .map(|t| (t.as_str(), *tier))
    })
}

/// The guessed layer for a mod nobody has described: the tier from its packs'
/// contents when they say something clear, else from its Steam tags. Also says
/// why, in words for the user ("82% of its files are animations").
pub fn guess(
    info: &ModInfo,
    pack_names: &[String],
    contents: Option<&Contents>,
) -> (ModKnowledge, Option<String>) {
    let mut out = heuristic(info, pack_names);
    let asset_pack = out.role.as_deref() == Some("assets");
    if let Some(g) = contents.and_then(|c| classify_contents(c, asset_pack)) {
        out.tier = Some(g.tier.to_owned());
        return (out, Some(g.why));
    }
    let why = steam_tag_tier(info).map(|(tag, _)| format!("its Steam tag is \"{tag}\""));
    (out, why)
}

/// A tier guessed from a pack's contents, and the reason in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guess {
    pub tier: &'static str,
    pub why: String,
}

const UNIT_TABLES: &[&str] = &[
    "land_units",
    "main_units",
    "variants",
    "unit_variants",
    "melee_weapons",
    "missile_weapons",
    "battle_entities",
    "units_custom_battle",
];
const LORD_TABLES: &[&str] = &[
    "agent_subtypes",
    "character_skill_nodes",
    "character_skill_node_sets",
    "faction_agent_permitted_subtypes",
    "start_pos_characters",
];
const MAP_TABLES: &[&str] = &[
    "building_chains",
    "building_levels",
    "regions",
    "settlements",
    "campaign_map",
];
const BATTLE_TABLES: &[&str] = &[
    "special_ability",
    "unit_special_abilities",
    "projectile",
    "battle_vortexs",
    "toggle_system",
];

/// Guesses a tier from a mod's content fingerprint, or `None` when it's unclear.
///
/// The rules come from measuring 415 hand-sorted mods; they agree with the
/// hand-sorted tier about two times in three (`warp classify-report`). Mostly
/// one kind of file decides it; DB-only mods are judged by the tables they touch.
pub fn classify_contents(c: &Contents, asset_pack: bool) -> Option<Guess> {
    if c.files == 0 {
        return None;
    }
    let share = |f: Facet| c.share(f);
    let pct = |f: Facet| format!("{:.0}%", c.share(f) * 100.0);
    let matching = |prefixes: &[&str]| -> Vec<&str> {
        c.tables
            .iter()
            .filter(|t| prefixes.iter().any(|p| t.starts_with(p)))
            .map(|t| t.trim_end_matches("_tables"))
            .collect()
    };
    // A couple of table names, so the reason can be checked against the mod.
    let examples = |names: &[&str]| names.iter().take(2).copied().collect::<Vec<_>>().join(", ");
    let count = |prefixes: &[&str]| matching(prefixes).len();
    let tables = c.tables.len();
    let guess = |tier: &'static str, why: String| Some(Guess { tier, why });

    if share(Facet::Anim) >= 0.6 {
        guess(
            "animations",
            format!("{} of its files are animations", pct(Facet::Anim)),
        )
    } else if c.tables.iter().any(|t| t == "battles_tables") {
        guess("battle_maps", "it adds battle maps".into())
    } else if share(Facet::Terrain) >= 0.6 {
        guess(
            "battle_maps",
            format!("{} of its files are battle terrain", pct(Facet::Terrain)),
        )
    } else if share(Facet::Ui) >= 0.6 {
        guess("ui", format!("{} of its files are UI", pct(Facet::Ui)))
    } else if share(Facet::Audio) >= 0.5 {
        guess(
            "audio",
            format!("{} of its files are sounds", pct(Facet::Audio)),
        )
    } else if share(Facet::Vfx) >= 0.3 {
        guess(
            "graphics",
            format!(
                "{} of its files are visual effects (weather, particles, lighting)",
                pct(Facet::Vfx)
            ),
        )
    } else if share(Facet::Startpos) > 0.0 {
        // A new campaign start position: a total conversion everything else sits on.
        guess(
            "core",
            "it has its own campaign start position, like a total conversion".into(),
        )
    } else if share(Facet::Map) >= 0.05 {
        guess("campaign_map", "it has campaign map files".into())
    } else if share(Facet::Terrain) >= 0.2 && share(Facet::Db) < 0.5 {
        guess(
            "campaign_map",
            format!("{} of its files are terrain", pct(Facet::Terrain)),
        )
    } else if tables >= 80 && count(LORD_TABLES) >= 2 {
        guess(
            "overhaul",
            format!("it changes {tables} DB tables, including lords and heroes"),
        )
    } else if share(Facet::Art) >= 0.5 {
        // Models and textures: a reskin or new units, unless it's a shared asset pack.
        if asset_pack && tables <= 1 {
            guess(
                "core",
                "it's an asset pack: models and textures other mods build on".into(),
            )
        } else {
            guess(
                "units",
                format!("{} of its files are models and textures", pct(Facet::Art)),
            )
        }
    } else if tables == 0 {
        if share(Facet::Script) + share(Facet::Text) > 0.0 {
            guess("campaign", "it only has scripts and text".into())
        } else {
            None
        }
    } else if count(UNIT_TABLES) >= 2 || (count(UNIT_TABLES) >= 1 && share(Facet::Art) >= 0.2) {
        guess(
            "units",
            format!(
                "its DB tables are about units ({})",
                examples(&matching(UNIT_TABLES))
            ),
        )
    } else if count(MAP_TABLES) >= 1 && count(MAP_TABLES) >= count(BATTLE_TABLES) {
        guess(
            "campaign_map",
            format!(
                "its DB tables are about buildings and regions ({})",
                examples(&matching(MAP_TABLES))
            ),
        )
    } else {
        let all: Vec<&str> = c
            .tables
            .iter()
            .map(|t| t.trim_end_matches("_tables"))
            .collect();
        guess(
            "campaign",
            format!("its DB tables change campaign rules ({})", examples(&all)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(tier: Option<&str>, role: Option<&str>) -> ModKnowledge {
        ModKnowledge {
            tier: tier.map(Into::into),
            role: role.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn user_beats_community_beats_heuristic() {
        let t = Taxonomy::builtin();
        let r = resolve(
            &t,
            Some(&k(Some("ui"), None)),
            Some(&k(Some("units"), Some("submod"))),
            &k(Some("battle"), Some("assets")),
        );
        assert_eq!((r.tier.as_str(), r.tier_source), ("ui", Source::User));
        assert_eq!(
            (r.role.as_str(), r.role_source),
            ("submod", Source::Community)
        );
    }

    #[test]
    fn relations_add_up_across_layers() {
        let t = Taxonomy::builtin();
        let user = ModKnowledge {
            requires: vec![WorkshopId(1)],
            ..Default::default()
        };
        let community = ModKnowledge {
            requires: vec![WorkshopId(2), WorkshopId(1)],
            ..Default::default()
        };
        let derived = ModKnowledge {
            requires: vec![WorkshopId(3)],
            ..Default::default()
        };
        let r = resolve(&t, Some(&user), Some(&community), &derived);
        assert_eq!(r.requires, [WorkshopId(1), WorkshopId(2), WorkshopId(3)]);
    }

    #[test]
    fn unknown_keys_fall_through_to_defaults() {
        let t = Taxonomy::builtin();
        let r = resolve(
            &t,
            Some(&k(Some("nonsense"), Some("nope"))),
            None,
            &ModKnowledge::default(),
        );
        assert_eq!((r.tier.as_str(), r.tier_source), ("core", Source::Default));
        assert_eq!(
            (r.role.as_str(), r.role_source),
            ("content", Source::Default)
        );
    }

    #[test]
    fn heuristic_spots_submods_and_ui() {
        let mut info = ModInfo::unknown(WorkshopId(1));
        info.title = "Variant Selector Support for Mixu's mods".into();
        info.steam_tags = vec!["mod".into(), "ui".into()];
        let h = heuristic(
            &info,
            &["!ab_mixu_mods_variant_selector_submod.pack".into()],
        );
        assert_eq!(h.role.as_deref(), Some("submod"));
        assert_eq!(h.tier.as_deref(), Some("ui"));
    }

    fn contents(files: &[&str]) -> Contents {
        Contents::of(&files.iter().map(|f| f.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn contents_decide_the_tier() {
        let guess = |files: &[&str]| classify_contents(&contents(files), false).map(|g| g.tier);
        assert_eq!(
            guess(&["animations/a.anim", "animations/b.anim", "db/x/y"]),
            Some("animations")
        );
        assert_eq!(
            guess(&["ui/skins/a.png", "ui/templates/b.twui.xml"]),
            Some("ui")
        );
        assert_eq!(
            guess(&["audio/wwise/a.wem", "audio/wwise/b.bnk"]),
            Some("audio")
        );
        assert_eq!(
            guess(&["db/battles_tables/m", "text/db/m.loc"]),
            Some("battle_maps")
        );
        assert_eq!(
            guess(&["campaigns/main/startpos.esf", "db/x/y", "db/z/y"]),
            Some("core")
        );
        assert_eq!(
            guess(&["variantmeshes/a.rigid_model_v2", "variantmeshes/a.dds"]),
            Some("units")
        );
        assert_eq!(
            guess(&[
                "db/land_units_tables/m",
                "db/main_units_tables/m",
                "text/db/m.loc"
            ]),
            Some("units")
        );
        assert_eq!(
            guess(&["db/technologies_tables/m", "db/effects_tables/m"]),
            Some("campaign")
        );
        assert_eq!(guess(&["script/campaign/mod/m.lua"]), Some("campaign"));
        assert_eq!(
            guess(&["db/building_chains_tables/m", "db/building_levels_tables/m"]),
            Some("campaign_map")
        );
        assert_eq!(guess(&["readme.md"]), None);
        assert_eq!(guess(&[]), None);
    }

    #[test]
    fn guesses_say_why() {
        let g = classify_contents(
            &contents(&[
                "animations/a.anim",
                "animations/b.anim",
                "animations/c.anim",
                "db/x/y",
            ]),
            false,
        )
        .unwrap();
        assert_eq!(g.why, "75% of its files are animations");
        let g = classify_contents(
            &contents(&[
                "db/land_units_tables/m",
                "db/main_units_tables/m",
                "text/db/m.loc",
            ]),
            false,
        )
        .unwrap();
        assert_eq!(
            g.why,
            "its DB tables are about units (land_units, main_units)"
        );

        let mut info = ModInfo::unknown(WorkshopId(1));
        info.steam_tags = vec!["Graphical".into()];
        let (k, why) = guess(&info, &[], None);
        assert_eq!(k.tier.as_deref(), Some("graphics"));
        assert_eq!(why.as_deref(), Some("its Steam tag is \"Graphical\""));
        assert_eq!(guess(&ModInfo::unknown(WorkshopId(2)), &[], None).1, None);
    }

    #[test]
    fn asset_packs_sit_at_the_bottom() {
        let art = contents(&["variantmeshes/a.dds", "variantmeshes/b.dds"]);
        assert_eq!(classify_contents(&art, true).unwrap().tier, "core");
        assert_eq!(classify_contents(&art, false).unwrap().tier, "units");
    }

    #[test]
    fn content_guess_beats_steam_tags() {
        let mut info = ModInfo::unknown(WorkshopId(1));
        info.steam_tags = vec!["Units".into()];
        let ui = contents(&["ui/a.png", "ui/b.png"]);
        assert_eq!(guess(&info, &[], Some(&ui)).0.tier.as_deref(), Some("ui"));
        assert_eq!(guess(&info, &[], None).0.tier.as_deref(), Some("units"));
        let unclear = contents(&["readme.md"]);
        assert_eq!(
            guess(&info, &[], Some(&unclear)).0.tier.as_deref(),
            Some("units")
        );
    }

    #[test]
    fn kb_round_trips_and_rejects_future_formats() {
        let mut kb = KnowledgeBase::new();
        kb.mods.insert(
            WorkshopId(42),
            ModKnowledge {
                title: "x".into(),
                tier: Some("ui".into()),
                ..Default::default()
            },
        );
        assert_eq!(KnowledgeBase::parse(&kb.to_json()).unwrap(), kb);
        assert!(KnowledgeBase::parse(r#"{"format": 99, "mods": {}}"#).is_err());
    }
}
