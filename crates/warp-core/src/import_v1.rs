//! One-time import of a W.A.R.P. v1 workbook (`WARP Database.xlsm`, or a zip holding it).
//!
//! v1 packed three ideas into two columns: Category became the **tier**, and
//! Subcategory mixed the **role** (Submod, Fixes, Assets, Framework) with a
//! **topic** (Fog, Diplomacy, Camera 2). The topic becomes a tag, since it never
//! mattered for load order.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};

use crate::Error;
use crate::knowledge::ModKnowledge;
use crate::model::{ModInfo, WorkshopId};

#[derive(Debug, Clone)]
pub struct V1Mod {
    pub info: ModInfo,
    pub pack: String,
    pub knowledge: ModKnowledge,
    /// v1's single "Component" (now a personal set).
    pub component: Option<String>,
    /// v1 marked unsubscribed mods with an `x` in the Archive column.
    pub archived: bool,
    /// The raw v1 Category and Subcategory cells, kept for comparison with v1's ordering.
    pub source_category: String,
    pub source_subcategory: String,
}

#[derive(Debug, Clone)]
pub struct V1Import {
    pub mods: Vec<V1Mod>,
    /// The Profile Builder sheet: profile name and its components.
    pub profile: Option<(String, Vec<String>)>,
    /// Dependency cells that didn't match any mod in the workbook: (pack, text).
    pub unresolved_dependencies: Vec<(String, String)>,
}

/// Reads a v1 workbook from an `.xlsm`/`.xlsx`, or from a `.zip` containing one.
pub fn read(path: &Path) -> Result<V1Import, Error> {
    let bytes = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|e| Error::Excel(e.to_string()))?;
        let name = zip
            .file_names()
            .find(|n| n.to_lowercase().ends_with(".xlsm") || n.to_lowercase().ends_with(".xlsx"))
            .map(str::to_owned)
            .ok_or_else(|| Error::Excel("the zip contains no .xlsm/.xlsx workbook".into()))?;
        let mut entry = zip.by_name(&name).map_err(|e| Error::Excel(e.to_string()))?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        buf
    } else {
        std::fs::read(path)?
    };
    parse(bytes)
}

fn parse(bytes: Vec<u8>) -> Result<V1Import, Error> {
    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).map_err(|e: calamine::XlsxError| Error::Excel(e.to_string()))?;
    let dump = wb.worksheet_range("Dump").map_err(|e: calamine::XlsxError| Error::Excel(format!("sheet 'Dump': {e}")))?;
    let mut rows = dump.rows();
    let header = rows.next().ok_or_else(|| Error::Excel("sheet 'Dump' is empty".into()))?;
    let col: HashMap<String, usize> = header.iter().enumerate().map(|(i, c)| (text(c), i)).collect();
    let need = |name: &str| col.get(name).copied().ok_or_else(|| Error::Excel(format!("column '{name}' not found")));
    let (c_id, c_pack, c_title) = (need("PublishedFileId")?, need("Pack_File")?, need("Title")?);
    let get = |row: &[Data], name: &str| col.get(name).and_then(|&i| row.get(i)).map(text).unwrap_or_default();
    let get_num = |row: &[Data], name: &str| col.get(name).and_then(|&i| row.get(i)).and_then(number);

    let mut mods = Vec::new();
    let mut dependency_text = Vec::new();
    for row in rows {
        let Some(id) = row.get(c_id).and_then(number).filter(|n| *n > 0.0).map(|n| WorkshopId(n as u64)) else {
            continue;
        };
        let pack = row.get(c_pack).map(text).unwrap_or_default();
        let (source_category, source_subcategory) = (get(row, "Category"), get(row, "Subcategory"));
        let (role, mut tags) = map_subcategory(&source_subcategory);
        tags.extend(split_list(&get(row, "Affected systems")));
        dedup(&mut tags);

        let info = ModInfo {
            id,
            title: row.get(c_title).map(text).unwrap_or_default(),
            description: get(row, "Description"),
            steam_tags: split_list(&get(row, "Tags")),
            file_size: get_num(row, "FileSize_MB").map_or(0, |mb| (mb * 1_048_576.0) as u64),
            time_created: get_num(row, "TimeCreated").map_or(0, excel_to_unix),
            time_updated: get_num(row, "TimeUpdated").map_or(0, excel_to_unix),
            preview_url: get(row, "Preview_URL"),
            subscriptions: get_num(row, "Subscribers").unwrap_or(0.0) as u64,
            favorited: get_num(row, "Favorites").unwrap_or(0.0) as u64,
            views: get_num(row, "Views").unwrap_or(0.0) as u64,
            // v1 stored Steam's result code; 1 means OK. Blank means it was never fetched.
            available: get_num(row, "API_Result").is_none_or(|r| r as i64 == 1),
        };
        let knowledge = ModKnowledge {
            title: info.title.clone(),
            tier: map_category(&source_category).map(str::to_owned),
            role: role.map(str::to_owned),
            tags,
            factions: split_list(&get(row, "Faction(s)")),
            units: split_list(&get(row, "Unit(s)")),
            ..ModKnowledge::default()
        };
        let dep = get(row, "Dependency");
        if !dep.is_empty() {
            dependency_text.push((mods.len(), dep));
        }
        let component = Some(get(row, "Component")).filter(|c| !c.is_empty());
        let archived = get(row, "Archive").eq_ignore_ascii_case("x");
        mods.push(V1Mod { info, pack, knowledge, component, archived, source_category, source_subcategory });
    }

    let unresolved_dependencies = resolve_dependencies(&mut mods, dependency_text);
    let profile = read_profile_builder(&mut wb);
    Ok(V1Import { mods, profile, unresolved_dependencies })
}

