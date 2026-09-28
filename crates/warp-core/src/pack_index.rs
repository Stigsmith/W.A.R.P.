//! Pack X-ray: what's inside each `.pack`, read with `rpfm_lib` (the library behind
//! RPFM, the community's reference pack tool). Only the index is read, never file contents.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use rpfm_lib::files::Container;
use rpfm_lib::files::pack::Pack;
use rpfm_lib::games::GameInfo;
use rpfm_lib::games::supported_games::SupportedGames;
use serde::{Deserialize, Serialize};

use crate::Error;

/// Bump when what [`read`] extracts changes, so cached indexes are re-read.
pub const INDEX_VERSION: i64 = 3;

/// What a pack contains, as far as load order is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackIndex {
    /// File name, e.g. `!proper_chaos_units.pack`.
    pub name: String,
    pub size: u64,
    /// Last-modified time of the pack file (unix seconds); with `size`, decides if the cache is stale.
    pub modified: i64,
    /// `mod`, `movie`, `boot`, `release` or `patch`, from the pack header.
    pub kind: String,
    /// Packs this one declares it needs (from its header), e.g. a submod naming its parent.
    pub dependencies: Vec<String>,
    /// Every file path in the pack, lowercase, `/`-separated, sorted.
    pub files: Vec<String>,
}

impl PackIndex {
    /// The pack's content fingerprint.
    pub fn contents(&self) -> Contents {
        Contents::of(&self.files)
    }

    /// The file list packed for storage.
    pub fn files_blob(&self) -> Vec<u8> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(self.files.join("\n").as_bytes())
            .expect("writing to a Vec cannot fail");
        enc.finish().expect("writing to a Vec cannot fail")
    }

    pub fn files_from_blob(blob: &[u8]) -> Result<Vec<String>, Error> {
        let mut text = String::new();
        ZlibDecoder::new(blob)
            .read_to_string(&mut text)
            .map_err(|e| Error::Format(format!("damaged pack index: {e}")))?;
        Ok(if text.is_empty() {
            Vec::new()
        } else {
            text.split('\n').map(str::to_owned).collect()
        })
    }
}

/// The kinds of content that matter when two packs ship the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    /// `db/<table>/<file>`: a whole table file. Only one pack's copy is used.
    DbTable,
    /// Campaign start positions. Only one survives, and it defines the whole campaign.
    Startpos,
    /// Lua scripts and their data.
    Script,
    /// Localisation (`text/`).
    Text,
    /// Campaign map data (`campaign_maps/`).
    Map,
    /// Interface layouts and images.
    Ui,
    /// Models, textures, animations, effects, sound, video.
    Art,
    Other,
}

/// A DB table file (`db/<table>/<file>`), as opposed to a note left in `db/`.
pub fn is_db_file(path: &str) -> bool {
    path.strip_prefix("db/")
        .is_some_and(|rest| rest.contains('/'))
}

/// Finer content groups than [`ContentKind`], for guessing what kind of mod a pack is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facet {
    Db,
    Script,
    Text,
    Ui,
    Map,
    Startpos,
    Anim,
    Terrain,
    Audio,
    Vfx,
    Art,
    Other,
}

impl Facet {
    pub fn of(path: &str) -> Self {
        if path.ends_with("startpos.esf") {
            return Self::Startpos;
        }
        let top = path.split('/').next().unwrap_or("");
        let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
        match top {
            "db" => Self::Db,
            "script" | "script_data" => Self::Script,
            "text" => Self::Text,
            "ui" => Self::Ui,
            "campaign_maps" => Self::Map,
            "animations" => Self::Anim,
            "battleterrain" | "terrain" | "prefabs" => Self::Terrain,
            "audio" | "audioprojects" => Self::Audio,
            "weather" | "lut" | "particles" | "vfx" | "skyboxes" | "shaders" => Self::Vfx,
            _ if matches!(ext, "anim" | "frg") => Self::Anim,
            _ if matches!(ext, "wem" | "bnk") => Self::Audio,
            _ if ContentKind::of(path) == ContentKind::Art => Self::Art,
            _ => Self::Other,
        }
    }
}

