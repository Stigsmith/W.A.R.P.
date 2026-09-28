//! Finding Warhammer III and its Workshop mods on disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::model::{WorkshopId, is_pack_name};
use crate::steam::APP_ID;

/// Where the game and its downloaded Workshop mods live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Install {
    /// `...\steamapps\common\Total War WARHAMMER III`
    pub game_dir: PathBuf,
    /// `...\steamapps\workshop\content\1142710`, one folder per mod.
    pub workshop_dir: PathBuf,
    /// `...\steamapps\workshop\appworkshop_1142710.acf`, Steam's record of installed mods.
    pub manifest: PathBuf,
}

/// A Workshop mod as installed on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledItem {
    pub id: WorkshopId,
    /// The version on disk (Steam's `timeupdated` when it was downloaded).
    pub time_updated: i64,
    pub size: u64,
    /// Full paths of the `.pack` files in the mod's folder.
    pub packs: Vec<PathBuf>,
}

impl Install {
    /// Finds the game through Steam's own records.
    pub fn locate() -> Result<Self, Error> {
        let steam = steamlocate::SteamDir::locate()
            .map_err(|e| Error::Install(format!("Steam wasn't found: {e}")))?;
        let (app, library) = steam
            .find_app(APP_ID)
            .map_err(|e| Error::Install(format!("couldn't read Steam's libraries: {e}")))?
            .ok_or_else(|| {
                Error::Install(
                    "Total War: WARHAMMER III isn't installed in any Steam library".into(),
                )
            })?;
        Ok(Self::in_library(
            library.path(),
            &library.resolve_app_dir(&app),
        ))
    }

    /// The layout inside a given Steam library folder.
    pub fn in_library(library: &Path, game_dir: &Path) -> Self {
        let workshop = library.join("steamapps").join("workshop");
        Self {
            game_dir: game_dir.to_path_buf(),
            workshop_dir: workshop.join("content").join(APP_ID.to_string()),
            manifest: workshop.join(format!("appworkshop_{APP_ID}.acf")),
        }
    }

    pub fn game_exe(&self) -> PathBuf {
        self.game_dir.join("Warhammer3.exe")
    }

    pub fn data_dir(&self) -> PathBuf {
        self.game_dir.join("data")
    }

    /// The mods Steam has installed, with their pack files.
    pub fn installed_items(&self) -> Result<Vec<InstalledItem>, Error> {
        let text = std::fs::read_to_string(&self.manifest)
            .map_err(|e| Error::Install(format!("can't read {}: {e}", self.manifest.display())))?;
        let manifest = parse_vdf(&text)?;
        let installed = manifest
            .get("AppWorkshop")
            .and_then(|w| w.get("WorkshopItemsInstalled"))
            .and_then(Vdf::entries)
            .unwrap_or_default();

        let mut items = Vec::with_capacity(installed.len());
        for (key, value) in installed {
            let Ok(id) = key.parse::<WorkshopId>() else {
                continue;
            };
            let num = |name: &str| {
                value
                    .get(name)
                    .and_then(Vdf::as_str)
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0)
            };
            let dir = self.workshop_dir.join(key);
            let mut packs: Vec<PathBuf> = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .map(|e| e.path())
                        .filter(|p| {
                            p.file_name()
                                .and_then(|n| n.to_str())
                                .is_some_and(is_pack_name)
                        })
                        .collect()
                })
                .unwrap_or_default();
            packs.sort();
            items.push(InstalledItem {
                id,
                time_updated: num("timeupdated"),
                size: num("size").max(0) as u64,
                packs,
            });
        }
        Ok(items)
    }
}

/// A Valve KeyValues (VDF) value: a string or an ordered list of children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vdf {
    Str(String),
    Map(Vec<(String, Vdf)>),
}

impl Vdf {
    /// First child with this key (case-insensitive, as Steam treats keys).
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(entries) => entries
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn entries(&self) -> Option<Vec<(&str, &Vdf)>> {
        match self {
            Vdf::Map(entries) => Some(entries.iter().map(|(k, v)| (k.as_str(), v)).collect()),
            Vdf::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Map(_) => None,
        }
    }
}

/// Parses Steam's text KeyValues format (`.acf`, `.vdf`).
pub fn parse_vdf(text: &str) -> Result<Vdf, Error> {
    let mut tokens = Tokens {
        chars: text.chars().peekable(),
    };
    let entries = parse_entries(&mut tokens, false)?;
    Ok(Vdf::Map(entries))
}