/// Turns v1's free-text Dependency cells into links between mods. Submods get
/// `patches`, everything else `requires`; both place the mod above its target.
fn resolve_dependencies(mods: &mut [V1Mod], deps: Vec<(usize, String)>) -> Vec<(String, String)> {
    let mut exact: HashMap<String, WorkshopId> = HashMap::new();
    for m in mods.iter() {
        exact.insert(normalize(&m.pack), m.info.id);
        exact.entry(normalize(&m.info.title)).or_insert(m.info.id);
    }
    let mut unresolved = Vec::new();
    for (i, text) in deps {
        let key = normalize(&text);
        let found = exact.get(&key).copied().or_else(|| {
            // Fall back to a unique title containing the text.
            let mut hits = mods.iter().filter(|m| normalize(&m.info.title).contains(&key)).map(|m| m.info.id);
            match (hits.next(), hits.next()) {
                (Some(id), None) => Some(id),
                _ => None,
            }
        });
        match found.filter(|&id| id != mods[i].info.id) {
            Some(target) => {
                let k = &mut mods[i].knowledge;
                if k.role.as_deref() == Some("submod") { k.patches.push(target) } else { k.requires.push(target) }
            }
            None => unresolved.push((mods[i].pack.clone(), text)),
        }
    }
    unresolved
}

fn read_profile_builder<R: std::io::Read + std::io::Seek>(wb: &mut Xlsx<R>) -> Option<(String, Vec<String>)> {
    let sheet = wb.worksheet_range("Profile Builder").ok()?;
    let mut name = None;
    let mut components = Vec::new();
    for row in sheet.rows() {
        let label = row.first().map(text).unwrap_or_default().to_lowercase();
        let value = row.get(1).map(text).unwrap_or_default();
        if label == "profile name" {
            name = Some(value.strip_prefix("profile_").unwrap_or(&value).to_owned());
        } else if label.starts_with("component") && !value.is_empty() {
            components.push(value);
        }
    }
    Some((name.filter(|n| !n.is_empty())?, components))
}

/// v1 Category -> tier key. Categories carried a numeric prefix ("03_Units").
pub fn map_category(category: &str) -> Option<&'static str> {
    Some(match strip_number(category).to_lowercase().as_str() {
        "core tools" | "core" => "core",
        "overhaul" => "overhaul",
        "campaign" => "campaign",
        "units" | "unit" => "units",
        "campaign map" => "campaign_map",
        "animations" => "animations",
        "graphics" => "graphics",
        "battle" => "battle",
        "battlemaps" | "battle maps" => "battle_maps",
        "ui" => "ui",
        "audio" => "audio",
        "experimental" => "experimental",
        _ => return None,
    })
}

