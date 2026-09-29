//! The desktop app: Tauri commands over `warp-core`. No logic lives here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use warp_core::backup::{self, BackupInfo};
use warp_core::conflicts::{ConflictReport, DbOverlap};
use warp_core::install::Install;
use warp_core::kaedrin;
use warp_core::knowledge::ModKnowledge;
use warp_core::library::{ImportSummary, Library, LibraryEntry, ResolvedProfile, SyncSummary};
use warp_core::model::WorkshopId;
use warp_core::mp::{self, ListDiff, ShareList};
use warp_core::sets::SetUpdate;
use warp_core::store::{ModSet, ProfileDef, Store};
use warp_core::taxonomy::Taxonomy;
use warp_core::{import_v1, log, report, steam};

/// The library, or why its database couldn't be opened (or is damaged). The app
/// still starts without it, so the user sees what went wrong, can restore a
/// backup (which swaps the library in place) and can copy a report.
struct AppState(Mutex<Result<Library, String>>);

type CmdResult<T> = Result<T, String>;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn with_lib<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut Library) -> Result<T, warp_core::Error>,
) -> CmdResult<T> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "internal error: library lock poisoned".to_owned())?;
    let lib = guard.as_mut().map_err(|e| e.clone())?;
    f(lib).map_err(|e| e.to_string())
}

/// Opens the database and makes sure it's healthy before anything uses it; takes
/// a backup of a healthy library. Everything it finds goes into the log.
fn open_library(db: &Path) -> Result<Library, String> {
    let store = Store::open(db)
        .map_err(|e| format!("couldn't open the database at {}: {e}", db.display()))?;
    let check = store.quick_check().map_err(|e| e.to_string())?;
    if check.first().map(String::as_str) != Some("ok") {
        return Err(format!(
            "the database at {} is damaged ({})",
            db.display(),
            // SQLite lists every broken page; the first line says enough.
            check
                .first()
                .and_then(|c| c.lines().find(|l| !l.starts_with("***")))
                .unwrap_or_default()
        ));
    }
    let lib = Library::new(store, warp_core::builtin_kb());
    let count =
        |n: Result<usize, warp_core::Error>| n.map_or_else(|e| e.to_string(), |n| n.to_string());
    log::line(format_args!(
        "database: {} mods, {} sets, {} profiles",
        count(lib.store.mods().map(|m| m.len())),
        count(lib.store.sets().map(|s| s.len())),
        count(lib.store.profiles().map(|p| p.len())),
    ));
    match backup::take_if_due(&lib.store, db) {
        Ok(Some(path)) => log::line(format_args!("backup: {}", path.display())),
        Ok(None) => {}
        Err(e) => log::line(format_args!("backup failed: {e}")),
    }
    Ok(lib)
}

#[derive(Serialize)]
struct Bootstrap {
    version: &'static str,
    /// A backup worth offering when the library is empty but the backup isn't.
    restorable: Option<BackupInfo>,
    taxonomy: Taxonomy,
    mod_count: usize,
    data_dir: String,
    kaedrin_dir: Option<String>,
    /// Where the game is, or why it wasn't found.
    install: Result<Install, String>,
}

#[tauri::command]
async fn bootstrap(state: State<'_, AppState>) -> CmdResult<Bootstrap> {
    with_lib(&state, |lib| {
        let empty = lib.store.mods()?.is_empty() && lib.store.profiles()?.is_empty();
        let restorable = if empty {
            backup::list(&warp_core::default_db_path())
                .into_iter()
                .find(|b| b.mods > 0)
        } else {
            None
        };
        Ok(Bootstrap {
            version: VERSION,
            restorable,
            taxonomy: lib.taxonomy.clone(),
            mod_count: lib.store.mods()?.len(),
            data_dir: warp_core::data_dir().display().to_string(),
            kaedrin_dir: kaedrin::profiles_dir()
                .filter(|d| d.is_dir())
                .map(|d| d.display().to_string()),
            install: Install::locate().map_err(|e| e.to_string()),
        })
    })
}