/// A pack's content fingerprint: files per [`Facet`] and the DB tables it touches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Contents {
    pub files: usize,
    pub facets: BTreeMap<Facet, usize>,
    /// Distinct DB table folders, e.g. `land_units_tables`, sorted.
    pub tables: Vec<String>,
}

impl Contents {
    pub fn of(files: &[String]) -> Self {
        let mut out = Self {
            files: files.len(),
            ..Self::default()
        };
        let mut tables = std::collections::BTreeSet::new();
        for f in files {
            *out.facets.entry(Facet::of(f)).or_insert(0) += 1;
            if is_db_file(f)
                && let Some((table, _)) = f[3..].split_once('/')
            {
                tables.insert(table.to_owned());
            }
        }
        out.tables = tables.into_iter().collect();
        out
    }

    /// Several packs (one mod) as one fingerprint.
    pub fn merge<'a>(parts: impl IntoIterator<Item = &'a Contents>) -> Self {
        let mut out = Self::default();
        let mut tables = std::collections::BTreeSet::new();
        for c in parts {
            out.files += c.files;
            for (k, n) in &c.facets {
                *out.facets.entry(*k).or_insert(0) += n;
            }
            tables.extend(c.tables.iter().cloned());
        }
        out.tables = tables.into_iter().collect();
        out
    }

    /// Share of files in this facet, 0..=1.
    pub fn share(&self, facet: Facet) -> f64 {
        if self.files == 0 {
            0.0
        } else {
            self.facets.get(&facet).copied().unwrap_or(0) as f64 / self.files as f64
        }
    }
}

/// Top-level folders that only hold assets.
const ASSET_FOLDERS: &[&str] = &[
    "animations",
    "art",
    "audio",
    "audioprojects",
    "battleterrain",
    "commontextures",
    "composite_scene",
    "lut",
    "materials",
    "models",
    "movies",
    "particles",
    "portraits",
    "prefabs",
    "projectiles",
    "rigidmodels",
    "shaders",
    "skyboxes",
    "terrain",
    "textures",
    "variantmeshes",
    "vfx",
    "warmachines",
    "weather",
];

/// File types that are assets wherever they sit (outside `ui/`).
const ASSET_EXTENSIONS: &[&str] = &[
    "anim",
    "bnk",
    "ca_vp8",
    "dds",
    "frg",
    "jpg",
    "material",
    "png",
    "rigid_model_v2",
    "tga",
    "variantmeshdefinition",
    "wem",
    "wsmodel",
];

impl ContentKind {
    pub fn of(path: &str) -> Self {
        if path.ends_with("startpos.esf") {
            return Self::Startpos;
        }
        let top = path.split('/').next().unwrap_or("");
        let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
        match top {
            "db" => Self::DbTable,
            "script" | "script_data" => Self::Script,
            "text" => Self::Text,
            "campaign_maps" => Self::Map,
            "ui" => Self::Ui,
            _ if ASSET_FOLDERS.contains(&top) || ASSET_EXTENSIONS.contains(&ext) => Self::Art,
            _ => Self::Other,
        }
    }
}

/// Reads a pack's index from disk.
pub fn read(path: &Path) -> Result<PackIndex, Error> {
    let meta = std::fs::metadata(path)?;
    let pack = Pack::read_and_merge(&[path.to_path_buf()], wh3(), true, false, false)
        .map_err(|e| Error::Pack(format!("{}: {e}", path.display())))?;
    let mut files: Vec<String> = pack
        .paths_raw()
        .into_iter()
        .map(|p| p.replace('\\', "/").to_lowercase())
        .filter(|p| !is_tool_file(p))
        .collect();
    files.sort_unstable();
    files.dedup();
    Ok(PackIndex {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: meta.len(),
        modified: modified_secs(&meta),
        kind: format!("{:?}", pack.pfh_file_type()).to_lowercase(),
        // Hard dependencies are written into the pack header, which the game reads;
        // soft ones are only notes RPFM keeps inside the pack.
        dependencies: pack
            .dependencies()
            .iter()
            .filter(|(hard, _)| *hard)
            .map(|(_, name)| name.clone())
            .collect(),
        files,
    })
}

pub fn modified_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

