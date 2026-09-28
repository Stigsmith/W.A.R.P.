//! Multiplayer sync: share codes and modlist comparison.
//!
//! Multiplayer only works when both players run the same packs, in the same
//! order, at the same version. A share code carries exactly that, compactly
//! enough to paste into one Discord message:
//!
//! - mods are identified by workshop id; pack names are left out whenever the
//!   receiver can look them up from the id,
//! - versions are a 16-bit fingerprint of Steam's `time_updated`, enough to spot a
//!   mismatch. Which side is stale is then settled against Steam's current value
//!   ([`resolve_stale`]).

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::model::{WorkshopId, pack_key};

pub const CODE_PREFIX: &str = "WARP1:";
const PAYLOAD_VERSION: u8 = 1;
const WARP_FILE_FORMAT: u32 = 1;

/// One pack in a shared list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareEntry {
    /// Pack file name. Empty in a decoded share code when the workshop id is enough
    /// (see [`crate::library::Library::fill_pack_names`]).
    pub pack: String,
    pub workshop_id: Option<WorkshopId>,
    /// Steam's `time_updated` for the mod (unix seconds, 0 = unknown). Not carried by share codes.
    #[serde(default)]
    pub time_updated: i64,
    /// Fingerprint of `time_updated`: what share codes carry instead of the timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u16>,
}

impl ShareEntry {
    pub fn new(pack: impl Into<String>, workshop_id: Option<WorkshopId>, time_updated: i64) -> Self {
        Self { pack: pack.into(), workshop_id, time_updated, version: None }
    }

    /// The version fingerprint, from the timestamp when known.
    pub fn version_tag(&self) -> Option<u16> {
        version_tag(self.time_updated).or(self.version)
    }

    /// A name to show: the pack file, else the workshop id.
    pub fn label(&self) -> String {
        match (&self.pack, self.workshop_id) {
            (p, _) if !p.is_empty() => p.clone(),
            (_, Some(id)) => format!("workshop item {id}"),
            _ => "(unknown pack)".to_owned(),
        }
    }
}

/// A modlist as exchanged between players. Top of the load order first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareList {
    pub name: String,
    pub entries: Vec<ShareEntry>,
}

/// 16-bit fingerprint of a `time_updated` (FNV-1a folded). 0 is reserved for "unknown".
pub fn version_tag(time_updated: i64) -> Option<u16> {
    if time_updated <= 0 {
        return None;
    }
    let mut h: u32 = 0x811c_9dc5;
    for b in time_updated.to_le_bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    let tag = ((h >> 16) ^ (h & 0xffff)) as u16;
    Some(if tag == 0 { 1 } else { tag })
}

/// Encodes a list as `WARP1:<base64url(deflate(payload))>`.
pub fn encode(list: &ShareList) -> String {
    let ids = id_counts(list);
    let mut payload = vec![PAYLOAD_VERSION];
    put_bytes(&mut payload, truncate(&list.name, 64).as_bytes());
    put_varint(&mut payload, list.entries.len() as u64);
    // Columnar layout (ids, versions, names) compresses far better than rows.
    for e in &list.entries {
        put_varint(&mut payload, e.workshop_id.map_or(0, |id| id.0));
    }
    for e in &list.entries {
        payload.extend_from_slice(&e.version_tag().unwrap_or(0).to_le_bytes());
    }
    for e in &list.entries {
        // The receiver can look a single-pack mod's name up by id; send the rest.
        let needs_name = e.workshop_id.is_none_or(|id| ids[&id] > 1);
        put_bytes(&mut payload, if needs_name { e.pack.as_bytes() } else { b"" });
    }

    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&payload).expect("writing to a Vec cannot fail");
    let compressed = enc.finish().expect("writing to a Vec cannot fail");
    format!("{CODE_PREFIX}{}", URL_SAFE_NO_PAD.encode(compressed))
}