/// Reads the installed game: subscriptions, installed versions and the pack index.
#[tauri::command]
async fn sync_install(state: State<'_, AppState>, check_steam: bool) -> CmdResult<SyncSummary> {
    let install = Install::locate().map_err(|e| e.to_string())?;
    let result = with_lib(&state, |lib| lib.sync_install(&install, check_steam));
    match &result {
        Ok(s) => log::line(format_args!(
            "sync: {} installed, {} new, {} packs read, {} unreadable",
            s.installed,
            s.new_mods.len(),
            s.packs_indexed,
            s.pack_errors.len()
        )),
        Err(e) => log::line(format_args!("sync failed: {e}")),
    }
    result
}

#[tauri::command]
async fn conflicts(state: State<'_, AppState>, profile: ProfileDef) -> CmdResult<ConflictReport> {
    with_lib(&state, |lib| lib.conflicts(&profile))
}

/// Writes the profile's modlist and starts the game; returns the modlist path.
#[tauri::command]
async fn play(state: State<'_, AppState>, profile: ProfileDef) -> CmdResult<String> {
    let install = Install::locate().map_err(|e| e.to_string())?;
    let result = with_lib(&state, |lib| lib.play(&profile, &install, None));
    match &result {
        Ok(path) => log::line(format_args!(
            "play: \"{}\", wrote {}",
            profile.name,
            path.display()
        )),
        Err(e) => log::line(format_args!("play: \"{}\" failed: {e}", profile.name)),
    }
    result.map(|p| p.display().to_string())
}

/// Everything W.A.R.P. sees on this PC as plain text, for the user to send back.
/// Works even when the database couldn't be opened.
#[tauri::command]
async fn diagnostics(state: State<'_, AppState>, extra: String) -> CmdResult<String> {
    let install = Install::locate().map_err(|e| e.to_string());
    let guard = state
        .0
        .lock()
        .map_err(|_| "internal error: library lock poisoned".to_owned())?;
    Ok(report::report(
        guard.as_ref().ok(),
        &report::Context {
            version: VERSION,
            install: &install,
            library_error: guard.as_ref().err().map(String::as_str),
            extra: &extra,
        },
    ))
}

/// Backups of the database, newest first.
#[tauri::command]
async fn backups() -> CmdResult<Vec<BackupInfo>> {
    Ok(backup::list(&warp_core::default_db_path()))
}

/// Puts a backup in place of the database, keeping the replaced files, and
/// reopens the library. Works while the database is damaged, too.
#[tauri::command]
async fn restore_backup(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let db = warp_core::default_db_path();
    let backup_path = PathBuf::from(&path);
    // Only files from our own backups folder.
    if backup_path.parent() != Some(backup::dir(&db).as_path()) {
        return Err("that isn't one of W.A.R.P.'s backups".into());
    }
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "internal error: library lock poisoned".to_owned())?;
    // Close the current database before moving its files.
    *guard = Err("restoring a backup".into());
    let aside = backup::restore(&backup_path, &db).map_err(|e| e.to_string());
    match &aside {
        Ok(dir) => log::line(format_args!(
            "restored {path}; the replaced files are in {}",
            dir.display()
        )),
        Err(e) => log::line(format_args!("restoring {path} failed: {e}")),
    }
    *guard = open_library(&db);
    aside.and(guard.as_ref().map(drop).map_err(|e| e.clone()))
}

/// Errors the user saw in the app, so they end up in the log and the report.
#[tauri::command]
async fn log_ui(line: String) -> CmdResult<()> {
    log::line(format_args!("ui: {line}"));
    Ok(())
}

/// Installed mods that look like two versions of the same mod.
#[tauri::command]
async fn either_or_pairs(state: State<'_, AppState>) -> CmdResult<Vec<DbOverlap>> {
    with_lib(&state, |lib| lib.either_or_pairs())
}

#[tauri::command]
async fn library(state: State<'_, AppState>) -> CmdResult<Vec<LibraryEntry>> {
    with_lib(&state, |lib| lib.entries())
}

#[tauri::command]
async fn set_knowledge(
    state: State<'_, AppState>,
    id: WorkshopId,
    knowledge: ModKnowledge,
) -> CmdResult<()> {
    with_lib(&state, |lib| lib.set_user_knowledge(id, &knowledge))
}