/// Files the game never loads: modding-tool leftovers and modders' notes
/// (`whmm_update.txt`, `text/patch_notes.txt`). Only localisation `.loc` files count in `text/`.
fn is_tool_file(path: &str) -> bool {
    let note = path.ends_with(".txt") || path.ends_with(".md");
    path.ends_with(".rpfm_reserved") || (note && (!path.contains('/') || path.starts_with("text/")))
}

fn wh3() -> &'static GameInfo {
    static GAMES: OnceLock<SupportedGames> = OnceLock::new();
    GAMES
        .get_or_init(SupportedGames::default)
        .game("warhammer_3")
        .expect("rpfm_lib knows Warhammer III")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_kinds() {
        assert_eq!(
            ContentKind::of("db/units_tables/data__"),
            ContentKind::DbTable
        );
        assert_eq!(
            ContentKind::of("campaigns/wh3_main_combi/startpos.esf"),
            ContentKind::Startpos
        );
        assert_eq!(
            ContentKind::of("script/campaign/mod/my_mod.lua"),
            ContentKind::Script
        );
        assert_eq!(ContentKind::of("text/db/my_mod.loc"), ContentKind::Text);
        assert_eq!(
            ContentKind::of("ui/skins/default/icon.png"),
            ContentKind::Ui
        );
        assert_eq!(
            ContentKind::of("variantmeshes/wh_variantmodels/x.rigid_model_v2"),
            ContentKind::Art
        );
        assert_eq!(
            ContentKind::of("rigidmodels/campaign/decals/a.rigid_model_v2"),
            ContentKind::Art
        );
        assert_eq!(
            ContentKind::of("1/odd_folder/texture.dds"),
            ContentKind::Art,
            "assets by file type"
        );
        assert_eq!(
            ContentKind::of("campaign_maps/main_warhammer/map.xml"),
            ContentKind::Map
        );
        assert_eq!(ContentKind::of("script_data/x.lua"), ContentKind::Script);
        assert_eq!(ContentKind::of("settings.xml"), ContentKind::Other);
    }

    #[test]
    fn files_blob_round_trips() {
        let idx = PackIndex {
            name: "a.pack".into(),
            size: 1,
            modified: 2,
            kind: "mod".into(),
            dependencies: vec![],
            files: vec!["db/a/b".into(), "ui/c.png".into()],
        };
        assert_eq!(
            PackIndex::files_from_blob(&idx.files_blob()).unwrap(),
            idx.files
        );
        let empty = PackIndex {
            files: vec![],
            ..idx
        };
        assert!(
            PackIndex::files_from_blob(&empty.files_blob())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn notes_and_tool_files_are_ignored() {
        assert!(is_tool_file("whmm_update.txt"));
        assert!(is_tool_file("text/patch_notes.txt"));
        assert!(is_tool_file("settings.rpfm_reserved"));
        assert!(!is_tool_file("text/db/my_mod.loc"));
        assert!(
            !is_tool_file("script/readme_parser/data.txt"),
            "nested data files still count"
        );
    }

    #[test]
    fn contents_fingerprint() {
        let files: Vec<String> = [
            "db/land_units_tables/my_mod",
            "db/land_units_tables/my_mod_2",
            "db/main_units_tables/my_mod",
            "animations/battle/x.anim",
            "variantmeshes/a.dds",
            "campaigns/wh3_main_combi/startpos.esf",
        ]
        .map(String::from)
        .to_vec();
        let c = Contents::of(&files);
        assert_eq!(c.files, 6);
        assert_eq!(c.tables, ["land_units_tables", "main_units_tables"]);
        assert_eq!(c.facets[&Facet::Db], 3);
        assert!((c.share(Facet::Db) - 0.5).abs() < 1e-9);
        assert_eq!(c.facets[&Facet::Startpos], 1);
        let both = Contents::merge([&c, &c]);
        assert_eq!(both.files, 12);
        assert_eq!(both.tables.len(), 2);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Contents>(&json).unwrap(), c);
        assert_eq!(
            serde_json::from_str::<Contents>("{}").unwrap(),
            Contents::default()
        );
    }

    #[test]
    fn missing_pack_is_an_error() {
        assert!(read(Path::new("definitely/not/here.pack")).is_err());
    }
}