/// Decodes a share code. Accepts a whole pasted message: the code is found
/// wherever it sits, with Discord formatting and line breaks ignored.
pub fn decode(text: &str) -> Result<ShareList, Error> {
    let start = text.find(CODE_PREFIX).ok_or_else(|| bad("no WARP share code found"))?;
    let body: String = text[start + CODE_PREFIX.len()..]
        .chars()
        .filter(|c| !c.is_whitespace())
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let compressed = URL_SAFE_NO_PAD.decode(body).map_err(|_| bad("the code is damaged (invalid characters)"))?;
    let mut payload = Vec::new();
    DeflateDecoder::new(&compressed[..])
        .take(4 << 20)
        .read_to_end(&mut payload)
        .map_err(|_| bad("the code is damaged (incomplete or altered)"))?;

    let mut r = Reader { buf: &payload, pos: 0 };
    if r.byte()? != PAYLOAD_VERSION {
        return Err(bad("this code was made by a newer WARP; update to read it"));
    }
    let name = r.string()?;
    let count = r.varint()? as usize;
    if count > 10_000 {
        return Err(bad("the code is damaged (implausible size)"));
    }
    let ids: Vec<u64> = (0..count).map(|_| r.varint()).collect::<Result<_, _>>()?;
    let tags: Vec<u16> = (0..count).map(|_| Ok(u16::from_le_bytes([r.byte()?, r.byte()?]))).collect::<Result<_, Error>>()?;
    let packs: Vec<String> = (0..count).map(|_| r.string()).collect::<Result<_, _>>()?;
    let entries = packs
        .into_iter()
        .zip(ids)
        .zip(tags)
        .map(|((pack, id), tag)| ShareEntry {
            pack,
            workshop_id: (id != 0).then_some(WorkshopId(id)),
            time_updated: 0,
            version: (tag != 0).then_some(tag),
        })
        .collect();
    Ok(ShareList { name, entries })
}

#[derive(Serialize, Deserialize)]
struct WarpFile {
    warp: u32,
    #[serde(flatten)]
    list: ShareList,
}

/// The `.warp` file form of a list: complete (names and timestamps), for lists too long to paste.
pub fn to_warp_file(list: &ShareList) -> String {
    serde_json::to_string_pretty(&WarpFile { warp: WARP_FILE_FORMAT, list: list.clone() }).expect("serializes")
}

pub fn from_warp_file(json: &str) -> Result<ShareList, Error> {
    let file: WarpFile = serde_json::from_str(json).map_err(|e| bad(&format!("not a .warp file: {e}")))?;
    if file.warp > WARP_FILE_FORMAT {
        return Err(bad("this .warp file was made by a newer WARP; update to read it"));
    }
    Ok(file.list)
}

/// Which list a finding refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    A,
    B,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffEntry {
    pub pack: String,
    pub workshop_id: Option<WorkshopId>,
    /// Position in the list it appears in (0 = top).
    pub position: usize,
}