#[tauri::command]
async fn sets(state: State<'_, AppState>) -> CmdResult<Vec<ModSet>> {
    with_lib(&state, |lib| lib.store.sets())
}

#[tauri::command]
async fn save_set(state: State<'_, AppState>, set: ModSet) -> CmdResult<()> {
    with_lib(&state, |lib| lib.save_set(&set))
}

#[tauri::command]
async fn create_set(
    state: State<'_, AppState>,
    name: String,
    members: Vec<WorkshopId>,
) -> CmdResult<()> {
    with_lib(&state, |lib| lib.create_set(&name, &members))
}

/// Adds and removes mods in one go (the Library's set columns and bulk actions).
#[tauri::command]
async fn edit_set(
    state: State<'_, AppState>,
    name: String,
    add: Vec<WorkshopId>,
    remove: Vec<WorkshopId>,
) -> CmdResult<()> {
    with_lib(&state, |lib| lib.edit_set(&name, &add, &remove))
}

#[tauri::command]
async fn rename_set(state: State<'_, AppState>, from: String, to: String) -> CmdResult<()> {
    with_lib(&state, |lib| lib.rename_set(&from, &to))
}

#[tauri::command]
async fn delete_set(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    with_lib(&state, |lib| lib.delete_set(&name))
}

/// Set changes each profile hasn't taken or dismissed yet.
#[tauri::command]
async fn set_updates(state: State<'_, AppState>) -> CmdResult<BTreeMap<String, Vec<SetUpdate>>> {
    with_lib(&state, |lib| lib.set_updates())
}

#[tauri::command]
async fn apply_set_updates(
    state: State<'_, AppState>,
    profile: String,
    sets: Vec<String>,
) -> CmdResult<ProfileDef> {
    with_lib(&state, |lib| lib.apply_set_updates(&profile, &sets))
}

#[tauri::command]
async fn dismiss_set_updates(
    state: State<'_, AppState>,
    profile: String,
    sets: Vec<String>,
) -> CmdResult<ProfileDef> {
    with_lib(&state, |lib| lib.dismiss_set_updates(&profile, &sets))
}

#[tauri::command]
async fn profiles(state: State<'_, AppState>) -> CmdResult<Vec<ProfileDef>> {
    with_lib(&state, |lib| lib.store.profiles())
}

#[tauri::command]
async fn save_profile(state: State<'_, AppState>, profile: ProfileDef) -> CmdResult<()> {
    with_lib(&state, |lib| lib.save_profile(&profile))
}

#[tauri::command]
async fn delete_profile(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    with_lib(&state, |lib| lib.store.delete_profile(&name))
}

#[tauri::command]
async fn resolve_profile(
    state: State<'_, AppState>,
    profile: ProfileDef,
) -> CmdResult<ResolvedProfile> {
    with_lib(&state, |lib| lib.resolve_profile(&profile))
}

#[tauri::command]
async fn share_list(state: State<'_, AppState>, profile: ProfileDef) -> CmdResult<ShareList> {
    with_lib(&state, |lib| lib.share_list(&profile))
}

#[tauri::command]
async fn encode_share(list: ShareList) -> CmdResult<String> {
    Ok(mp::encode(&list))
}

#[tauri::command]
async fn save_warp_file(list: ShareList, path: String) -> CmdResult<()> {
    std::fs::write(path, mp::to_warp_file(&list)).map_err(|e| e.to_string())
}

/// Where a modlist to compare comes from.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ListSource {
    /// A share code, or a whole message containing one.
    Code { text: String },
    /// A `.warp` file or a Kaedrin profile `.txt`.
    File { path: String },
    /// One of the user's profiles.
    Profile { name: String },
}

