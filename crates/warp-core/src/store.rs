//! Per-user storage (SQLite): the Steam metadata cache, packs, subscriptions,
//! installed versions, the pack index, the user's knowledge overrides, sets,
//! profiles and launches.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::conflicts::DbOverlap;
use crate::knowledge::ModKnowledge;
use crate::model::{ModInfo, WorkshopId, pack_key};
use crate::order::Pin;
use crate::pack_index::{self, Contents, PackIndex};
use crate::sets::{self, SetChanges};

/// Schema migrations, applied in order. Never edit a released entry; append a new one.
const MIGRATIONS: &[&str] = &[
    r"
    CREATE TABLE mods (
        id          INTEGER PRIMARY KEY,
        info        TEXT NOT NULL,
        subscribed  INTEGER NOT NULL DEFAULT 1,
        updated_at  INTEGER NOT NULL
    );
    CREATE TABLE packs (
        mod_id  INTEGER NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
        name    TEXT NOT NULL,
        PRIMARY KEY (mod_id, name)
    );
    CREATE TABLE user_knowledge (
        mod_id  INTEGER PRIMARY KEY,
        data    TEXT NOT NULL
    );
    CREATE TABLE sets (
        name  TEXT PRIMARY KEY COLLATE NOCASE
    );
    CREATE TABLE set_members (
        set_name  TEXT NOT NULL COLLATE NOCASE REFERENCES sets(name) ON DELETE CASCADE ON UPDATE CASCADE,
        mod_id    INTEGER NOT NULL,
        PRIMARY KEY (set_name, mod_id)
    );
    CREATE TABLE profiles (
        name        TEXT PRIMARY KEY COLLATE NOCASE,
        data        TEXT NOT NULL,
        updated_at  INTEGER NOT NULL
    );
",
    r"
    -- The version of each mod actually on disk, from Steam's workshop manifest.
    CREATE TABLE installed (
        mod_id        INTEGER PRIMARY KEY,
        time_updated  INTEGER NOT NULL,
        size          INTEGER NOT NULL
    );
    -- Pack X-ray cache, keyed by file path; stale when size or mtime change.
    CREATE TABLE pack_index (
        path          TEXT PRIMARY KEY,
        name          TEXT NOT NULL,
        mod_id        INTEGER,
        size          INTEGER NOT NULL,
        modified      INTEGER NOT NULL,
        version       INTEGER NOT NULL,
        kind          TEXT NOT NULL,
        dependencies  TEXT NOT NULL,
        file_count    INTEGER NOT NULL,
        files         BLOB NOT NULL
    );
    CREATE INDEX pack_index_name ON pack_index (name COLLATE NOCASE);
    CREATE TABLE launches (
        id       INTEGER PRIMARY KEY,
        at       INTEGER NOT NULL,
        profile  TEXT NOT NULL,
        packs    TEXT NOT NULL
    );
",
    r"
    -- Content fingerprint per pack (facet counts + DB tables), for guessing tiers.
    ALTER TABLE pack_index ADD COLUMN contents TEXT NOT NULL DEFAULT '{}';
",
    r"
    -- Mod pairs shipping identical DB file paths, worked out after each sync.
    CREATE TABLE db_overlaps (
        a        INTEGER NOT NULL,
        b        INTEGER NOT NULL,
        shared   INTEGER NOT NULL,
        a_files  INTEGER NOT NULL,
        b_files  INTEGER NOT NULL,
        example  TEXT NOT NULL,
        PRIMARY KEY (a, b)
    );
",
];

/// A named, reusable group of mods (v1 called these Components).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSet {
    pub name: String,
    pub members: Vec<WorkshopId>,
}

