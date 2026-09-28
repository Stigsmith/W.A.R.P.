//! Per-user storage (SQLite): the Steam metadata cache, packs, subscriptions,
//! the user's knowledge overrides, sets and profiles.

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::knowledge::ModKnowledge;
use crate::model::{ModInfo, WorkshopId};
use crate::order::Pin;

/// Schema migrations, applied in order. Never edit a released entry; append a new one.
const MIGRATIONS: &[&str] = &[r"
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
"];

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
            return Err(Error::Format("this database was created by a newer WARP".into()));
        }
        let tx = conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    /// Runs `f` in one transaction: all of it lands, or none of it.
    pub fn transaction<T>(&mut self, f: impl FnOnce(&Tx<'_>) -> Result<T, Error>) -> Result<T, Error> {
        let tx = Tx(self.conn.transaction()?);
        let out = f(&tx)?;
        tx.0.commit()?;
        Ok(out)
    }

    pub fn mods(&self) -> Result<Vec<(ModInfo, bool)>, Error> {
        let mut stmt = self.conn.prepare("SELECT info, subscribed FROM mods ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))?;
        rows.map(|row| {
            let (json, subscribed) = row?;
            Ok((serde_json::from_str(&json).map_err(|e| Error::Format(e.to_string()))?, subscribed))
        })
        .collect()
    }

    pub fn packs(&self) -> Result<HashMap<WorkshopId, Vec<String>>, Error> {
        let mut stmt = self.conn.prepare("SELECT mod_id, name FROM packs ORDER BY mod_id, name")?;
        let mut out: HashMap<WorkshopId, Vec<String>> = HashMap::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            out.entry(WorkshopId(id as u64)).or_default().push(name);
        }
        Ok(out)
    }

    pub fn user_knowledge(&self) -> Result<HashMap<WorkshopId, ModKnowledge>, Error> {
        let mut stmt = self.conn.prepare("SELECT mod_id, data FROM user_knowledge")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (id, json) = row?;
            Ok((WorkshopId(id as u64), serde_json::from_str(&json).map_err(|e| Error::Format(e.to_string()))?))
        })
        .collect()
    }

    pub fn set_user_knowledge(&self, id: WorkshopId, k: &ModKnowledge) -> Result<(), Error> {
        if k.is_empty() {
            self.conn.execute("DELETE FROM user_knowledge WHERE mod_id = ?1", params![id.0 as i64])?;
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
        let mut sets: Vec<ModSet> = self
            .conn
            .prepare("SELECT name FROM sets ORDER BY name COLLATE NOCASE")?
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|name| Ok(ModSet { name: name?, members: Vec::new() }))
            .collect::<Result<_, Error>>()?;
        let mut stmt = self.conn.prepare("SELECT set_name, mod_id FROM set_members ORDER BY mod_id")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (name, id) = row?;
            if let Some(set) = sets.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&name)) {
                set.members.push(WorkshopId(id as u64));
            }
        }
        Ok(sets)
    }

    /// Creates the set if needed and replaces its members.
    pub fn save_set(&mut self, set: &ModSet) -> Result<(), Error> {
        self.transaction(|tx| tx.save_set(set))
    }

    pub fn rename_set(&self, from: &str, to: &str) -> Result<(), Error> {
        self.conn.execute("UPDATE sets SET name = ?2 WHERE name = ?1", params![from, to.trim()])?;
        Ok(())
    }

    pub fn delete_set(&self, name: &str) -> Result<(), Error> {
        self.conn.execute("DELETE FROM sets WHERE name = ?1", params![name])?;
        Ok(())
    }

    pub fn profiles(&self) -> Result<Vec<ProfileDef>, Error> {
        let mut stmt = self.conn.prepare("SELECT data FROM profiles ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(|e| Error::Format(e.to_string()))).collect()
    }

    pub fn profile(&self, name: &str) -> Result<Option<ProfileDef>, Error> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT data FROM profiles WHERE name = ?1", params![name], |r| r.get(0))
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(|e| Error::Format(e.to_string()))).transpose()
    }

    pub fn save_profile(&mut self, profile: &ProfileDef) -> Result<(), Error> {
        self.transaction(|tx| tx.save_profile(profile))
    }

    pub fn delete_profile(&self, name: &str) -> Result<(), Error> {
        self.conn.execute("DELETE FROM profiles WHERE name = ?1", params![name])?;
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
        self.0.execute("UPDATE mods SET subscribed = ?2 WHERE id = ?1", params![id.0 as i64, subscribed])?;
        Ok(())
    }

    /// Replaces the packs recorded for a mod.
    pub fn set_packs(&self, id: WorkshopId, packs: &[String]) -> Result<(), Error> {
        self.0.execute("DELETE FROM packs WHERE mod_id = ?1", params![id.0 as i64])?;
        for p in packs {
            self.0.execute("INSERT OR IGNORE INTO packs (mod_id, name) VALUES (?1, ?2)", params![id.0 as i64, p])?;
        }
        Ok(())
    }

    pub fn save_set(&self, set: &ModSet) -> Result<(), Error> {
        let name = set.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a set needs a name".into()));
        }
        self.0.execute("INSERT OR IGNORE INTO sets (name) VALUES (?1)", params![name])?;
        self.0.execute("DELETE FROM set_members WHERE set_name = ?1", params![name])?;
        for id in &set.members {
            self.0.execute(
                "INSERT OR IGNORE INTO set_members (set_name, mod_id) VALUES (?1, ?2)",
                params![name, id.0 as i64],
            )?;
        }
        Ok(())
    }

    pub fn save_profile(&self, profile: &ProfileDef) -> Result<(), Error> {
        if profile.name.trim().is_empty() {
            return Err(Error::Invalid("a profile needs a name".into()));
        }
        self.0.execute(
            "INSERT INTO profiles (name, data, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (name) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
            params![profile.name.trim(), to_json(profile), now()],
        )?;
        Ok(())
    }
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("store types serialize")
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
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

        store.save_set(&ModSet { name: "Base v1".into(), members: vec![info.id] }).unwrap();
        store.rename_set("base V1", "Base v2").unwrap();
        assert_eq!(store.sets().unwrap(), vec![ModSet { name: "Base v2".into(), members: vec![info.id] }]);

        let p = ProfileDef { name: "Solo Chaos".into(), sets: vec!["Base v2".into()], ..Default::default() };
        store.save_profile(&p).unwrap();
        assert_eq!(store.profile("solo chaos").unwrap(), Some(p));

        store.delete_set("Base v2").unwrap();
        assert!(store.sets().unwrap().is_empty());
    }

    #[test]
    fn empty_user_knowledge_is_deleted() {
        let store = Store::open_in_memory().unwrap();
        let k = ModKnowledge { tier: Some("ui".into()), ..Default::default() };
        store.set_user_knowledge(WorkshopId(1), &k).unwrap();
        assert_eq!(store.user_knowledge().unwrap().len(), 1);
        store.set_user_knowledge(WorkshopId(1), &ModKnowledge::default()).unwrap();
        assert!(store.user_knowledge().unwrap().is_empty());
    }
}