struct Tokens<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

enum Token {
    Str(String),
    Open,
    Close,
}

impl Tokens<'_> {
    fn next(&mut self) -> Result<Option<Token>, Error> {
        loop {
            match self.chars.peek() {
                None => return Ok(None),
                Some(c) if c.is_whitespace() => {
                    self.chars.next();
                }
                Some('/') => {
                    // `// comment` to end of line.
                    for c in self.chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('{') => {
                    self.chars.next();
                    return Ok(Some(Token::Open));
                }
                Some('}') => {
                    self.chars.next();
                    return Ok(Some(Token::Close));
                }
                Some('"') => {
                    self.chars.next();
                    let mut s = String::new();
                    loop {
                        match self.chars.next() {
                            None => return Err(Error::Format("unterminated string in VDF".into())),
                            Some('"') => break,
                            Some('\\') => match self.chars.next() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some(c) => s.push(c),
                                None => {
                                    return Err(Error::Format("unterminated escape in VDF".into()));
                                }
                            },
                            Some(c) => s.push(c),
                        }
                    }
                    return Ok(Some(Token::Str(s)));
                }
                Some(_) => {
                    // Unquoted token.
                    let mut s = String::new();
                    while let Some(&c) = self.chars.peek() {
                        if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                            break;
                        }
                        s.push(c);
                        self.chars.next();
                    }
                    return Ok(Some(Token::Str(s)));
                }
            }
        }
    }
}

fn parse_entries(tokens: &mut Tokens<'_>, nested: bool) -> Result<Vec<(String, Vdf)>, Error> {
    let mut entries = Vec::new();
    loop {
        let key = match tokens.next()? {
            None if !nested => return Ok(entries),
            None => return Err(Error::Format("unexpected end of VDF".into())),
            Some(Token::Close) if nested => return Ok(entries),
            Some(Token::Close) | Some(Token::Open) => {
                return Err(Error::Format("malformed VDF".into()));
            }
            Some(Token::Str(k)) => k,
        };
        let value = match tokens.next()? {
            Some(Token::Str(v)) => Vdf::Str(v),
            Some(Token::Open) => Vdf::Map(parse_entries(tokens, true)?),
            _ => return Err(Error::Format(format!("VDF key '{key}' has no value"))),
        };
        entries.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
"AppWorkshop"
{
	"appid"		"1142710"
	"WorkshopItemsInstalled"
	{
		"2789853654"
		{
			"size"		"374560"
			"timeupdated"		"1790356027"
			"manifest"		"8769817087481082213"
		}
		"notanid" { "size" "1" }
	}
	// a comment
	"WorkshopItemDetails" { "2789853654" { "latest_timeupdated" "1790356027" } }
}
"#;

    #[test]
    fn parses_nested_vdf() {
        let v = parse_vdf(MANIFEST).unwrap();
        let items = v
            .get("appworkshop")
            .unwrap()
            .get("WorkshopItemsInstalled")
            .unwrap();
        let item = items.get("2789853654").unwrap();
        assert_eq!(
            item.get("timeupdated").unwrap().as_str(),
            Some("1790356027")
        );
        assert_eq!(items.entries().unwrap().len(), 2);
    }

    #[test]
    fn escapes_and_errors() {
        let v = parse_vdf(r#""k" "a \"quoted\" \\ path""#).unwrap();
        assert_eq!(v.get("k").unwrap().as_str(), Some(r#"a "quoted" \ path"#));
        assert!(parse_vdf(r#""k" { "a" "b""#).is_err());
        assert!(parse_vdf(r#""k""#).is_err());
    }

    #[test]
    fn reads_installed_items_and_their_packs() {
        let root = std::env::temp_dir().join(format!("warp-install-test-{}", std::process::id()));
        let install = Install::in_library(&root, &root.join("game"));
        let mod_dir = install.workshop_dir.join("2789853654");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("b.pack"), b"").unwrap();
        std::fs::write(mod_dir.join("a.pack"), b"").unwrap();
        std::fs::write(mod_dir.join("thumb.png"), b"").unwrap();
        std::fs::write(&install.manifest, MANIFEST).unwrap();

        let items = install.installed_items().unwrap();
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(items.len(), 1, "non-numeric keys are skipped");
        let item = &items[0];
        assert_eq!(item.id, WorkshopId(2_789_853_654));
        assert_eq!(item.time_updated, 1_790_356_027);
        let names: Vec<_> = item
            .packs
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["a.pack", "b.pack"]);
    }
}