/// A profile: sets stacked together, plus per-profile tweaks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDef {
    pub name: String,
    pub sets: Vec<String>,
    #[serde(default)]
    pub include: Vec<WorkshopId>,
    #[serde(default)]
    pub exclude: Vec<WorkshopId>,
    #[serde(default)]
    pub pins: Vec<Pin>,
    /// The profile's own copy of each of its sets (see [`sets`]). Filled in on save.
    #[serde(default)]
    pub set_members: BTreeMap<String, Vec<WorkshopId>>,
    #[serde(default)]
    pub set_changes: SetChanges,
    /// Set changes the user dismissed: set name to the set's fingerprint at the time.
    #[serde(default)]
    pub dismissed: BTreeMap<String, String>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, Error> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let version = usize::try_from(version).unwrap_or(0);
        if version > MIGRATIONS.len() {
            return Err(Error::Format(
                "this database was created by a newer WARP".into(),
            ));
        }
        let tx = conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        tx.commit()?;
        let mut store = Self { conn };
        // Profiles saved before profiles kept their own copy of each set get one now.
        let old: Vec<ProfileDef> = store
            .profiles()?
            .into_iter()
            .filter(|p| p.sets.iter().any(|s| !p.set_members.contains_key(s)))
            .collect();
        if !old.is_empty() {
            store.transaction(|tx| old.iter().try_for_each(|p| tx.save_profile(p)))?;
        }
        Ok(store)
    }

    /// Runs `f` in one transaction: all of it lands, or none of it.
    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&Tx<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let tx = Tx(self.conn.transaction()?);
        let out = f(&tx)?;
        tx.0.commit()?;
        Ok(out)
    }

    pub fn mods(&self) -> Result<Vec<(ModInfo, bool)>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT info, subscribed FROM mods ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))?;
        rows.map(|row| {
            let (json, subscribed) = row?;
            Ok((
                serde_json::from_str(&json).map_err(|e| Error::Format(e.to_string()))?,
                subscribed,
            ))
        })
        .collect()
    }

    pub fn packs(&self) -> Result<HashMap<WorkshopId, Vec<String>>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT mod_id, name FROM packs ORDER BY mod_id, name")?;
        let mut out: HashMap<WorkshopId, Vec<String>> = HashMap::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            out.entry(WorkshopId(id as u64)).or_default().push(name);
        }
        Ok(out)
    }

    pub fn user_knowledge(&self) -> Result<HashMap<WorkshopId, ModKnowledge>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT mod_id, data FROM user_knowledge")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (id, json) = row?;
            Ok((
                WorkshopId(id as u64),
                serde_json::from_str(&json).map_err(|e| Error::Format(e.to_string()))?,
            ))
        })
        .collect()
    }

    pub fn set_user_knowledge(&self, id: WorkshopId, k: &ModKnowledge) -> Result<(), Error> {
        if k.is_empty() {
            self.conn.execute(
                "DELETE FROM user_knowledge WHERE mod_id = ?1",
                params![id.0 as i64],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO user_knowledge (mod_id, data) VALUES (?1, ?2)
                 ON CONFLICT (mod_id) DO UPDATE SET data = excluded.data",
                params![id.0 as i64, to_json(k)],
            )?;
        }
        Ok(())
    }

    pub fn sets(&self) -> Result<Vec<ModSet>, Error> {
        read_sets(&self.conn)
    }

    /// Creates the set if needed and replaces its members.
    pub fn save_set(&mut self, set: &ModSet) -> Result<(), Error> {
        self.transaction(|tx| tx.save_set(set))
    }

    pub fn profiles(&self) -> Result<Vec<ProfileDef>, Error> {
        read_profiles(&self.conn)
    }

    pub fn profile(&self, name: &str) -> Result<Option<ProfileDef>, Error> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT data FROM profiles WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(|e| Error::Format(e.to_string())))
            .transpose()
    }

    pub fn save_profile(&mut self, profile: &ProfileDef) -> Result<(), Error> {
        self.transaction(|tx| tx.save_profile(profile))
    }

    pub fn delete_profile(&self, name: &str) -> Result<(), Error> {
        self.conn
            .execute("DELETE FROM profiles WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// The installed version (`time_updated`) of each mod on disk.
    pub fn installed(&self) -> Result<HashMap<WorkshopId, i64>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT mod_id, time_updated FROM installed")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        rows.map(|row| {
            let (id, t) = row?;
            Ok((WorkshopId(id as u64), t))
        })
        .collect()
    }

    /// (size, modified) of every indexed pack, by path, to tell which need re-reading.
    /// Only entries written by the current index format count; older ones get re-read.
    pub fn pack_index_stamps(&self) -> Result<HashMap<String, (u64, i64)>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, size, modified FROM pack_index WHERE version = ?1")?;
        let rows = stmt.query_map([pack_index::INDEX_VERSION], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)?),
            ))
        })?;
        rows.map(|row| Ok(row?)).collect()
    }

    /// Declared dependencies of every indexed pack, by lowercase pack name. Cheap: no file lists.
    pub fn pack_dependencies(&self) -> Result<HashMap<String, Vec<String>>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, dependencies FROM pack_index")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (name, deps) = row?;
            let deps: Vec<String> =
                serde_json::from_str(&deps).map_err(|e| Error::Format(e.to_string()))?;
            Ok((pack_key(&name), deps))
        })
        .collect()
    }

    /// Full indexes (with file lists) for the named packs, by lowercase name.
    pub fn pack_indexes(&self, names: &[String]) -> Result<HashMap<String, PackIndex>, Error> {
        let wanted: std::collections::HashSet<String> = names.iter().map(|n| pack_key(n)).collect();
        let mut stmt = self
            .conn
            .prepare("SELECT name, size, modified, kind, dependencies, files FROM pack_index")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Vec<u8>>(5)?,
            ))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (name, size, modified, kind, deps, files) = row?;
            let key = pack_key(&name);
            if !wanted.contains(&key) {
                continue;
            }
            let index = PackIndex {
                name,
                size: size as u64,
                modified,
                kind,
                dependencies: serde_json::from_str(&deps)
                    .map_err(|e| Error::Format(e.to_string()))?,
                files: PackIndex::files_from_blob(&files)?,
            };
            out.insert(key, index);
        }
        Ok(out)
    }

    /// Each mod's DB table files across its indexed packs.
    pub fn mod_db_files(&self) -> Result<Vec<(WorkshopId, Vec<String>)>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT mod_id, files FROM pack_index WHERE mod_id IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
        let mut out: HashMap<WorkshopId, std::collections::BTreeSet<String>> = HashMap::new();
        for row in rows {
            let (id, blob) = row?;
            let files = PackIndex::files_from_blob(&blob)?;
            out.entry(WorkshopId(id as u64))
                .or_default()
                .extend(files.into_iter().filter(|f| pack_index::is_db_file(f)));
        }
        let mut out: Vec<(WorkshopId, Vec<String>)> = out
            .into_iter()
            .map(|(id, files)| (id, files.into_iter().collect()))
            .collect();
        out.sort_by_key(|(id, _)| *id);
        Ok(out)
    }

    pub fn db_overlaps(&self) -> Result<Vec<DbOverlap>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT a, b, shared, a_files, b_files, example FROM db_overlaps ORDER BY a, b",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(DbOverlap {
                a: WorkshopId(r.get::<_, i64>(0)? as u64),
                b: WorkshopId(r.get::<_, i64>(1)? as u64),
                shared: r.get::<_, i64>(2)? as usize,
                a_files: r.get::<_, i64>(3)? as usize,
                b_files: r.get::<_, i64>(4)? as usize,
                example: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Content fingerprint of every indexed pack, by lowercase pack name.
    pub fn pack_contents(&self) -> Result<HashMap<String, Contents>, Error> {
        let mut stmt = self.conn.prepare("SELECT name, contents FROM pack_index")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (name, json) = row?;
            Ok((
                pack_key(&name),
                serde_json::from_str(&json).unwrap_or_default(),
            ))
        })
        .collect()
    }

    /// Per-pack summaries (no file lists): name, file count, kind.
    pub fn pack_summaries(&self) -> Result<HashMap<String, (usize, String)>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, file_count, kind FROM pack_index")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)? as usize,
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (name, count, kind) = row?;
            Ok((pack_key(&name), (count, kind)))
        })
        .collect()
    }

    /// When the game was last started from WARP: time, profile, pack count.
    pub fn last_launch(&self) -> Result<Option<(i64, String, usize)>, Error> {
        let row: Option<(i64, String, String)> = self
            .conn
            .query_row(
                "SELECT at, profile, packs FROM launches ORDER BY at DESC, id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(row.map(|(at, profile, packs)| {
            let n = serde_json::from_str::<Vec<String>>(&packs).map_or(0, |p| p.len());
            (at, profile, n)
        }))
    }

    pub fn record_launch(&self, profile: &str, packs: &[String]) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO launches (at, profile, packs) VALUES (?1, ?2, ?3)",
            params![now(), profile, to_json(&packs)],
        )?;
        Ok(())
    }
}

