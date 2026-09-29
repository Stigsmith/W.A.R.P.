//! Backups of the database: taken when the app starts on a healthy library, kept
//! in `backups/` next to it, and restorable when the database is damaged or
//! turns up empty. A lost set list is hours of someone's work.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::log;
use crate::store::Store;

/// How many automatic backups to keep.
const KEEP: usize = 7;
/// Don't take another automatic backup within this many seconds of the last one.
const MIN_GAP_SECS: i64 = 3600;

/// A backup file and what's in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupInfo {
    pub path: String,
    /// Unix seconds, from the file name.
    pub at: i64,
    pub mods: usize,
    pub sets: usize,
    pub profiles: usize,
}

pub fn dir(db: &Path) -> PathBuf {
    db.with_file_name("backups")
}

/// Backups next to `db`, newest first. Files that can't be read are skipped.
pub fn list(db: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(dir(db)) else {
        return Vec::new();
    };
    let mut out: Vec<BackupInfo> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|p| {
            let at = p
                .file_name()?
                .to_str()?
                .strip_prefix("warp-")?
                .strip_suffix(".db")?
                .parse()
                .ok()?;
            describe(&p, at)
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.at));
    out
}

fn describe(path: &Path, at: i64) -> Option<BackupInfo> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let count = |table: &str| -> Option<usize> {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
            r.get::<_, i64>(0)
        })
        .ok()
        .map(|n| n as usize)
    };
    Some(BackupInfo {
        path: path.display().to_string(),
        at,
        mods: count("mods")?,
        sets: count("sets")?,
        profiles: count("profiles")?,
    })
}

/// Takes a backup of a healthy, non-empty library, unless one was taken in the last
/// hour, and prunes old ones. Returns the new backup, if any.
pub fn take_if_due(store: &Store, db: &Path) -> Result<Option<PathBuf>, Error> {
    if store.mods()?.is_empty() {
        // An empty library is never worth a backup: it would push out the good ones.
        return Ok(None);
    }
    let now = log::now();
    if list(db).first().is_some_and(|b| now - b.at < MIN_GAP_SECS) {
        return Ok(None);
    }
    let dir = dir(db);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("warp-{now}.db"));
    store.backup_to(&path)?;
    for old in list(db).into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(&old.path);
    }
    Ok(Some(path))
}

/// Puts a backup in place of the database. The database must be closed. The files
/// it replaces are moved to `backups/replaced-<time>/`, never deleted.
pub fn restore(backup: &Path, db: &Path) -> Result<PathBuf, Error> {
    let aside = dir(db).join(format!("replaced-{}", log::now()));
    std::fs::create_dir_all(&aside)?;
    for suffix in ["", "-wal", "-shm"] {
        let file = PathBuf::from(format!("{}{suffix}", db.display()));
        if file.exists() {
            let name = file.file_name().expect("a database path has a file name");
            std::fs::rename(&file, aside.join(name))?;
        }
    }
    std::fs::copy(backup, db)?;
    Ok(aside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ModInfo, WorkshopId};
    use crate::store::ModSet;

    fn temp_db(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("warp-backup-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("warp.db")
    }

    fn add_mod(store: &mut Store, id: u64) {
        store
            .transaction(|tx| tx.upsert_mod(&ModInfo::unknown(WorkshopId(id))))
            .unwrap();
    }

    #[test]
    fn backs_up_a_healthy_library_and_restores_it() {
        let db = temp_db("roundtrip");
        let mut store = Store::open(&db).unwrap();
        assert_eq!(
            take_if_due(&store, &db).unwrap(),
            None,
            "an empty library isn't backed up"
        );

        add_mod(&mut store, 1);
        store
            .save_set(&ModSet {
                name: "Base".into(),
                members: vec![WorkshopId(1)],
            })
            .unwrap();
        let taken = take_if_due(&store, &db).unwrap().expect("a backup");
        assert_eq!(
            take_if_due(&store, &db).unwrap(),
            None,
            "not twice within the hour"
        );
        let listed = list(&db);
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (listed[0].mods, listed[0].sets, listed[0].profiles),
            (1, 1, 0)
        );

        // Lose everything, then restore.
        add_mod(&mut store, 2);
        drop(store);
        let aside = restore(&taken, &db).unwrap();
        assert!(
            aside.join("warp.db").exists(),
            "the replaced database is kept"
        );
        let store = Store::open(&db).unwrap();
        assert_eq!(store.mods().unwrap().len(), 1);
        assert_eq!(store.sets().unwrap()[0].name, "Base");
        drop(store);
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }
}
