//! The plain-text report a tester copies and sends back: what W.A.R.P. sees on
//! their PC, in one paste. It works without a library too, because a database
//! that won't open is exactly when a report is needed.

use std::fmt::Write;

use crate::conflicts::Severity;
use crate::install::Install;
use crate::knowledge::Source;
use crate::library::Library;
use crate::log;

/// What went into a report besides the library.
pub struct Context<'a> {
    pub version: &'a str,
    pub install: &'a Result<Install, String>,
    /// Why the library couldn't be opened, if it couldn't.
    pub library_error: Option<&'a str>,
    /// Anything the app adds, such as the playtest checklist.
    pub extra: &'a str,
}

pub fn report(lib: Option<&Library>, cx: &Context<'_>) -> String {
    let mut out = String::new();
    // Writing to a String can't fail.
    let _ = write_report(&mut out, lib, cx);
    out
}

fn write_report(out: &mut String, lib: Option<&Library>, cx: &Context<'_>) -> std::fmt::Result {
    writeln!(
        out,
        "W.A.R.P. {} report, {}",
        cx.version,
        log::timestamp(log::now())
    )?;
    writeln!(out, "System: {}", system())?;
    match cx.install {
        Ok(i) => writeln!(out, "Game: {}", i.game_dir.display())?,
        Err(e) => writeln!(out, "Game: not found ({e})")?,
    }
    writeln!(out, "Data: {}", crate::data_dir().display())?;
    if let Some(e) = cx.library_error {
        writeln!(out, "Database: couldn't open ({e})")?;
    }
    if let Some(lib) = lib
        && let Err(e) = write_library(out, lib)
    {
        writeln!(out, "(library details failed: {e})")?;
    }
    if !cx.extra.trim().is_empty() {
        writeln!(out, "\n{}", cx.extra.trim_end())?;
    }
    writeln!(out, "\nRecent log:")?;
    let lines = log::tail(30);
    if lines.is_empty() {
        writeln!(out, "  (empty)")?;
    }
    for l in lines {
        writeln!(out, "  {l}")?;
    }
    Ok(())
}

/// The library part; a failure here still leaves the rest of the report.
fn write_library(out: &mut String, lib: &Library) -> Result<(), crate::Error> {
    let entries = lib.entries()?;
    let installed: Vec<_> = entries.iter().filter(|e| e.subscribed).collect();
    let source_count = |s: Source| {
        installed
            .iter()
            .filter(|e| e.knowledge.tier_source == s)
            .count()
    };
    let all_sets = lib.store.sets()?;
    let updates = lib.set_updates()?;
    let either_or = lib.either_or_pairs()?;

    let _ = writeln!(
        out,
        "Library: {} mods known, {} installed, {} packs indexed, {} sets",
        entries.len(),
        installed.len(),
        lib.store.pack_summaries()?.len(),
        all_sets.len()
    );
    let _ = writeln!(
        out,
        "Tiers of installed mods: {} community, {} guessed, {} set by you, {} default",
        source_count(Source::Community),
        source_count(Source::Heuristic),
        source_count(Source::User),
        source_count(Source::Default)
    );
    let _ = writeln!(out, "Either/or pairs installed: {}", either_or.len());

    let profiles = lib.store.profiles()?;
    let _ = writeln!(out, "Profiles: {}", profiles.len());
    for p in &profiles {
        let r = lib.resolve_profile(p)?;
        let c = lib.conflicts(p)?;
        let problems = |sev: Severity| {
            c.pairs
                .iter()
                .filter(|x| !x.intended && x.severity == sev)
                .count()
        };
        let _ = writeln!(
            out,
            "  {}: {} packs from {} set(s) + {} picked - {} left out; conflicts {} high / {} medium / {} low ({} intended); either/or {}; missing requirements {}; unsubscribed {}; set changes waiting {} (on change: {:?})",
            p.name,
            r.order.placements.len(),
            p.sets.len(),
            p.include.len(),
            p.exclude.len(),
            problems(Severity::High),
            problems(Severity::Medium),
            problems(Severity::Low),
            c.pairs.iter().filter(|x| x.intended).count(),
            r.either_or.len(),
            r.missing_requirements.len(),
            r.unsubscribed.len(),
            updates.get(&p.name).map_or(0, Vec::len),
            p.set_changes,
        );
    }
    match lib.store.last_launch()? {
        Some((at, profile, packs)) => {
            let _ = writeln!(
                out,
                "Last Play: {} with \"{profile}\" ({packs} packs)",
                log::timestamp(at)
            );
        }
        None => {
            let _ = writeln!(out, "Last Play: never");
        }
    }
    Ok(())
}

/// OS name, architecture and, on Windows, the build number.
fn system() -> String {
    let base = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up from the GUI app.
        let ver = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .creation_flags(0x0800_0000)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|v| !v.is_empty());
        if let Some(ver) = ver {
            return format!("{base}, {ver}");
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_without_a_library_still_says_what_it_can() {
        let install = Err("Steam isn't installed".to_owned());
        let text = report(
            None,
            &Context {
                version: "9.9.9",
                install: &install,
                library_error: Some("disk on fire"),
                extra: "Playtest checklist: [x] Read mods",
            },
        );
        assert!(text.starts_with("W.A.R.P. 9.9.9 report, "));
        assert!(text.contains("Game: not found (Steam isn't installed)"));
        assert!(text.contains("Database: couldn't open (disk on fire)"));
        assert!(text.contains("[x] Read mods"));
        assert!(text.contains("Recent log:"));
    }
}
