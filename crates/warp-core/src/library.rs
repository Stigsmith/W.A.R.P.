//! The user's mod library: store + knowledge + taxonomy, and the operations the
//! app and CLI are built on.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::conflicts::{self, ConflictReport};
use crate::import_v1::V1Import;
use crate::install::Install;
use crate::knowledge::{self, KnowledgeBase, ModKnowledge, Resolved};
use crate::launch::{self, ModListEntry};
use crate::model::{ModInfo, WorkshopId, pack_key};
use crate::mp::{self, ListDiff, ShareEntry, ShareList};
use crate::order::{self, OrderItem, OrderResult};
use crate::pack_index::{self, Contents, PackIndex};
use crate::steam;
use crate::store::{ModSet, ProfileDef, Store};
use crate::taxonomy::Taxonomy;

pub struct Library {
    pub store: Store,
    pub kb: KnowledgeBase,
    pub taxonomy: Taxonomy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub info: ModInfo,
    pub packs: Vec<String>,
    pub subscribed: bool,
    /// The merged view used for ordering.
    pub knowledge: Resolved,
    /// The user's own overrides, if any.
    pub user: Option<ModKnowledge>,
    /// What the community knowledge base says, if anything.
    pub community: Option<ModKnowledge>,
    /// What WARP guessed from the packs' contents, headers and Steam tags.
    pub guessed: ModKnowledge,
    pub sets: Vec<String>,
    /// Version on disk (Steam's `time_updated` when downloaded); `None` if not installed.
    pub installed_version: Option<i64>,
    /// Files across the mod's packs, once indexed.
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedProfile {
    pub profile: ProfileDef,
    pub order: OrderResult,
    /// Mods named by the profile that aren't in the library.
    pub unknown_mods: Vec<WorkshopId>,
    /// Mods with no known pack file; they can't be placed.
    pub mods_without_packs: Vec<WorkshopId>,
    /// Mods in the profile the user is no longer subscribed to.
    pub unsubscribed: Vec<WorkshopId>,
    /// (mod, required mod that isn't in the profile).
    pub missing_requirements: Vec<(WorkshopId, WorkshopId)>,
    /// Pairs in the profile that are known not to work together.
    pub incompatibilities: Vec<(WorkshopId, WorkshopId)>,
}

/// What a sync with the installed game found and did.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncSummary {
    pub installed: usize,
    pub new_mods: Vec<WorkshopId>,
    pub unsubscribed: Vec<WorkshopId>,
    pub resubscribed: Vec<WorkshopId>,
    pub packs_indexed: usize,
    pub packs_cached: usize,
    pub pack_errors: Vec<(String, String)>,
    pub steam_refreshed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub mods: usize,
    pub sets: Vec<String>,
    pub profile: Option<String>,
    pub overrides: usize,
    pub unresolved_dependencies: Vec<(String, String)>,
}

impl Library {
    pub fn new(store: Store, kb: KnowledgeBase) -> Self {
        Self {
            store,
            kb,
            taxonomy: Taxonomy::builtin(),
        }
    }

    pub fn entries(&self) -> Result<Vec<LibraryEntry>, Error> {
        let packs = self.store.packs()?;
        let user = self.store.user_knowledge()?;
        let installed = self.store.installed()?;
        let summaries = self.store.pack_summaries()?;
        let contents = self.store.pack_contents()?;
        let declared = self.declared_requires(&packs)?;
        let mut sets_of: HashMap<WorkshopId, Vec<String>> = HashMap::new();
        for set in self.store.sets()? {
            for id in set.members {
                sets_of.entry(id).or_default().push(set.name.clone());
            }
        }
        Ok(self
            .store
            .mods()?
            .into_iter()
            .map(|(info, subscribed)| {
                let id = info.id;
                let packs = packs.get(&id).cloned().unwrap_or_default();
                let parts: Vec<_> = packs
                    .iter()
                    .filter_map(|p| contents.get(&pack_key(p)))
                    .collect();
                let merged = (!parts.is_empty()).then(|| Contents::merge(parts));
                let mut derived = knowledge::guess(&info, &packs, merged.as_ref());
                derived.requires = declared.get(&id).cloned().unwrap_or_default();
                LibraryEntry {
                    knowledge: knowledge::resolve(
                        &self.taxonomy,
                        user.get(&id),
                        self.kb.mods.get(&id),
                        &derived,
                    ),
                    user: user.get(&id).cloned(),
                    community: self.kb.mods.get(&id).cloned(),
                    guessed: derived,
                    sets: sets_of.remove(&id).unwrap_or_default(),
                    installed_version: installed.get(&id).copied(),
                    files: packs
                        .iter()
                        .filter_map(|p| summaries.get(&pack_key(p)))
                        .map(|(n, _)| n)
                        .sum(),
                    info,
                    packs,
                    subscribed,
                }
            })
            .collect())
    }

