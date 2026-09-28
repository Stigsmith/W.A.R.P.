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
pub const INDEX_VERSION: i64 = 2;

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
    /// Files per kind of content, the pack's "fingerprint".
    pub fn contents(&self) -> BTreeMap<ContentKind, usize> {
        let mut out = BTreeMap::new();
        for f in &self.files {
            *out.entry(ContentKind::of(f)).or_insert(0) += 1;
        }
        out
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
    fn missing_pack_is_an_error() {
        assert!(read(Path::new("definitely/not/here.pack")).is_err());
    }
}