/// Write operations, usable inside [`Store::transaction`].
pub struct Tx<'a>(Transaction<'a>);

impl Tx<'_> {
    pub fn upsert_mod(&self, info: &ModInfo) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO mods (id, info, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (id) DO UPDATE SET info = excluded.info, updated_at = excluded.updated_at",
            params![info.id.0 as i64, to_json(info), now()],
        )?;
        Ok(())
    }

    pub fn set_subscribed(&self, id: WorkshopId, subscribed: bool) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mods SET subscribed = ?2 WHERE id = ?1",
            params![id.0 as i64, subscribed],
        )?;
        Ok(())
    }

    /// Replaces the packs recorded for a mod.
    pub fn set_packs(&self, id: WorkshopId, packs: &[String]) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM packs WHERE mod_id = ?1", params![id.0 as i64])?;
        for p in packs {
            self.0.execute(
                "INSERT OR IGNORE INTO packs (mod_id, name) VALUES (?1, ?2)",
                params![id.0 as i64, p],
            )?;
        }
        Ok(())
    }

    pub fn save_set(&self, set: &ModSet) -> Result<(), Error> {
        let name = set.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a set needs a name".into()));
        }
        self.0.execute(
            "INSERT OR IGNORE INTO sets (name) VALUES (?1)",
            params![name],
        )?;
        self.0
            .execute("DELETE FROM set_members WHERE set_name = ?1", params![name])?;
        for id in &set.members {
            self.0.execute(
                "INSERT OR IGNORE INTO set_members (set_name, mod_id) VALUES (?1, ?2)",
                params![name, id.0 as i64],
            )?;
        }
        Ok(())
    }

    /// Saves a profile, first giving it its own copy of any set it has none of.
    pub fn save_profile(&self, profile: &ProfileDef) -> Result<(), Error> {
        if profile.name.trim().is_empty() {
            return Err(Error::Invalid("a profile needs a name".into()));
        }
        let mut profile = profile.clone();
        sets::fill(&mut profile, &read_sets(&self.0)?);
        self.0.execute(
            "INSERT INTO profiles (name, data, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (name) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
            params![profile.name.trim(), to_json(&profile), now()],
        )?;
        Ok(())
    }

    pub fn profiles(&self) -> Result<Vec<ProfileDef>, Error> {
        read_profiles(&self.0)
    }

    /// Creates an empty set. Fails if one with that name (in any case) exists.
    pub fn create_set(&self, name: &str) -> Result<(), Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a set needs a name".into()));
        }
        if sets::find(&read_sets(&self.0)?, name).is_some() {
            return Err(Error::Invalid(format!(
                "there is already a set called \"{name}\""
            )));
        }
        self.0
            .execute("INSERT INTO sets (name) VALUES (?1)", params![name])?;
        Ok(())
    }

    /// Adds and removes members without touching the rest of the set.
    pub fn edit_set(
        &self,
        name: &str,
        add: &[WorkshopId],
        remove: &[WorkshopId],
    ) -> Result<(), Error> {
        if sets::find(&read_sets(&self.0)?, name).is_none() {
            return Err(Error::Invalid(format!("there is no set called \"{name}\"")));
        }
        for id in add {
            self.0.execute(
                "INSERT OR IGNORE INTO set_members (set_name, mod_id) VALUES (?1, ?2)",
                params![name, id.0 as i64],
            )?;
        }
        for id in remove {
            self.0.execute(
                "DELETE FROM set_members WHERE set_name = ?1 AND mod_id = ?2",
                params![name, id.0 as i64],
            )?;
        }
        Ok(())
    }

    /// Renames a set, and follows the rename in every profile that uses it.
    pub fn rename_set(&self, from: &str, to: &str) -> Result<(), Error> {
        let to = to.trim();
        if to.is_empty() {
            return Err(Error::Invalid("a set needs a name".into()));
        }
        let all = read_sets(&self.0)?;
        if sets::find(&all, from).is_none() {
            return Err(Error::Invalid(format!("there is no set called \"{from}\"")));
        }
        if !from.eq_ignore_ascii_case(to) && sets::find(&all, to).is_some() {
            return Err(Error::Invalid(format!(
                "there is already a set called \"{to}\""
            )));
        }
        self.0.execute(
            "UPDATE sets SET name = ?2 WHERE name = ?1",
            params![from, to],
        )?;
        for mut p in self.profiles()? {
            if sets::rename(&mut p, from, to) {
                self.save_profile(&p)?;
            }
        }
        Ok(())
    }

    pub fn delete_set(&self, name: &str) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM sets WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// Profiles that follow their sets take the sets' current contents.
    pub fn refresh_following(&self) -> Result<(), Error> {
        for p in self.profiles()? {
            if p.set_changes == SetChanges::Follow {
                self.save_profile(&p)?;
            }
        }
        Ok(())
    }

    /// Replaces the record of what's installed.
    pub fn replace_installed(&self, items: &[(WorkshopId, i64, u64)]) -> Result<(), Error> {
        self.0.execute("DELETE FROM installed", [])?;
        for (id, time_updated, size) in items {
            self.0.execute(
                "INSERT INTO installed (mod_id, time_updated, size) VALUES (?1, ?2, ?3)",
                params![id.0 as i64, time_updated, *size as i64],
            )?;
        }
        Ok(())
    }

    pub fn put_pack_index(
        &self,
        path: &str,
        mod_id: Option<WorkshopId>,
        index: &PackIndex,
    ) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO pack_index (path, name, mod_id, size, modified, kind, dependencies, file_count, files, version, contents)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT (path) DO UPDATE SET name = excluded.name, mod_id = excluded.mod_id,
                size = excluded.size, modified = excluded.modified, kind = excluded.kind,
                version = excluded.version, contents = excluded.contents,
                dependencies = excluded.dependencies, file_count = excluded.file_count, files = excluded.files",
            params![
                path,
                index.name,
                mod_id.map(|id| id.0 as i64),
                index.size as i64,
                index.modified,
                index.kind,
                to_json(&index.dependencies),
                index.files.len() as i64,
                index.files_blob(),
                pack_index::INDEX_VERSION,
                to_json(&index.contents()),
            ],
        )?;
        Ok(())
    }

    pub fn replace_db_overlaps(&self, overlaps: &[DbOverlap]) -> Result<(), Error> {
        self.0.execute("DELETE FROM db_overlaps", [])?;
        let mut stmt = self.0.prepare(
            "INSERT INTO db_overlaps (a, b, shared, a_files, b_files, example) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for o in overlaps {
            stmt.execute(params![
                o.a.0 as i64,
                o.b.0 as i64,
                o.shared as i64,
                o.a_files as i64,
                o.b_files as i64,
                o.example
            ])?;
        }
        Ok(())
    }

    /// Drops index entries for packs that are no longer on disk.
    pub fn retain_pack_index(
        &self,
        paths: &std::collections::HashSet<String>,
    ) -> Result<usize, Error> {
        let existing: Vec<String> = self
            .0
            .prepare("SELECT path FROM pack_index")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        let mut removed = 0;
        for path in existing.iter().filter(|p| !paths.contains(*p)) {
            removed += self
                .0
                .execute("DELETE FROM pack_index WHERE path = ?1", params![path])?;
        }
        Ok(removed)
    }
}

