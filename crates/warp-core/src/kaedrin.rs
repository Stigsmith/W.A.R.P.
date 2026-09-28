//! Kaedrin Mod Manager interop: its name ordering and its profile files.
//!
//! A Kaedrin profile (`%APPDATA%\Kaedrin Mod Manager\Profiles\Warhammer3\profile_*.txt`)
//! is one pack file name per line, top of the load order first.

use std::cmp::Ordering;
use std::path::PathBuf;

use crate::model::is_pack_name;

/// Kaedrin's Warhammer III profile folder (it may not exist).
pub fn profiles_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("Kaedrin Mod Manager").join("Profiles").join("Warhammer3"))
}

/// Kaedrin's file name for a profile.
pub fn profile_file_name(profile: &str) -> String {
    let name: String = profile.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect();
    format!("profile_{}.txt", name.trim())
}

/// Character order used to sort pack names, from the Total War modding wiki.
/// Space is placed first (as Windows does); characters not listed sort after
/// all listed ones, by code point.
const ORDER: &str = " !#$%&'()+,-;=@0123456789abcdefghijklmnopqrstuvwxyz[]^_`{}~";

fn rank(c: char) -> u32 {
    let c = c.to_ascii_lowercase();
    match ORDER.find(c) {
        Some(i) => i as u32,
        None => ORDER.len() as u32 + c as u32,
    }
}

fn stem(name: &str) -> &str {
    if is_pack_name(name) { &name[..name.len() - 5] } else { name }
}

/// A sortable key for a pack name. Earlier in Kaedrin order means a smaller key.
/// The `.pack` extension is ignored, and a name that is a prefix of another sorts first.
pub fn name_key(name: &str) -> Vec<u32> {
    stem(name.trim()).chars().map(rank).collect()
}

/// Compares pack names in Kaedrin order. `Less` means `a` comes first (higher in the list).
pub fn compare_names(a: &str, b: &str) -> Ordering {
    name_key(a).cmp(&name_key(b))
}

/// Reads a Kaedrin profile: pack names in load order. Blank lines and anything
/// that isn't a `.pack` name are skipped.
pub fn read_profile(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .filter(|l| is_pack_name(l))
        .map(str::to_owned)
        .collect()
}

/// Writes a Kaedrin profile. Kaedrin itself writes CRLF line endings.
pub fn write_profile<S: AsRef<str>>(packs: &[S]) -> String {
    let mut out = String::new();
    for p in packs {
        out.push_str(p.as_ref());
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_before_digits_before_letters_before_brackets() {
        let mut names = vec!["_z.pack", "b.pack", "1.pack", "@a.pack", "!a.pack", "~x.pack"];
        names.sort_by(|a, b| compare_names(a, b));
        assert_eq!(names, ["!a.pack", "@a.pack", "1.pack", "b.pack", "_z.pack", "~x.pack"]);
    }

    #[test]
    fn more_bangs_sort_first_and_case_is_ignored() {
        assert_eq!(compare_names("!!!x.pack", "!x.pack"), Ordering::Less);
        assert_eq!(compare_names("ABC.pack", "abc.pack"), Ordering::Equal);
    }

    #[test]
    fn hyphen_and_space_are_ranked() {
        // v1 used an en dash in its table, so '-' fell through to "unknown".
        assert_eq!(compare_names("a-b.pack", "a0.pack"), Ordering::Less);
        assert_eq!(compare_names("Azazel - Reskin.pack", "Azazel_Reskin.pack"), Ordering::Less);
    }

    #[test]
    fn prefix_sorts_first_and_extension_is_ignored() {
        assert_eq!(compare_names("marthreload.pack", "marthreload_fix.pack"), Ordering::Less);
    }

    #[test]
    fn profile_round_trip() {
        let text = "\u{feff}a.pack\r\n\r\nnot a pack\r\n!b.pack\r\n";
        let packs = read_profile(text);
        assert_eq!(packs, ["a.pack", "!b.pack"]);
        assert_eq!(write_profile(&packs), "a.pack\r\n!b.pack\r\n");
    }
}
