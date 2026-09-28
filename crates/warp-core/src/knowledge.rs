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

    // Workshop tags authors pick from a fixed list; map the unambiguous ones.
    let tags: Vec<String> = info.steam_tags.iter().map(|t| t.to_lowercase()).collect();
    let tier = [
        ("ui", "ui"),
        ("units", "units"),
        ("graphical", "graphics"),
        ("overhaul", "overhaul"),
        ("battle", "battle"),
        ("campaign", "campaign"),
    ]
    .iter()
    .find(|(tag, _)| tags.iter().any(|t| t == tag))
    .map(|(_, tier)| *tier);

    ModKnowledge {
        tier: tier.map(str::to_owned),
        role: role.map(str::to_owned),
        ..ModKnowledge::default()
    }
}

/// The guessed layer for a mod nobody has described: the tier from its packs'
/// contents when they say something clear, else from its Steam tags.
pub fn guess(info: &ModInfo, pack_names: &[String], contents: Option<&Contents>) -> ModKnowledge {
    let mut out = heuristic(info, pack_names);
    let asset_pack = out.role.as_deref() == Some("assets");
    if let Some(tier) = contents.and_then(|c| classify_contents(c, asset_pack)) {
        out.tier = Some(tier.to_owned());
    }
    out
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
pub fn classify_contents(c: &Contents, asset_pack: bool) -> Option<&'static str> {
    if c.files == 0 {
        return None;
    }
    let share = |f: Facet| c.share(f);
    let count = |prefixes: &[&str]| {
        c.tables
            .iter()
            .filter(|t| prefixes.iter().any(|p| t.starts_with(p)))
            .count()
    };
    let tables = c.tables.len();

    let tier = if share(Facet::Anim) >= 0.6 {
        "animations"
    } else if share(Facet::Terrain) >= 0.6 || c.tables.iter().any(|t| t == "battles_tables") {
        "battle_maps"
    } else if share(Facet::Ui) >= 0.6 {
        "ui"
    } else if share(Facet::Audio) >= 0.5 {
        "audio"
    } else if share(Facet::Vfx) >= 0.3 {
        "graphics"
    } else if share(Facet::Startpos) > 0.0 {
        // A new campaign start position: a total conversion everything else sits on.
        "core"
    } else if share(Facet::Map) >= 0.05 || (share(Facet::Terrain) >= 0.2 && share(Facet::Db) < 0.5)
    {
        "campaign_map"
    } else if tables >= 80 && count(LORD_TABLES) >= 2 {
        "overhaul"
    } else if share(Facet::Art) >= 0.5 {
        // Models and textures: a reskin or new units, unless it's a shared asset pack.
        if asset_pack && tables <= 1 {
            "core"
        } else {
            "units"
        }
    } else if tables == 0 {
        if share(Facet::Script) + share(Facet::Text) > 0.0 {
            "campaign"
        } else {
            return None;
        }
    } else if count(UNIT_TABLES) >= 2 || (count(UNIT_TABLES) >= 1 && share(Facet::Art) >= 0.2) {
        "units"
    } else if count(MAP_TABLES) >= 1 && count(MAP_TABLES) >= count(BATTLE_TABLES) {
        "campaign_map"
    } else {
        "campaign"
    };
    Some(tier)
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
        let guess = |files: &[&str]| classify_contents(&contents(files), false);
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
    fn asset_packs_sit_at_the_bottom() {
        let art = contents(&["variantmeshes/a.dds", "variantmeshes/b.dds"]);
        assert_eq!(classify_contents(&art, true), Some("core"));
        assert_eq!(classify_contents(&art, false), Some("units"));
    }

    #[test]
    fn content_guess_beats_steam_tags() {
        let mut info = ModInfo::unknown(WorkshopId(1));
        info.steam_tags = vec!["Units".into()];
        let ui = contents(&["ui/a.png", "ui/b.png"]);
        assert_eq!(guess(&info, &[], Some(&ui)).tier.as_deref(), Some("ui"));
        assert_eq!(guess(&info, &[], None).tier.as_deref(), Some("units"));
        let unclear = contents(&["readme.md"]);
        assert_eq!(
            guess(&info, &[], Some(&unclear)).tier.as_deref(),
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