fn read_sets(conn: &Connection) -> Result<Vec<ModSet>, Error> {
    let mut sets: Vec<ModSet> = conn
        .prepare("SELECT name FROM sets ORDER BY name COLLATE NOCASE")?
        .query_map([], |r| r.get::<_, String>(0))?
        .map(|name| {
            Ok(ModSet {
                name: name?,
                members: Vec::new(),
            })
        })
        .collect::<Result<_, Error>>()?;
    let mut stmt = conn.prepare("SELECT set_name, mod_id FROM set_members ORDER BY mod_id")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (name, id) = row?;
        if let Some(set) = sets.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&name)) {
            set.members.push(WorkshopId(id as u64));
        }
    }
    Ok(sets)
}

fn read_profiles(conn: &Connection) -> Result<Vec<ProfileDef>, Error> {
    let mut stmt = conn.prepare("SELECT data FROM profiles ORDER BY name COLLATE NOCASE")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.map(|row| serde_json::from_str(&row?).map_err(|e| Error::Format(e.to_string())))
        .collect()
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("store types serialize")
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mods_packs_sets_and_profiles_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        let info = ModInfo::unknown(WorkshopId(7));
        store
            .transaction(|tx| {
                tx.upsert_mod(&info)?;
                tx.set_packs(info.id, &["a.pack".into(), "b.pack".into()])?;
                tx.set_subscribed(info.id, false)
            })
            .unwrap();
        assert_eq!(store.mods().unwrap(), vec![(info.clone(), false)]);
        assert_eq!(store.packs().unwrap()[&info.id], ["a.pack", "b.pack"]);

        store
            .save_set(&ModSet {
                name: "Base v1".into(),
                members: vec![info.id],
            })
            .unwrap();
        store
            .transaction(|tx| tx.rename_set("base V1", "Base v2"))
            .unwrap();
        assert_eq!(
            store.sets().unwrap(),
            vec![ModSet {
                name: "Base v2".into(),
                members: vec![info.id]
            }]
        );

        let p = ProfileDef {
            name: "Solo Chaos".into(),
            sets: vec!["Base v2".into()],
            ..Default::default()
        };
        store.save_profile(&p).unwrap();
        let saved = store.profile("solo chaos").unwrap().unwrap();
        assert_eq!(
            saved.set_members["Base v2"],
            [info.id],
            "saving takes a copy of each set"
        );
        assert_eq!(
            ProfileDef {
                set_members: Default::default(),
                ..saved
            },
            p
        );

        store.transaction(|tx| tx.delete_set("Base v2")).unwrap();
        assert!(store.sets().unwrap().is_empty());
    }

    #[test]
    fn profiles_from_before_set_copies_get_one_on_open() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .save_set(&ModSet {
                name: "Base".into(),
                members: vec![WorkshopId(1)],
            })
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO profiles (name, data, updated_at) VALUES ('Old', ?1, 0)",
                [r#"{"name":"Old","sets":["Base"]}"#],
            )
            .unwrap();
        let store = Store::init(store.conn).unwrap();
        let old = store.profile("Old").unwrap().unwrap();
        assert_eq!(old.set_members["Base"], [WorkshopId(1)]);
    }

    #[test]
    fn empty_user_knowledge_is_deleted() {
        let store = Store::open_in_memory().unwrap();
        let k = ModKnowledge {
            tier: Some("ui".into()),
            ..Default::default()
        };
        store.set_user_knowledge(WorkshopId(1), &k).unwrap();
        assert_eq!(store.user_knowledge().unwrap().len(), 1);
        store
            .set_user_knowledge(WorkshopId(1), &ModKnowledge::default())
            .unwrap();
        assert!(store.user_knowledge().unwrap().is_empty());
    }
}