/// v1 Subcategory -> (role key, tags).
pub fn map_subcategory(subcategory: &str) -> (Option<&'static str>, Vec<String>) {
    let name = strip_number(subcategory);
    let tag = |t: &str| vec![t.to_owned()];
    match name.to_lowercase().as_str() {
        "" => (None, vec![]),
        "submod" => (Some("submod"), vec![]),
        "fixes" | "fix" => (Some("fix"), vec![]),
        "assets" => (Some("assets"), vec![]),
        "core" | "core tools" | "framework" => (Some("framework"), vec![]),
        "core graphics" => (Some("framework"), tag("Graphics")),
        "game mode" => (Some("framework"), tag("Game mode")),
        "tooling" => (Some("framework"), tag("Tooling")),
        "experimental" => (None, tag("Experimental")),
        "ai behavior" | "ai behaviour" => (None, tag("AI behaviour")),
        "qol" => (None, tag("QoL")),
        "camera" | "camera 1" | "camera 2" => (None, tag("Camera")),
        "second" => (None, tag("Secondary")),
        _ => (None, tag(name)),
    }
}

/// "03_Units" -> "Units". Leaves names without a numeric prefix alone.
fn strip_number(s: &str) -> &str {
    let s = s.trim();
    match s.split_once('_') {
        Some((n, rest)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => rest.trim(),
        _ => s,
    }
}

/// Lowercase, no `.pack`, no leading `!@_` noise, runs of punctuation collapsed to one space.
fn normalize(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_suffix(".pack").or_else(|| s.strip_suffix(".PACK")).unwrap_or(s).to_lowercase();
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

fn dedup(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|t| seen.insert(t.to_lowercase()));
}

fn text(d: &Data) -> String {
    match d {
        Data::String(s) => s.trim().to_owned(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => d.as_f64().to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(_) | Data::Empty => String::new(),
    }
}

fn number(d: &Data) -> Option<f64> {
    match d {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::DateTime(d) => Some(d.as_f64()),
        Data::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Excel serial date (days since 1899-12-30) to unix seconds. v1 wrote UTC.
fn excel_to_unix(serial: f64) -> i64 {
    ((serial - 25_569.0) * 86_400.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_map_to_tiers() {
        assert_eq!(map_category("00_Core Tools"), Some("core"));
        assert_eq!(map_category("04_Campaign Map"), Some("campaign_map"));
        assert_eq!(map_category("08_BattleMaps"), Some("battle_maps"));
        assert_eq!(map_category("11_Experimental"), Some("experimental"));
        assert_eq!(map_category(""), None);
    }

    #[test]
    fn subcategories_split_into_role_and_topic() {
        assert_eq!(map_subcategory("08_Submod"), (Some("submod"), vec![]));
        assert_eq!(map_subcategory("09_Submod"), (Some("submod"), vec![]));
        assert_eq!(map_subcategory("07_Fixes"), (Some("fix"), vec![]));
        assert_eq!(map_subcategory("00_Assets"), (Some("assets"), vec![]));
        assert_eq!(map_subcategory("04_Framework"), (Some("framework"), vec![]));
        assert_eq!(map_subcategory("03_AI Behaviour"), (None, vec!["AI behaviour".to_owned()]));
        assert_eq!(map_subcategory("04_AI Behavior"), (None, vec!["AI behaviour".to_owned()]));
        assert_eq!(map_subcategory("06_Camera 2"), (None, vec!["Camera".to_owned()]));
        assert_eq!(map_subcategory("02_Traits & Technologies"), (None, vec!["Traits & Technologies".to_owned()]));
    }

    #[test]
    fn normalize_matches_pack_names_to_titles() {
        assert_eq!(normalize("ZC_MODDING_ASSETS.pack"), normalize("ZC MODDING ASSETS"));
        assert_eq!(normalize("!wh1_texture_proj.pack"), "wh1 texture proj");
    }

    #[test]
    fn excel_dates() {
        // 2023-01-01 00:00:00 UTC
        assert_eq!(excel_to_unix(44_927.0), 1_672_531_200);
    }
}