#[tauri::command]
async fn load_list(state: State<'_, AppState>, source: ListSource) -> CmdResult<ShareList> {
    with_lib(&state, |lib| match source {
        ListSource::Code { text } => mp::decode_any(&text),
        ListSource::File { path } => {
            let path = PathBuf::from(path);
            let text = std::fs::read_to_string(&path)?;
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("warp"))
            {
                mp::from_warp_file(&text)
            } else {
                let name = file_label(&path);
                lib.share_list_from_packs(&name, &kaedrin::read_profile(&text))
            }
        }
        ListSource::Profile { name } => match lib.store.profile(&name)? {
            Some(def) => lib.share_list(&def),
            None => Err(warp_core::Error::Invalid(format!(
                "no profile named '{name}'"
            ))),
        },
    })
}

fn file_label(path: &Path) -> String {
    path.file_stem()
        .map(|s| {
            s.to_string_lossy()
                .trim_start_matches("profile_")
                .to_owned()
        })
        .unwrap_or_default()
}

#[derive(Serialize)]
struct CompareResult {
    diff: ListDiff,
    /// Both lists, with pack names filled in from the library where a code left them out.
    a: ShareList,
    b: ShareList,
    /// Titles from Steam for mods in the lists that aren't in the library.
    titles: HashMap<WorkshopId, String>,
}

#[tauri::command]
async fn compare(
    state: State<'_, AppState>,
    mut a: ShareList,
    mut b: ShareList,
    check_steam: bool,
) -> CmdResult<CompareResult> {
    with_lib(&state, |lib| {
        let diff = lib.compare(&mut a, &mut b, check_steam)?;
        let known: HashSet<WorkshopId> = lib.store.mods()?.into_iter().map(|(m, _)| m.id).collect();
        let unknown: Vec<WorkshopId> = a
            .entries
            .iter()
            .chain(&b.entries)
            .filter_map(|e| e.workshop_id)
            .filter(|id| !known.contains(id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        // Titles are a nicety: without Steam the ids still identify the mods.
        let titles = if check_steam && !unknown.is_empty() {
            steam::fetch_details(&unknown)
                .map(|infos| {
                    infos
                        .into_iter()
                        .filter(|m| !m.title.is_empty())
                        .map(|m| (m.id, m.title))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        Ok(CompareResult { diff, a, b, titles })
    })
}

/// Writes packs as a Kaedrin profile; returns the file written.
#[tauri::command]
async fn export_kaedrin(
    packs: Vec<String>,
    name: String,
    path: Option<String>,
) -> CmdResult<String> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => {
            let dir = kaedrin::profiles_dir().ok_or("Kaedrin Mod Manager's folder wasn't found")?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            dir.join(kaedrin::profile_file_name(&name))
        }
    };
    std::fs::write(&path, kaedrin::write_profile(&packs)).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
async fn import_v1(state: State<'_, AppState>, path: String) -> CmdResult<ImportSummary> {
    let data = import_v1::read(Path::new(&path)).map_err(|e| e.to_string())?;
    with_lib(&state, |lib| lib.import_v1(&data))
}

#[tauri::command]
async fn refresh_steam(state: State<'_, AppState>) -> CmdResult<usize> {
    with_lib(&state, |lib| lib.refresh_from_steam(None))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::line(format_args!("crash: {info}"));
        default_hook(info);
    }));

    tauri::Builder::default()
        // First: a second W.A.R.P. only brings this window forward, so two copies
        // never share (and fight over) one database.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // After the single-instance check, so a second copy quits before it
        // touches the database.
        .setup(|app| {
            log::line(format_args!("W.A.R.P. {VERSION} started"));
            let opened = open_library(&warp_core::default_db_path()).inspect_err(|e| log::line(e));
            app.manage(AppState(Mutex::new(opened)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            library,
            set_knowledge,
            sets,
            save_set,
            create_set,
            edit_set,
            rename_set,
            delete_set,
            set_updates,
            apply_set_updates,
            dismiss_set_updates,
            profiles,
            save_profile,
            delete_profile,
            resolve_profile,
            share_list,
            encode_share,
            save_warp_file,
            load_list,
            compare,
            export_kaedrin,
            import_v1,
            refresh_steam,
            sync_install,
            conflicts,
            either_or_pairs,
            play,
            diagnostics,
            log_ui,
            backups,
            restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("W.A.R.P. failed to start");
}