    /// Mods each mod needs according to its packs' own headers (vanilla packs ignored).
    fn declared_requires(
        &self,
        packs: &HashMap<WorkshopId, Vec<String>>,
    ) -> Result<HashMap<WorkshopId, Vec<WorkshopId>>, Error> {
        let owner: HashMap<String, WorkshopId> = packs
            .iter()
            .flat_map(|(id, names)| names.iter().map(move |n| (pack_key(n), *id)))
            .collect();
        let mut out: HashMap<WorkshopId, Vec<WorkshopId>> = HashMap::new();
        for (pack, deps) in self.store.pack_dependencies()? {
            let Some(&id) = owner.get(&pack) else {
                continue;
            };
            for dep in deps
                .iter()
                .filter_map(|d| owner.get(&pack_key(d)))
                .filter(|&&d| d != id)
            {
                let list = out.entry(id).or_default();
                if !list.contains(dep) {
                    list.push(*dep);
                }
            }
        }
        Ok(out)
    }

    /// Works out a profile's mods and their load order.
    pub fn resolve_profile(&self, def: &ProfileDef) -> Result<ResolvedProfile, Error> {
        let entries: HashMap<WorkshopId, LibraryEntry> = self
            .entries()?
            .into_iter()
            .map(|e| (e.info.id, e))
            .collect();
        let sets = self.store.sets()?;

        // Members: the stacked sets, plus extras, minus exclusions. First mention wins.
        let mut members: Vec<WorkshopId> = Vec::new();
        let mut seen = HashSet::new();
        for name in &def.sets {
            if let Some(set) = sets.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
                members.extend(set.members.iter().filter(|id| seen.insert(**id)));
            }
        }
        members.extend(def.include.iter().filter(|id| seen.insert(**id)));
        let excluded: HashSet<_> = def.exclude.iter().collect();
        members.retain(|id| !excluded.contains(id));
        let in_profile: HashSet<WorkshopId> = members.iter().copied().collect();

