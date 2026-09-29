//! Starting the game with a modlist.
//!
//! The game reads its mods from a text file named on its command line, run from
//! the game folder: `Warhammer3.exe warp_mods.txt;`. Each mod folder is added with
//! `add_working_directory "<dir>";`, then `mod "<pack>";` lines list the packs,
//! top of the load order first. WARP writes its own file so it never overwrites the
//! `used_mods.txt` of CA's launcher or mod manager.
//!
//! A game started outside Steam can hand itself back to Steam (after a patch, until
//! it has once run from Steam). Steam then asks about "custom arguments" and opens
//! CA's launcher with nothing ticked. Valve's `steam_appid.txt` next to the exe, and
//! the app id Steam itself sets in the environment, let the game run on its own.

use std::path::PathBuf;
use std::process::Command;

use crate::Error;
use crate::install::Install;
use crate::log;
use crate::steam::APP_ID;

pub const MOD_LIST_FILE: &str = "warp_mods.txt";
pub const STEAM_APPID_FILE: &str = "steam_appid.txt";

/// A pack to load and the folder it lives in (`None`: the game's own `data` folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModListEntry {
    pub pack: String,
    pub dir: Option<PathBuf>,
}

/// The modlist file's contents.
pub fn mod_list(entries: &[ModListEntry]) -> String {
    let mut out = String::new();
    let mut seen = std::collections::HashSet::new();
    for dir in entries.iter().filter_map(|e| e.dir.as_ref()) {
        let dir = dir.to_string_lossy().replace('\\', "/");
        if seen.insert(dir.to_lowercase()) {
            out.push_str(&format!("add_working_directory \"{dir}\";\r\n"));
        }
    }
    for e in entries {
        out.push_str(&format!("mod \"{}\";\r\n", e.pack));
    }
    out
}

/// Writes the modlist into the game folder and returns its path.
/// Packs the game won't find: not in their workshop folder, nor in `data`.
pub fn uninstalled<'a>(install: &Install, entries: &'a [ModListEntry]) -> Vec<&'a str> {
    entries
        .iter()
        .filter(|e| e.dir.is_none() && !install.data_dir().join(&e.pack).is_file())
        .map(|e| e.pack.as_str())
        .collect()
}

pub fn write_mod_list(install: &Install, entries: &[ModListEntry]) -> Result<PathBuf, Error> {
    let path = install.game_dir.join(MOD_LIST_FILE);
    std::fs::write(&path, mod_list(entries)).map_err(|e| {
        Error::Install(format!(
            "can't write the modlist to {}: {e}",
            path.display()
        ))
    })?;
    Ok(path)
}

/// Writes `steam_appid.txt` into the game folder, so the game started directly doesn't
/// hand itself back to Steam. A failure is logged, not fatal: the game often starts anyway.
pub fn allow_direct_start(install: &Install) {
    let path = install.game_dir.join(STEAM_APPID_FILE);
    let id = APP_ID.to_string();
    if std::fs::read_to_string(&path).is_ok_and(|t| t.trim() == id) {
        return;
    }
    if let Err(e) = std::fs::write(&path, &id) {
        log::line(format!("can't write {}: {e}", path.display()));
    }
}

/// The game's arguments. The semicolons belong to the game's own argument syntax.
pub fn game_args(continue_save: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(save) = continue_save {
        args.extend([
            "game_startup_mode".to_owned(),
            "campaign_load".to_owned(),
            save.to_owned(),
            ";".to_owned(),
        ]);
    }
    args.push(format!("{MOD_LIST_FILE};"));
    args
}

/// Starts the game with the modlist written by [`write_mod_list`], optionally
/// loading straight into a saved campaign. Returns once the process has started.
pub fn launch(install: &Install, continue_save: Option<&str>) -> Result<(), Error> {
    let exe = install.game_exe();
    if !exe.is_file() {
        return Err(Error::Install(format!(
            "the game isn't at {}",
            exe.display()
        )));
    }
    allow_direct_start(install);
    let mut cmd = Command::new(&exe);
    let id = APP_ID.to_string();
    cmd.args(game_args(continue_save))
        .current_dir(&install.game_dir)
        .env("SteamAppId", &id)
        .env("SteamGameId", &id);
    spawn_detached(cmd).map_err(|e| Error::Install(format!("the game didn't start: {e}")))
}

#[cfg(windows)]
fn spawn_detached(mut cmd: Command) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(drop)
}

#[cfg(not(windows))]
fn spawn_detached(mut cmd: Command) -> std::io::Result<()> {
    cmd.spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_list_format() {
        let dir = PathBuf::from(r"C:\Steam\steamapps\workshop\content\1142710\42");
        let text = mod_list(&[
            ModListEntry {
                pack: "!top.pack".into(),
                dir: Some(dir.clone()),
            },
            ModListEntry {
                pack: "second.pack".into(),
                dir: Some(dir),
            },
            ModListEntry {
                pack: "local.pack".into(),
                dir: None,
            },
        ]);
        assert_eq!(
            text,
            "add_working_directory \"C:/Steam/steamapps/workshop/content/1142710/42\";\r\n\
             mod \"!top.pack\";\r\n\
             mod \"second.pack\";\r\n\
             mod \"local.pack\";\r\n"
        );
    }

    #[test]
    fn steam_appid_written_once() {
        let dir = std::env::temp_dir().join(format!("warp-appid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let install = Install::in_library(&dir, &dir);
        allow_direct_start(&install);
        let path = dir.join(STEAM_APPID_FILE);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "1142710");
        std::fs::write(
            &path, "1142710
",
        )
        .unwrap();
        allow_direct_start(&install);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "1142710
"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn args_with_and_without_a_save() {
        assert_eq!(game_args(None), ["warp_mods.txt;"]);
        assert_eq!(
            game_args(Some("my campaign.save")),
            [
                "game_startup_mode",
                "campaign_load",
                "my campaign.save",
                ";",
                "warp_mods.txt;"
            ]
        );
    }
}