/// One step to turn list A's order into list B's. Apply the steps top to bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub pack: String,
    pub workshop_id: Option<WorkshopId>,
    /// Put it directly below this pack; `None` means at the very top.
    pub below: Option<String>,
    pub below_workshop_id: Option<WorkshopId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionMismatch {
    pub pack: String,
    pub workshop_id: Option<WorkshopId>,
    /// 0 when that side's list didn't carry a timestamp (e.g. a share code).
    pub a_time_updated: i64,
    pub b_time_updated: i64,
    /// Who has an outdated copy and needs Steam to re-download it. `None` until
    /// known: either both timestamps are known, or [`resolve_stale`] checked Steam.
    pub stale: Option<Side>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDiff {
    pub identical: bool,
    pub common: usize,
    pub only_in_a: Vec<DiffEntry>,
    pub only_in_b: Vec<DiffEntry>,
    /// The fewest moves that make A's shared packs follow B's order.
    pub moves: Vec<Move>,
    pub version_mismatches: Vec<VersionMismatch>,
}

/// Compares two lists. Entries match by workshop id when both sides have one,
/// otherwise by pack file name (ignoring case).
pub fn diff(a: &ShareList, b: &ShareList) -> ListDiff {
    let b_to_a = match_entries(a, b);
    let matched_a: HashSet<usize> = b_to_a.iter().flatten().copied().collect();

    let only_in_a = entries_where(a, |i| !matched_a.contains(&i));
    let only_in_b = entries_where(b, |i| b_to_a[i].is_none());

    // Shared entries in B's order: (a entry, b entry, position in A).
    let shared: Vec<(&ShareEntry, &ShareEntry, usize)> =
        b.entries.iter().zip(&b_to_a).filter_map(|(be, ai)| ai.map(|ai| (&a.entries[ai], be, ai))).collect();
    let a_positions: Vec<usize> = shared.iter().map(|&(_, _, ai)| ai).collect();
    let keep = longest_increasing_subsequence(&a_positions);

    let moves = (0..shared.len())
        .filter(|i| !keep.contains(i))
        .map(|i| {
            let (ae, be, _) = shared[i];
            let prev = i.checked_sub(1).map(|p| shared[p].0);
            Move {
                pack: best_label(ae, be),
                workshop_id: be.workshop_id.or(ae.workshop_id),
                below: prev.map(ShareEntry::label),
                below_workshop_id: prev.and_then(|p| p.workshop_id),
            }
        })
        .collect::<Vec<_>>();

    let version_mismatches: Vec<VersionMismatch> = shared
        .iter()
        .filter_map(|&(ae, be, _)| {
            let (ta, tb) = (ae.version_tag()?, be.version_tag()?);
            (ta != tb).then(|| VersionMismatch {
                pack: best_label(ae, be),
                workshop_id: be.workshop_id.or(ae.workshop_id),
                a_time_updated: ae.time_updated,
                b_time_updated: be.time_updated,
                stale: (ae.time_updated > 0 && be.time_updated > 0)
                    .then_some(if ae.time_updated < be.time_updated { Side::A } else { Side::B }),
            })
        })
        .collect();

    ListDiff {
        identical: only_in_a.is_empty() && only_in_b.is_empty() && moves.is_empty() && version_mismatches.is_empty(),
        common: shared.len(),
        only_in_a,
        only_in_b,
        moves,
        version_mismatches,
    }
}

/// Settles who is stale using Steam's current `time_updated` per mod. The side
/// whose version matches Steam is up to date.
pub fn resolve_stale(diff: &mut ListDiff, a: &ShareList, b: &ShareList, steam_current: &HashMap<WorkshopId, i64>) {
    let tag_of = |list: &ShareList, id: WorkshopId| list.entries.iter().find(|e| e.workshop_id == Some(id)).and_then(ShareEntry::version_tag);
    for m in &mut diff.version_mismatches {
        let Some(id) = m.workshop_id else { continue };
        let Some(current) = steam_current.get(&id).and_then(|&t| version_tag(t)) else { continue };
        let (a_ok, b_ok) = (tag_of(a, id) == Some(current), tag_of(b, id) == Some(current));
        m.stale = Some(match (a_ok, b_ok) {
            (true, false) => Side::B,
            (false, true) => Side::A,
            _ => Side::Both,
        });
    }
}

/// For each entry of `b`, the index of the matching entry in `a`, if any.
fn match_entries(a: &ShareList, b: &ShareList) -> Vec<Option<usize>> {
    let (a_ids, b_ids) = (id_counts(a), id_counts(b));
    // Pass 1: by workshop id (with the pack name too, for mods that ship several packs).
    let key = |e: &ShareEntry, counts: &HashMap<WorkshopId, usize>| {
        e.workshop_id.map(|id| if counts[&id] > 1 { format!("{id}/{}", pack_key(&e.pack)) } else { id.to_string() })
    };
    let mut a_by_key: HashMap<String, usize> = HashMap::new();
    for (i, e) in a.entries.iter().enumerate() {
        if let Some(k) = key(e, &a_ids) {
            a_by_key.entry(k).or_insert(i);
        }
    }
    let mut used = vec![false; a.entries.len()];
    let mut out: Vec<Option<usize>> = b
        .entries
        .iter()
        .map(|e| {
            let i = key(e, &b_ids).and_then(|k| a_by_key.get(&k).copied()).filter(|&i| !used[i])?;
            used[i] = true;
            Some(i)
        })
        .collect();

    // Pass 2: whatever is left, by pack name.
    let mut a_by_pack: HashMap<String, usize> = HashMap::new();
    for (i, e) in a.entries.iter().enumerate() {
        if !used[i] && !e.pack.is_empty() {
            a_by_pack.entry(pack_key(&e.pack)).or_insert(i);
        }
    }
    for (slot, e) in out.iter_mut().zip(&b.entries) {
        if slot.is_none() && !e.pack.is_empty() {
            if let Some(i) = a_by_pack.remove(&pack_key(&e.pack)) {
                *slot = Some(i);
            }
        }
    }
    out
}

fn entries_where(list: &ShareList, keep: impl Fn(usize) -> bool) -> Vec<DiffEntry> {
    list.entries
        .iter()
        .enumerate()
        .filter(|(i, _)| keep(*i))
        .map(|(i, e)| DiffEntry { pack: e.label(), workshop_id: e.workshop_id, position: i })
        .collect()
}

fn best_label(a: &ShareEntry, b: &ShareEntry) -> String {
    if b.pack.is_empty() { a.label() } else { b.label() }
}

fn id_counts(list: &ShareList) -> HashMap<WorkshopId, usize> {
    let mut counts = HashMap::new();
    for id in list.entries.iter().filter_map(|e| e.workshop_id) {
        *counts.entry(id).or_insert(0) += 1;
    }
    counts
}

/// Indices (into `seq`) of one longest strictly increasing subsequence. O(n log n).
fn longest_increasing_subsequence(seq: &[usize]) -> HashSet<usize> {
    let mut tails: Vec<usize> = Vec::new(); // index into seq of the smallest tail per length
    let mut prev: Vec<Option<usize>> = vec![None; seq.len()];
    for i in 0..seq.len() {
        let len = tails.partition_point(|&t| seq[t] < seq[i]);
        prev[i] = len.checked_sub(1).map(|l| tails[l]);
        if len == tails.len() {
            tails.push(i);
        } else {
            tails[len] = i;
        }
    }
    let mut out = HashSet::new();
    let mut cur = tails.last().copied();
    while let Some(i) = cur {
        out.insert(i);
        cur = prev[i];
    }
    out
}

fn bad(msg: &str) -> Error {
    Error::ShareCode(msg.to_owned())
}

fn truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, Error> {
        let b = *self.buf.get(self.pos).ok_or_else(|| bad("the code is damaged (truncated)"))?;
        self.pos += 1;
        Ok(b)
    }

    fn varint(&mut self) -> Result<u64, Error> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(bad("the code is damaged (bad number)"))
    }

    fn string(&mut self) -> Result<String, Error> {
        let len = self.varint()? as usize;
        let end = self.pos.checked_add(len).filter(|&e| e <= self.buf.len()).ok_or_else(|| bad("the code is damaged (truncated)"))?;
        let s = std::str::from_utf8(&self.buf[self.pos..end]).map_err(|_| bad("the code is damaged (bad text)"))?;
        self.pos = end;
        Ok(s.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn list(packs: &[&str]) -> ShareList {
        ShareList {
            name: "test".into(),
            entries: packs
                .iter()
                .enumerate()
                .map(|(i, p)| ShareEntry::new(*p, Some(WorkshopId(2_800_000_000 + i as u64 * 7_919_111)), 1_690_000_000 + i as i64 * 86_413))
                .collect(),
        }
    }

    /// A list the way a receiver sees it after decoding: ids and versions, no names or timestamps.
    fn as_received(l: &ShareList) -> ShareList {
        ShareList {
            name: l.name.clone(),
            entries: l.entries.iter().map(|e| ShareEntry { pack: String::new(), time_updated: 0, version: e.version_tag(), ..e.clone() }).collect(),
        }
    }

    #[test]
    fn code_round_trips_inside_a_discord_message() {
        let mut l = list(&["!a.pack", "b.pack", "Aekold Reskin.pack"]);
        l.entries.push(ShareEntry::new("no_id.pack", None, 0));
        let code = encode(&l);
        let pasted = format!("here's my list:\n```\n{}\n{}\n```", &code[..20], &code[20..]);
        let got = decode(&pasted).unwrap();
        let mut expected = as_received(&l);
        expected.entries[3].pack = "no_id.pack".into(); // no id, so the name travels
        assert_eq!(got, expected);
    }

    #[test]
    fn multi_pack_mods_keep_their_names() {
        let mut l = list(&["a.pack", "b.pack"]);
        l.entries[1].workshop_id = l.entries[0].workshop_id;
        let got = decode(&encode(&l)).unwrap();
        assert_eq!(got.entries[0].pack, "a.pack");
        assert_eq!(got.entries[1].pack, "b.pack");
    }

    #[test]
    fn damaged_codes_fail_cleanly() {
        let code = encode(&list(&["a.pack"]));
        assert!(decode("hello").is_err());
        assert!(decode(&code[..code.len() - 4]).is_err());
        assert!(decode("WARP1:!!!!").is_err());
    }

    #[test]
    fn warp_file_round_trips() {
        let l = list(&["a.pack", "b.pack"]);
        assert_eq!(from_warp_file(&to_warp_file(&l)).unwrap(), l);
    }

    #[test]
    fn a_decoded_code_matches_the_list_it_came_from() {
        let l = list(&["a.pack", "b.pack", "c.pack"]);
        let d = diff(&l, &decode(&encode(&l)).unwrap());
        assert!(d.identical, "{d:?}");
    }

    #[test]
    fn finds_missing_extra_moves_and_versions() {
        let a = list(&["a.pack", "b.pack", "c.pack", "x.pack"]);
        let mut b = list(&["b.pack", "a.pack", "c.pack", "y.pack"]);
        // Same mods on both sides: align ids by name.
        for e in &mut b.entries {
            if let Some(ae) = a.entries.iter().find(|ae| ae.pack == e.pack) {
                e.workshop_id = ae.workshop_id;
                e.time_updated = ae.time_updated;
            } else {
                e.workshop_id = Some(WorkshopId(99));
            }
        }
        b.entries[2].time_updated += 5_000; // c.pack is newer in B
        let d = diff(&a, &b);
        assert!(!d.identical);
        assert_eq!(d.common, 3);
        assert_eq!(d.only_in_a.iter().map(|e| e.pack.as_str()).collect::<Vec<_>>(), ["x.pack"]);
        assert_eq!(d.only_in_b.iter().map(|e| e.pack.as_str()).collect::<Vec<_>>(), ["y.pack"]);
        assert_eq!(d.moves.len(), 1, "one move fixes a swap: {:?}", d.moves);
        assert_eq!(d.version_mismatches.len(), 1);
        assert_eq!(d.version_mismatches[0].stale, Some(Side::A));
    }

    #[test]
    fn stale_side_is_settled_against_steam() {
        let a = list(&["a.pack"]);
        let mut b = as_received(&a);
        b.entries[0].version = version_tag(1); // B shared an old build
        let mut d = diff(&a, &b);
        assert_eq!(d.version_mismatches[0].stale, None);
        let current = HashMap::from([(a.entries[0].workshop_id.unwrap(), a.entries[0].time_updated)]);
        resolve_stale(&mut d, &a, &b, &current);
        assert_eq!(d.version_mismatches[0].stale, Some(Side::B));
    }

    #[test]
    fn packs_without_ids_match_by_name() {
        let a = list(&["a.pack", "b.pack"]);
        let mut b = a.clone();
        b.entries[1].workshop_id = None; // e.g. a Kaedrin pack the library doesn't know
        assert_eq!(diff(&a, &b).common, 2);
    }

    #[test]
    fn a_150_mod_list_fits_in_a_discord_message() {
        let names: Vec<String> = (0..150).map(|i| format!("!some_typical_mod_name_{i:03}_sfo.pack")).collect();
        let code = encode(&list(&names.iter().map(String::as_str).collect::<Vec<_>>()));
        assert!(code.len() <= 2000, "code is {} chars", code.len());
    }

    /// Applies moves (top to bottom) to A's shared packs.
    fn apply(order: &[String], moves: &[Move]) -> Vec<String> {
        let mut v = order.to_vec();
        for m in moves {
            v.retain(|p| p != &m.pack);
            let at = match &m.below {
                None => 0,
                Some(below) => v.iter().position(|p| p == below).expect("anchor present") + 1,
            };
            v.insert(at, m.pack.clone());
        }
        v
    }

    proptest! {
        #[test]
        fn moves_are_minimal_and_correct(perm in Just((0..30).collect::<Vec<usize>>()).prop_shuffle()) {
            let names: Vec<String> = (0..30).map(|i| format!("p{i}.pack")).collect();
            let a = list(&names.iter().map(String::as_str).collect::<Vec<_>>());
            let b = ShareList { name: "b".into(), entries: perm.iter().map(|&i| a.entries[i].clone()).collect() };
            let d = diff(&a, &b);

            let result = apply(&names, &d.moves);
            prop_assert_eq!(result, perm.iter().map(|&i| names[i].clone()).collect::<Vec<_>>());

            // Minimal: moves = n - LIS, computed independently by O(n^2) DP.
            let mut best = vec![1usize; perm.len()];
            for i in 0..perm.len() {
                for j in 0..i {
                    if perm[j] < perm[i] { best[i] = best[i].max(best[j] + 1); }
                }
            }
            let lis = best.into_iter().max().unwrap_or(0);
            prop_assert_eq!(d.moves.len(), 30 - lis);
        }
    }
}