        let mut out = ResolvedProfile {
            profile: def.clone(),
            order: OrderResult::default(),
            unknown_mods: vec![],
            mods_without_packs: vec![],
            unsubscribed: vec![],
            missing_requirements: vec![],
            incompatibilities: vec![],
        };
        let mut items = Vec::new();
        let mut incompatible = BTreeSet::new();
        for id in &members {
            let Some(e) = entries.get(id) else {
                out.unknown_mods.push(*id);
                continue;
            };
            if !e.subscribed {
                out.unsubscribed.push(*id);
            }
            for req in &e.knowledge.requires {
                if !in_profile.contains(req) {
                    out.missing_requirements.push((*id, *req));
                }
            }
            for other in &e.knowledge.incompatible_with {
                if in_profile.contains(other) {
                    incompatible.insert(if id < other {
                        (*id, *other)
                    } else {
                        (*other, *id)
                    });
                }
            }
            if e.packs.is_empty() {
                out.mods_without_packs.push(*id);
            }
            let tier = self
                .taxonomy
                .tier(&e.knowledge.tier)
                .unwrap_or(self.taxonomy.fallback_tier());
            let role = self
                .taxonomy
                .role(&e.knowledge.role)
                .unwrap_or(self.taxonomy.default_role());
            items.extend(e.packs.iter().map(|pack| OrderItem {
                pack: pack.clone(),
                workshop_id: Some(*id),
                tier: tier.key.clone(),
                tier_priority: tier.priority,
                role: role.key.clone(),
                role_priority: role.priority,
                requires: e.knowledge.requires.clone(),
                patches: e.knowledge.patches.clone(),
            }));
        }
        out.incompatibilities = incompatible.into_iter().collect();
        out.order = order::solve(&items, &def.pins);
        Ok(out)
    }

    /// A profile's resolved order as a shareable list.
    pub fn share_list(&self, def: &ProfileDef) -> Result<ShareList, Error> {
        let resolved = self.resolve_profile(def)?;
        let times = self.time_updated()?;
        Ok(ShareList {
            name: def.name.clone(),
            entries: resolved
                .order
                .placements
                .iter()
                .map(|p| {
                    let time = p
                        .workshop_id
                        .and_then(|id| times.get(&id).copied())
                        .unwrap_or(0);
                    ShareEntry::new(p.pack.clone(), p.workshop_id, time)
                })
                .collect(),
        })
    }

    /// Turns a plain list of pack names (e.g. a Kaedrin profile) into a share list,
    /// filling in workshop ids and versions from the library where known.
    pub fn share_list_from_packs(&self, name: &str, packs: &[String]) -> Result<ShareList, Error> {
        let by_pack: HashMap<String, WorkshopId> = self
            .store
            .packs()?
            .into_iter()
            .flat_map(|(id, packs)| packs.into_iter().map(move |p| (pack_key(&p), id)))
            .collect();
        let times = self.time_updated()?;
        Ok(ShareList {
            name: name.to_owned(),
            entries: packs
                .iter()
                .map(|p| {
                    let id = by_pack.get(&pack_key(p)).copied();
                    ShareEntry::new(
                        p.clone(),
                        id,
                        id.and_then(|id| times.get(&id).copied()).unwrap_or(0),
                    )
                })
                .collect(),
        })
    }

    /// Fills in pack names a share code left out, for mods in the library.
    pub fn fill_pack_names(&self, list: &mut ShareList) -> Result<(), Error> {
        let packs = self.store.packs()?;
        for e in list.entries.iter_mut().filter(|e| e.pack.is_empty()) {
            if let Some([only]) = e
                .workshop_id
                .and_then(|id| packs.get(&id))
                .map(Vec::as_slice)
            {
                e.pack = only.clone();
            }
        }
        Ok(())
    }

    /// Compares two lists (A = mine, B = theirs, by convention). With `check_steam`,
    /// version mismatches whose stale side isn't known yet are settled by asking Steam.
    pub fn compare(
        &self,
        a: &mut ShareList,
        b: &mut ShareList,
        check_steam: bool,
    ) -> Result<ListDiff, Error> {
        self.fill_pack_names(a)?;
        self.fill_pack_names(b)?;
        let mut d = mp::diff(a, b);
        let unsettled: Vec<WorkshopId> = d
            .version_mismatches
            .iter()
            .filter(|m| m.stale.is_none())
            .filter_map(|m| m.workshop_id)
            .collect();
        if check_steam && !unsettled.is_empty() {
            let current = steam::fetch_details(&unsettled)?
                .into_iter()
                .map(|m| (m.id, m.time_updated))
                .collect();
            mp::resolve_stale(&mut d, a, b, &current);
        }
        Ok(d)
    }

    /// The version each mod is at: the installed copy when there is one (that's what the
    /// game will load), else Steam's latest.
    fn time_updated(&self) -> Result<HashMap<WorkshopId, i64>, Error> {
        let mut out: HashMap<WorkshopId, i64> = self
            .store
            .mods()?
            .into_iter()
            .map(|(m, _)| (m.id, m.time_updated))
            .collect();
        out.extend(self.store.installed()?);
        Ok(out)
    }

    /// Saves the user's knowledge for a mod, keeping only what differs from the community KB.
    pub fn set_user_knowledge(&self, id: WorkshopId, k: &ModKnowledge) -> Result<(), Error> {
        let community = self.kb.mods.get(&id).cloned().unwrap_or_default();
        self.store.set_user_knowledge(id, &overrides(k, &community))
    }

    /// Imports a v1 workbook: Steam metadata, packs, subscriptions, sets (from
    /// Components) and the Profile Builder profile. Knowledge that differs from
    /// the community KB is kept as the user's own overrides.
    pub fn import_v1(&mut self, import: &V1Import) -> Result<ImportSummary, Error> {
        let mut sets: Vec<ModSet> = Vec::new();
        for m in &import.mods {
            if let Some(c) = &m.component {
                match sets.iter_mut().find(|s| s.name.eq_ignore_ascii_case(c)) {
                    Some(set) => set.members.push(m.info.id),
                    None => sets.push(ModSet {
                        name: c.clone(),
                        members: vec![m.info.id],
                    }),
                }
            }
        }
        let profile = import
            .profile
            .as_ref()
            .map(|(name, components)| ProfileDef {
                name: name.clone(),
                sets: components.clone(),
                ..ProfileDef::default()
            });

        let kb = &self.kb;
        let overrides: Vec<(WorkshopId, ModKnowledge)> = import
            .mods
            .iter()
            .map(|m| {
                let community = kb.mods.get(&m.info.id).cloned().unwrap_or_default();
                (m.info.id, overrides(&m.knowledge, &community))
            })
            .collect();

        // The workbook is a snapshot from the past: it fills gaps but never overwrites
        // what a sync with the installed game (or Steam) already knows.
        let known: HashMap<WorkshopId, ModInfo> = self
            .store
            .mods()?
            .into_iter()
            .map(|(m, _)| (m.id, m))
            .collect();
        let has_packs = self.store.packs()?;
        let synced = !self.store.installed()?.is_empty();
        self.store.transaction(|tx| {
            for m in &import.mods {
                let fresher = known
                    .get(&m.info.id)
                    .is_some_and(|k| !k.title.is_empty() && k.time_updated >= m.info.time_updated);
                if !fresher {
                    tx.upsert_mod(&m.info)?;
                }
                if !m.pack.is_empty() && !has_packs.contains_key(&m.info.id) {
                    tx.set_packs(m.info.id, std::slice::from_ref(&m.pack))?;
                }
                if !synced {
                    tx.set_subscribed(m.info.id, !m.archived)?;
                }
            }
            for set in &sets {
                tx.save_set(set)?;
            }
            if let Some(p) = &profile {
                tx.save_profile(p)?;
            }
            Ok(())
        })?;
        let mut override_count = 0;
        for (id, k) in &overrides {
            if !k.is_empty() {
                override_count += 1;
            }
            self.store.set_user_knowledge(*id, k)?;
        }

        Ok(ImportSummary {
            mods: import.mods.len(),
            sets: sets.into_iter().map(|s| s.name).collect(),
            profile: profile.map(|p| p.name),
            overrides: override_count,
            unresolved_dependencies: import.unresolved_dependencies.clone(),
        })
    }

    /// Refreshes Steam metadata for the given mods (all mods when `None`).
    /// Returns how many items Steam answered for.
    pub fn refresh_from_steam(&mut self, ids: Option<&[WorkshopId]>) -> Result<usize, Error> {
        let ids: Vec<WorkshopId> = match ids {
            Some(ids) => ids.to_vec(),
            None => self.store.mods()?.into_iter().map(|(m, _)| m.id).collect(),
        };
        let fresh = steam::fetch_details(&ids)?;
        let known: HashMap<WorkshopId, ModInfo> = self
            .store
            .mods()?
            .into_iter()
            .map(|(m, _)| (m.id, m))
            .collect();
        self.store.transaction(|tx| {
            for info in &fresh {
                // A removed item loses its details on Steam; keep what we knew.
                let merged = match (info.available, known.get(&info.id)) {
                    (false, Some(old)) => ModInfo {
                        available: false,
                        ..old.clone()
                    },
                    _ => info.clone(),
                };
                tx.upsert_mod(&merged)?;
            }
            Ok(())
        })?;
        Ok(fresh.len())
    }

    /// Brings the library in line with what's installed: subscriptions, installed
    /// versions, packs, and the pack index. With `check_steam`, also refreshes Steam
    /// metadata for new mods and ones that changed.
    pub fn sync_install(
        &mut self,
        install: &Install,
        check_steam: bool,
    ) -> Result<SyncSummary, Error> {
        let items = install.installed_items()?;
        let known: HashMap<WorkshopId, (ModInfo, bool)> = self
            .store
            .mods()?
            .into_iter()
            .map(|(m, sub)| (m.id, (m, sub)))
            .collect();
        let installed_ids: HashSet<WorkshopId> = items.iter().map(|i| i.id).collect();

        let mut summary = SyncSummary {
            installed: items.len(),
            ..SyncSummary::default()
        };
        self.store.transaction(|tx| {
            for item in &items {
                match known.get(&item.id) {
                    None => {
                        let mut info = ModInfo::unknown(item.id);
                        info.time_updated = item.time_updated;
                        tx.upsert_mod(&info)?;
                        summary.new_mods.push(item.id);
                    }
                    Some((_, false)) => summary.resubscribed.push(item.id),
                    Some(_) => {}
                }
                let names: Vec<String> = item
                    .packs
                    .iter()
                    .filter_map(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .collect();
                tx.set_packs(item.id, &names)?;
                tx.set_subscribed(item.id, true)?;
            }
            for (id, (_, subscribed)) in &known {
                if *subscribed && !installed_ids.contains(id) {
                    tx.set_subscribed(*id, false)?;
                    summary.unsubscribed.push(*id);
                }
            }
            let rows: Vec<(WorkshopId, i64, u64)> = items
                .iter()
                .map(|i| (i.id, i.time_updated, i.size))
                .collect();
            tx.replace_installed(&rows)
        })?;

        // Pack X-ray: re-read only packs whose size or modification time changed.
        let stamps = self.store.pack_index_stamps()?;
        let wanted: Vec<(WorkshopId, PathBuf)> = items
            .iter()
            .flat_map(|i| i.packs.iter().map(move |p| (i.id, p.clone())))
            .collect();
        let stale: Vec<&(WorkshopId, PathBuf)> = wanted
            .iter()
            .filter(|(_, path)| {
                let stamp = std::fs::metadata(path)
                    .ok()
                    .map(|m| (m.len(), pack_index::modified_secs(&m)));
                stamp.is_none()
                    || stamp != stamps.get(&path.to_string_lossy().into_owned()).copied()
            })
            .collect();
        let read: Vec<(WorkshopId, String, Result<PackIndex, Error>)> = stale
            .par_iter()
            .map(|(id, path)| {
                (
                    *id,
                    path.to_string_lossy().into_owned(),
                    pack_index::read(path),
                )
            })
            .collect();
        let keep: HashSet<String> = wanted
            .iter()
            .map(|(_, p)| p.to_string_lossy().into_owned())
            .collect();
        self.store.transaction(|tx| {
            for (id, path, result) in &read {
                match result {
                    Ok(index) => {
                        tx.put_pack_index(path, Some(*id), index)?;
                        summary.packs_indexed += 1;
                    }
                    Err(e) => summary.pack_errors.push((path.clone(), e.to_string())),
                }
            }
            tx.retain_pack_index(&keep).map(drop)
        })?;
        summary.packs_cached = wanted.len() - stale.len();

        if check_steam {
            // New mods, and ones whose installed copy is newer than what Steam last told us.
            let to_fetch: Vec<WorkshopId> = items
                .iter()
                .filter(|i| {
                    known
                        .get(&i.id)
                        .is_none_or(|(m, _)| m.title.is_empty() || m.time_updated < i.time_updated)
                })
                .map(|i| i.id)
                .collect();
            if !to_fetch.is_empty() {
                summary.steam_refreshed = self.refresh_from_steam(Some(&to_fetch))?;
            }
        }
        Ok(summary)
    }

    /// The conflict map for a profile's load order.
    pub fn conflicts(&self, def: &ProfileDef) -> Result<ConflictReport, Error> {
        let resolved = self.resolve_profile(def)?;
        let order: Vec<String> = resolved
            .order
            .placements
            .iter()
            .map(|p| p.pack.clone())
            .collect();
        let indexes = self.store.pack_indexes(&order)?;

        // Who is meant to override whom: known relations between the mods, or a pack
        // naming the other as a dependency in its header.
        let entries: HashMap<WorkshopId, LibraryEntry> = self
            .entries()?
            .into_iter()
            .map(|e| (e.info.id, e))
            .collect();
        let mod_of: HashMap<String, WorkshopId> = resolved
            .order
            .placements
            .iter()
            .filter_map(|p| Some((pack_key(&p.pack), p.workshop_id?)))
            .collect();
        let intended = |winner: &str, loser: &str| {
            let declared = indexes.get(&pack_key(winner)).is_some_and(|i| {
                i.dependencies
                    .iter()
                    .any(|d| pack_key(d) == pack_key(loser))
            });
            let related = match (mod_of.get(&pack_key(winner)), mod_of.get(&pack_key(loser))) {
                (Some(w), Some(l)) => entries.get(w).is_some_and(|e| {
                    e.knowledge.requires.contains(l) || e.knowledge.patches.contains(l)
                }),
                _ => false,
            };
            declared || related
        };
        Ok(conflicts::analyze(&order, &indexes, &intended))
    }

    /// The modlist for a profile: every pack in load order with the folder it's in.
    pub fn mod_list_entries(
        &self,
        def: &ProfileDef,
        install: &Install,
    ) -> Result<Vec<ModListEntry>, Error> {
        let resolved = self.resolve_profile(def)?;
        Ok(resolved
            .order
            .placements
            .iter()
            .map(|p| {
                let dir = p
                    .workshop_id
                    .map(|id| install.workshop_dir.join(id.to_string()))
                    .filter(|d| d.join(&p.pack).is_file());
                ModListEntry {
                    pack: p.pack.clone(),
                    dir,
                }
            })
            .collect())
    }

    /// Writes the profile's modlist and starts the game. Returns the modlist path.
    pub fn play(
        &self,
        def: &ProfileDef,
        install: &Install,
        continue_save: Option<&str>,
    ) -> Result<PathBuf, Error> {
        let entries = self.mod_list_entries(def, install)?;
        let missing: Vec<&str> = entries
            .iter()
            .filter(|e| e.dir.is_none() && !install.data_dir().join(&e.pack).is_file())
            .map(|e| e.pack.as_str())
            .collect();
        if !missing.is_empty() {
            return Err(Error::Invalid(format!(
                "{} pack(s) in this profile aren't installed: {}",
                missing.len(),
                missing
                    .iter()
                    .take(5)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let path = launch::write_mod_list(install, &entries)?;
        launch::launch(install, continue_save)?;
        let packs: Vec<String> = entries.into_iter().map(|e| e.pack).collect();
        self.store.record_launch(&def.name, &packs)?;
        Ok(path)
    }
}

/// The parts of `mine` that differ from `community` (the title is never an override).
fn overrides(mine: &ModKnowledge, community: &ModKnowledge) -> ModKnowledge {
    fn diff<T: PartialEq + Clone + Default>(mine: &T, theirs: &T) -> T {
        if mine == theirs {
            T::default()
        } else {
            mine.clone()
        }
    }
    ModKnowledge {
        title: String::new(),
        tier: diff(&mine.tier, &community.tier),
        role: diff(&mine.role, &community.role),
        tags: diff(&mine.tags, &community.tags),
        factions: diff(&mine.factions, &community.factions),
        units: diff(&mine.units, &community.units),
        requires: diff(&mine.requires, &community.requires),
        patches: diff(&mine.patches, &community.patches),
        incompatible_with: diff(&mine.incompatible_with, &community.incompatible_with),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib_with(mods: &[(u64, &str, &str, &str)], kb_entries: &[(u64, ModKnowledge)]) -> Library {
        let mut store = Store::open_in_memory().unwrap();
        store
            .transaction(|tx| {
                for (id, pack, _, _) in mods {
                    let mut info = ModInfo::unknown(WorkshopId(*id));
                    info.time_updated = 1_700_000_000 + *id as i64;
                    tx.upsert_mod(&info)?;
                    tx.set_packs(info.id, &[(*pack).to_owned()])?;
                }
                Ok(())
            })
            .unwrap();
        let mut kb = KnowledgeBase::new();
        for (id, _, tier, role) in mods {
            kb.mods.insert(
                WorkshopId(*id),
                ModKnowledge {
                    tier: Some((*tier).into()),
                    role: Some((*role).into()),
                    ..Default::default()
                },
            );
        }
        for (id, k) in kb_entries {
            kb.mods.insert(WorkshopId(*id), k.clone());
        }
        Library::new(store, kb)
    }

    #[test]
    fn profile_stacks_sets_and_orders_by_tier() {
        let mut lib = lib_with(
            &[
                (1, "core.pack", "core", "framework"),
                (2, "ui.pack", "ui", "content"),
                (3, "units.pack", "units", "content"),
            ],
            &[],
        );
        lib.store
            .save_set(&ModSet {
                name: "Base".into(),
                members: vec![WorkshopId(1), WorkshopId(3)],
            })
            .unwrap();
        lib.store
            .save_set(&ModSet {
                name: "Extra".into(),
                members: vec![WorkshopId(2)],
            })
            .unwrap();
        let def = ProfileDef {
            name: "p".into(),
            sets: vec!["Base".into(), "extra".into()],
            exclude: vec![WorkshopId(3)],
            ..Default::default()
        };
        let r = lib.resolve_profile(&def).unwrap();
        assert_eq!(r.order.packs(), ["ui.pack", "core.pack"]);

        let share = lib.share_list(&def).unwrap();
        assert_eq!(share.entries[0].workshop_id, Some(WorkshopId(2)));
        assert_eq!(share.entries[0].time_updated, 1_700_000_002);
    }

    #[test]
    fn missing_requirements_and_incompatibilities_are_reported() {
        let needs = ModKnowledge {
            tier: Some("ui".into()),
            requires: vec![WorkshopId(9)],
            incompatible_with: vec![WorkshopId(2)],
            ..Default::default()
        };
        let mut lib = lib_with(
            &[
                (1, "a.pack", "ui", "content"),
                (2, "b.pack", "core", "content"),
            ],
            &[(1, needs)],
        );
        lib.store
            .save_set(&ModSet {
                name: "S".into(),
                members: vec![WorkshopId(1), WorkshopId(2)],
            })
            .unwrap();
        let r = lib
            .resolve_profile(&ProfileDef {
                name: "p".into(),
                sets: vec!["S".into()],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.missing_requirements, [(WorkshopId(1), WorkshopId(9))]);
        assert_eq!(r.incompatibilities, [(WorkshopId(1), WorkshopId(2))]);
    }

    #[test]
    fn user_knowledge_stores_only_differences() {
        let lib = lib_with(&[(1, "a.pack", "ui", "content")], &[]);
        let same = ModKnowledge {
            tier: Some("ui".into()),
            role: Some("content".into()),
            ..Default::default()
        };
        lib.set_user_knowledge(WorkshopId(1), &same).unwrap();
        assert!(lib.store.user_knowledge().unwrap().is_empty());
        let changed = ModKnowledge {
            tier: Some("battle".into()),
            role: Some("content".into()),
            ..Default::default()
        };
        lib.set_user_knowledge(WorkshopId(1), &changed).unwrap();
        let stored = &lib.store.user_knowledge().unwrap()[&WorkshopId(1)];
        assert_eq!(stored.tier.as_deref(), Some("battle"));
        assert_eq!(stored.role, None);
    }

    #[test]
    fn importing_v1_after_a_sync_only_fills_gaps() {
        use crate::import_v1::{V1Import, V1Mod};
        let mut lib = lib_with(&[(1, "real.pack", "ui", "content")], &[]);
        let mut fresh = ModInfo::unknown(WorkshopId(1));
        fresh.title = "Fresh from Steam".into();
        fresh.time_updated = 2_000;
        lib.store
            .transaction(|tx| {
                tx.upsert_mod(&fresh)?;
                tx.replace_installed(&[(WorkshopId(1), 2_000, 10)])
            })
            .unwrap();

        let old = |id: u64, title: &str, pack: &str| V1Mod {
            info: ModInfo {
                title: title.into(),
                time_updated: 1_000,
                ..ModInfo::unknown(WorkshopId(id))
            },
            pack: pack.into(),
            knowledge: ModKnowledge::default(),
            component: Some("Base".into()),
            archived: true,
            source_category: String::new(),
            source_subcategory: String::new(),
        };
        let import = V1Import {
            mods: vec![
                old(1, "Old title", "old.pack"),
                old(2, "Only in the workbook", "two.pack"),
            ],
            profile: None,
            unresolved_dependencies: vec![],
        };
        lib.import_v1(&import).unwrap();

        let mods: HashMap<WorkshopId, (ModInfo, bool)> = lib
            .store
            .mods()
            .unwrap()
            .into_iter()
            .map(|(m, s)| (m.id, (m, s)))
            .collect();
        assert_eq!(mods[&WorkshopId(1)].0.title, "Fresh from Steam");
        assert!(
            mods[&WorkshopId(1)].1,
            "subscription state from the sync is kept"
        );
        assert_eq!(lib.store.packs().unwrap()[&WorkshopId(1)], ["real.pack"]);
        assert_eq!(
            mods[&WorkshopId(2)].0.title,
            "Only in the workbook",
            "gaps are filled"
        );
        assert_eq!(
            lib.store.sets().unwrap()[0].members.len(),
            2,
            "sets still come across"
        );
    }

    #[test]
    fn kaedrin_packs_map_to_mods() {
        let lib = lib_with(&[(1, "Aekold Reskin.pack", "units", "content")], &[]);
        let l = lib
            .share_list_from_packs("k", &["aekold reskin.pack".into(), "unknown.pack".into()])
            .unwrap();
        assert_eq!(l.entries[0].workshop_id, Some(WorkshopId(1)));
        assert_eq!(l.entries[1].workshop_id, None);
    }
}
