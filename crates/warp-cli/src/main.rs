//! `warp`: W.A.R.P. from the command line.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use warp_core::install::Install;
use warp_core::knowledge::KnowledgeBase;
use warp_core::library::Library;
use warp_core::mp::{self, ShareList};
use warp_core::order::Reason;
use warp_core::store::Store;
use warp_core::{import_v1, kaedrin};

#[derive(Parser)]
#[command(
    name = "warp",
    version,
    about = "W.A.R.P. - mod manager for Total War: WARHAMMER III"
)]
struct Cli {
    /// Database file (default: %APPDATA%\WARP\warp.db, or $WARP_HOME/warp.db).
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    /// Community knowledge base to use instead of the built-in one.
    #[arg(long, global = true)]
    kb: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a W.A.R.P. v1 workbook (.xlsm, or a .zip holding one).
    ImportV1 {
        workbook: PathBuf,
        /// Also merge the workbook's tags into this knowledge base file (maintainers only).
        #[arg(long)]
        write_kb: Option<PathBuf>,
    },
    /// List mods in the library.
    Mods {
        /// Only mods whose title or pack contains this text.
        filter: Option<String>,
    },
    /// List sets and profiles.
    Profiles,
    /// Show a profile's load order.
    Build {
        profile: String,
        /// Show why each pack is where it is.
        #[arg(long)]
        explain: bool,
    },
    /// Write a profile as a Kaedrin Mod Manager profile.
    ExportKaedrin {
        profile: String,
        /// Output file (default: Kaedrin's profile folder).
        out: Option<PathBuf>,
    },
    /// Print a profile's share code, or write it as a .warp file.
    Share {
        profile: String,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Compare two modlists. Each side can be a profile name, a Kaedrin .txt,
    /// a .warp file or a share code.
    Diff {
        a: String,
        b: String,
        /// Don't ask Steam which side of a version mismatch is outdated.
        #[arg(long)]
        offline: bool,
    },
    /// Refresh Steam metadata for every mod in the library.
    SteamRefresh,
    /// Read the installed game: subscriptions, installed versions, and what's inside every pack.
    Sync {
        /// Don't ask Steam for metadata of new or updated mods.
        #[arg(long)]
        offline: bool,
    },
    /// Show which packs in a profile override each other's files.
    Conflicts {
        profile: String,
        /// Also list the overridden files.
        #[arg(long)]
        files: bool,
    },
    /// Installed mods that look like two versions of the same mod.
    EitherOr,
    /// Start the game with a profile.
    Play {
        profile: String,
        /// Write the modlist but don't start the game.
        #[arg(long)]
        dry_run: bool,
    },
    /// Write sample data for running the app UI in a browser (development only).
    #[command(hide = true)]
    DevFixture { out: PathBuf },
    /// How often WARP's guessed tier matches the community's (development only).
    #[command(hide = true)]
    ClassifyReport {
        /// Also list every mod where they differ.
        #[arg(long)]
        misses: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db = cli.db.clone().unwrap_or_else(warp_core::default_db_path);
    let kb = match &cli.kb {
        Some(path) => KnowledgeBase::parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?,
        )?,
        None => warp_core::builtin_kb(),
    };

    if let Command::ImportV1 { workbook, write_kb } = &cli.command {
        return import(&db, kb, workbook, write_kb.as_deref());
    }
    let mut lib = Library::new(
        Store::open(&db).with_context(|| format!("opening {}", db.display()))?,
        kb,
    );

    match cli.command {
        Command::ImportV1 { .. } => unreachable!("handled above"),
        Command::Mods { filter } => {
            let filter = filter.map(|f| f.to_lowercase());
            for e in lib.entries()? {
                let hay = format!("{} {}", e.info.title, e.packs.join(" ")).to_lowercase();
                if filter.as_ref().is_none_or(|f| hay.contains(f)) {
                    let flag = if e.subscribed { ' ' } else { 'x' };
                    println!(
                        "{flag} {:>10}  {:<13} {:<10} {}",
                        e.info.id, e.knowledge.tier, e.knowledge.role, e.info.title
                    );
                }
            }
        }
        Command::Profiles => {
            println!("Sets:");
            for s in lib.store.sets()? {
                println!("  {:<20} {} mods", s.name, s.members.len());
            }
            println!("Profiles:");
            for p in lib.store.profiles()? {
                println!("  {:<20} {}", p.name, p.sets.join(" + "));
            }
        }
        Command::Build { profile, explain } => build(&lib, &profile, explain)?,
        Command::ExportKaedrin { profile, out } => {
            let def = profile_def(&lib, &profile)?;
            let resolved = lib.resolve_profile(&def)?;
            let out = match out {
                Some(p) => p,
                None => kaedrin::profiles_dir()
                    .context("no Kaedrin folder")?
                    .join(kaedrin::profile_file_name(&def.name)),
            };
            std::fs::write(&out, kaedrin::write_profile(&resolved.order.packs()))?;
            println!(
                "Wrote {} packs to {}",
                resolved.order.placements.len(),
                out.display()
            );
        }
        Command::Share { profile, file } => {
            let list = lib.share_list(&profile_def(&lib, &profile)?)?;
            match file {
                Some(path) => {
                    std::fs::write(&path, mp::to_warp_file(&list))?;
                    println!("Wrote {}", path.display());
                }
                None => {
                    let code = mp::encode(&list);
                    println!("{code}");
                    eprintln!("({} packs, {} characters)", list.entries.len(), code.len());
                }
            }
        }
        Command::Diff { a, b, offline } => {
            let (mut a, mut b) = (load_list(&lib, &a)?, load_list(&lib, &b)?);
            let d = lib.compare(&mut a, &mut b, !offline)?;
            print_diff(&a, &b, &d);
        }
        Command::SteamRefresh => {
            let n = lib.refresh_from_steam(None)?;
            println!("Refreshed {n} mods from Steam");
        }
        Command::DevFixture { out } => dev_fixture(&lib, &out)?,
        Command::ClassifyReport { misses } => classify_report(&lib, misses)?,
        Command::Sync { offline } => {
            let install = Install::locate()?;
            let started = std::time::Instant::now();
            let s = lib.sync_install(&install, !offline)?;
            println!("Game: {}", install.game_dir.display());
            println!(
                "{} mods installed; {} new, {} unsubscribed, {} resubscribed",
                s.installed,
                s.new_mods.len(),
                s.unsubscribed.len(),
                s.resubscribed.len()
            );
            println!(
                "Packs: {} indexed, {} unchanged; Steam details refreshed for {} ({:.1}s)",
                s.packs_indexed,
                s.packs_cached,
                s.steam_refreshed,
                started.elapsed().as_secs_f64()
            );
            for (path, err) in &s.pack_errors {
                println!("! couldn't read {path}: {err}");
            }
        }
        Command::EitherOr => {
            let titles: std::collections::HashMap<_, _> = lib
                .entries()?
                .into_iter()
                .map(|e| (e.info.id, e.info.title))
                .collect();
            let title = |id| titles.get(&id).cloned().unwrap_or_else(|| format!("{id}"));
            for o in lib.either_or_pairs()? {
                println!("{}  <->  {}", title(o.a), title(o.b));
                println!(
                    "    {} of {}/{} DB files identical, e.g. {}",
                    o.shared, o.a_files, o.b_files, o.example
                );
            }
        }
        Command::Conflicts { profile, files } => {
            let r = lib.conflicts(&profile_def(&lib, &profile)?)?;
            print_conflicts(&r, files);
        }
        Command::Play { profile, dry_run } => {
            let install = Install::locate()?;
            let def = profile_def(&lib, &profile)?;
            if dry_run {
                let entries = lib.mod_list_entries(&def, &install)?;
                let missing = warp_core::launch::uninstalled(&install, &entries);
                if !missing.is_empty() {
                    println!(
                        "! {} pack(s) aren't installed; the game would refuse to start:",
                        missing.len()
                    );
                    for pack in missing {
                        println!("  {pack}");
                    }
                }
                let path = warp_core::launch::write_mod_list(&install, &entries)?;
                println!(
                    "Wrote {} ({} packs). Game not started.",
                    path.display(),
                    entries.len()
                );
                println!(
                    "Start it yourself with: \"{}\" {}",
                    install.game_exe().display(),
                    warp_core::launch::game_args(None).join(" ")
                );
            } else {
                let path = lib.play(&def, &install, None)?;
                println!("Started the game with {}", path.display());
            }
        }
    }
    Ok(())
}

fn import(
    db: &Path,
    mut kb: KnowledgeBase,
    workbook: &Path,
    write_kb: Option<&Path>,
) -> Result<()> {
    let data =
        import_v1::read(workbook).with_context(|| format!("reading {}", workbook.display()))?;
    if let Some(path) = write_kb {
        let mut file_kb = match std::fs::read_to_string(path) {
            Ok(text) => KnowledgeBase::parse(&text)?,
            Err(_) => KnowledgeBase::new(),
        };
        for m in &data.mods {
            file_kb.mods.insert(m.info.id, m.knowledge.clone());
        }
        std::fs::write(path, file_kb.to_json())?;
        println!(
            "Knowledge base: {} mods written to {}",
            file_kb.mods.len(),
            path.display()
        );
        kb = file_kb;
    }
    let mut lib = Library::new(Store::open(db)?, kb);
    let s = lib.import_v1(&data)?;
    println!("Imported {} mods into {}", s.mods, db.display());
    println!("Sets: {}", s.sets.join(", "));
    if let Some(p) = s.profile {
        println!("Profile: {p}");
    }
    println!(
        "Your own overrides (differ from the knowledge base): {}",
        s.overrides
    );
    if !s.unresolved_dependencies.is_empty() {
        println!("Dependencies that don't match a mod in the workbook:");
        for (pack, text) in s.unresolved_dependencies {
            println!("  {pack} -> {text}");
        }
    }
    Ok(())
}

fn profile_def(lib: &Library, name: &str) -> Result<warp_core::store::ProfileDef> {
    lib.store
        .profile(name)?
        .with_context(|| format!("no profile named '{name}'"))
}

fn build(lib: &Library, profile: &str, explain: bool) -> Result<()> {
    let r = lib.resolve_profile(&profile_def(lib, profile)?)?;
    for (i, p) in r.order.placements.iter().enumerate() {
        println!("{:>4}  {:<60} {} / {}", i + 1, p.pack, p.tier, p.role);
        if explain {
            for reason in &p.reasons {
                match reason {
                    Reason::Default { .. } => {}
                    Reason::Above { other, rule } => println!("        above {other} ({rule:?})"),
                    Reason::Below { other, rule } => println!("        below {other} ({rule:?})"),
                    Reason::Raised { other, rule } => {
                        println!("        RAISED above {other} ({rule:?})")
                    }
                    Reason::InCycle => {
                        println!("        in a rule cycle - placed by default order")
                    }
                }
            }
        }
    }
    let warn = |label: &str, n: usize| {
        if n > 0 {
            println!("! {label}: {n}");
        }
    };
    warn("mods not in library", r.unknown_mods.len());
    warn("mods without a known pack", r.mods_without_packs.len());
    warn("unsubscribed mods", r.unsubscribed.len());
    for (m, req) in &r.missing_requirements {
        println!("! {m} requires {req}, which isn't in the profile");
    }
    for (a, b) in &r.incompatibilities {
        println!("! {a} and {b} are incompatible");
    }
    for o in &r.either_or {
        println!(
            "! {} and {} look like two versions of the same mod ({} identical DB files, e.g. {})",
            o.a, o.b, o.shared, o.example
        );
    }
    for c in &r.order.cycles {
        println!(
            "! contradictory rules: {}",
            c.iter()
                .map(|r| format!("{} > {}", r.above, r.below))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

fn load_list(lib: &Library, source: &str) -> Result<ShareList> {
    if source.contains(mp::CODE_PREFIX) {
        return Ok(mp::decode(source)?);
    }
    let path = Path::new(source);
    if path.is_file() {
        let text = std::fs::read_to_string(path)?;
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("warp"))
        {
            return Ok(mp::from_warp_file(&text)?);
        }
        let name = path
            .file_stem()
            .map(|s| {
                s.to_string_lossy()
                    .trim_start_matches("profile_")
                    .to_owned()
            })
            .unwrap_or_default();
        return Ok(lib.share_list_from_packs(&name, &kaedrin::read_profile(&text))?);
    }
    match lib.store.profile(source)? {
        Some(def) => Ok(lib.share_list(&def)?),
        None => bail!("'{source}' is not a share code, a file, or a profile name"),
    }
}

fn print_diff(a: &ShareList, b: &ShareList, d: &mp::ListDiff) {
    println!(
        "A: {} ({} packs)   B: {} ({} packs)   shared: {}",
        a.name,
        a.entries.len(),
        b.name,
        b.entries.len(),
        d.common
    );
    if d.identical {
        println!("Identical. Good to play.");
        return;
    }
    if !d.only_in_b.is_empty() {
        println!("\nOnly in B ({}):", d.only_in_b.len());
        for e in &d.only_in_b {
            println!("  + {}", e.pack);
        }
    }
    if !d.only_in_a.is_empty() {
        println!("\nOnly in A ({}):", d.only_in_a.len());
        for e in &d.only_in_a {
            println!("  - {}", e.pack);
        }
    }
    if !d.moves.is_empty() {
        println!("\nTo match B's order, move in A ({} moves):", d.moves.len());
        for m in &d.moves {
            match &m.below {
                Some(below) => println!("  {}  ->  directly below {below}", m.pack),
                None => println!("  {}  ->  to the top", m.pack),
            }
        }
    }
    if !d.version_mismatches.is_empty() {
        println!("\nDifferent versions ({}):", d.version_mismatches.len());
        for v in &d.version_mismatches {
            let who = match v.stale {
                Some(mp::Side::A) => "A has an outdated copy",
                Some(mp::Side::B) => "B has an outdated copy",
                Some(mp::Side::Both) => "both copies differ from Steam's current version",
                None => "can't tell who is outdated",
            };
            println!("  {}  - {who}", v.pack);
        }
    }
}

/// Compares guessed tiers with community tiers: agreement and a confusion matrix.
fn classify_report(lib: &Library, misses: bool) -> Result<()> {
    use std::collections::BTreeMap;
    let fallback = lib.taxonomy.fallback_tier().key.clone();
    let mut matrix: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut missed = Vec::new();
    let entries = lib.entries()?;
    for e in &entries {
        let Some(truth) = e.community.as_ref().and_then(|c| c.tier.clone()) else {
            continue;
        };
        let guess = e.guessed.tier.clone().unwrap_or_else(|| fallback.clone());
        if guess != truth {
            missed.push((truth.clone(), guess.clone(), e.info.title.clone()));
        }
        *matrix.entry((truth, guess)).or_insert(0) += 1;
    }
    let total: usize = matrix.values().sum();
    let hits: usize = matrix
        .iter()
        .filter(|((t, g), _)| t == g)
        .map(|(_, n)| n)
        .sum();
    println!(
        "Guessed tier matches the community tier for {hits} of {total} mods ({}%)",
        hits * 100 / total.max(1)
    );
    // Rows: community tier. Columns: guess. Top of the load order first.
    let mut tiers: Vec<(i32, &str)> = lib
        .taxonomy
        .tiers
        .iter()
        .map(|t| (-t.priority, t.key.as_str()))
        .collect();
    tiers.sort();
    let tiers: Vec<&str> = tiers.into_iter().map(|(_, k)| k).collect();
    // Columns are numbered like the rows; tier names don't fit in a column.
    print!("\n{:<17}", "kb / guess");
    for i in 1..=tiers.len() {
        print!("{i:>5}");
    }
    println!();
    for (i, row) in tiers.iter().enumerate() {
        print!("{:>2} {row:<14}", i + 1);
        for col in &tiers {
            match matrix.get(&(row.to_string(), col.to_string())) {
                Some(n) => print!("{n:>5}"),
                None => print!("{:>5}", "."),
            }
        }
        println!();
    }

    // Only a few roles are ever guessed, so list just those guesses.
    let mut roles: BTreeMap<(String, String), usize> = BTreeMap::new();
    for e in &entries {
        let truth = e.community.as_ref().and_then(|c| c.role.clone());
        if let (Some(guess), Some(truth)) = (e.guessed.role.clone(), truth) {
            *roles.entry((guess, truth)).or_insert(0) += 1;
        }
    }
    println!("\nGuessed roles (guess -> community):");
    for ((guess, truth), n) in &roles {
        println!("  {guess:>10} -> {truth:<10} {n}");
    }
    if misses {
        println!();
        missed.sort();
        for (truth, guess, title) in missed {
            println!("{truth:>13} -> {guess:<13} {title}");
        }
    }
    Ok(())
}

/// Real data from the library, shaped like the app's command results, plus a
/// staged multiplayer comparison (the first profile against an altered copy).
fn dev_fixture(lib: &Library, out: &Path) -> Result<()> {
    use serde_json::json;
    let mut profiles = lib.store.profiles()?;
    let first = profiles
        .first()
        .context("the library has no profiles")?
        .clone();
    // A staged "everything" profile, like a new user's first one: shows every warning.
    profiles.push(warp_core::store::ProfileDef {
        name: "Everything installed".into(),
        sets: vec![],
        include: lib
            .entries()?
            .into_iter()
            .filter(|e| e.subscribed)
            .map(|e| e.info.id)
            .collect(),
        exclude: vec![],
        pins: vec![],
    });
    let first = &first;
    let mut resolved = serde_json::Map::new();
    let mut conflicts = serde_json::Map::new();
    for p in &profiles {
        resolved.insert(
            p.name.clone(),
            serde_json::to_value(lib.resolve_profile(p)?)?,
        );
        conflicts.insert(p.name.clone(), serde_json::to_value(lib.conflicts(p)?)?);
    }

    let mut mine = lib.share_list(first)?;
    let mut theirs = mine.clone();
    theirs.name = "Friend's list".into();
    theirs.entries.remove(12);
    theirs.entries.remove(40);
    theirs.entries.swap(3, 7);
    theirs.entries.swap(60, 61);
    theirs.entries.insert(
        20,
        mp::ShareEntry::new(
            "friend_only_mod.pack",
            Some(warp_core::model::WorkshopId(3_100_000_001)),
            1_750_000_000,
        ),
    );
    theirs.entries[30].time_updated += 86_400;
    let code = mp::encode(&theirs);
    let mut theirs = mp::decode(&code)?;
    let diff = lib.compare(&mut mine, &mut theirs, false)?;

    let fixture = json!({
        "bootstrap": {
            "taxonomy": lib.taxonomy,
            "mod_count": lib.store.mods()?.len(),
            "data_dir": r"C:\Users\you\AppData\Roaming\WARP",
            "kaedrin_dir": r"C:\Users\you\AppData\Roaming\Kaedrin Mod Manager\Profiles\Warhammer3",
            "install": { "Ok": {
                "game_dir": r"C:\Program Files (x86)\Steam\steamapps\common\Total War WARHAMMER III",
                "workshop_dir": r"C:\Program Files (x86)\Steam\steamapps\workshop\content\1142710",
                "manifest": r"C:\Program Files (x86)\Steam\steamapps\workshop\appworkshop_1142710.acf",
            } },
        },
        "library": lib.entries()?,
        "sets": lib.store.sets()?,
        "profiles": profiles,
        "resolved": resolved,
        "conflicts": conflicts,
        "either_or": lib.either_or_pairs()?,
        "share": { "list": lib.share_list(first)?, "code": mp::encode(&lib.share_list(first)?) },
        "compare": { "code": code, "result": { "diff": diff, "a": mine, "b": theirs } },
    });
    std::fs::write(out, serde_json::to_string(&fixture)?)?;
    println!("Wrote {}", out.display());
    Ok(())
}

fn print_conflicts(r: &warp_core::conflicts::ConflictReport, show_files: bool) {
    use warp_core::conflicts::Severity;
    let problems: Vec<_> = r.pairs.iter().filter(|p| !p.intended).collect();
    let count = |sev: Severity| problems.iter().filter(|p| p.severity == sev).count();
    println!(
        "{} overlaps: {} high, {} medium, {} low risk; {} intended (a patch over its parent)",
        r.pairs.len(),
        count(Severity::High),
        count(Severity::Medium),
        count(Severity::Low),
        r.pairs.len() - problems.len()
    );
    for p in &r.pairs {
        let kinds: Vec<String> = p
            .by_kind
            .iter()
            .map(|(k, n)| format!("{n} {k:?}"))
            .collect();
        let tag = if p.intended {
            "intended".to_owned()
        } else {
            format!("{:?}", p.severity).to_uppercase()
        };
        println!(
            "  [{tag:>8}] {}  over  {}  ({})",
            p.winner,
            p.loser,
            kinds.join(", ")
        );
        if show_files {
            for f in p.files.iter().take(10) {
                println!("               {f}");
            }
        }
    }
    if !r.shadowed.is_empty() {
        println!("Mostly or fully overridden packs:");
        for s in &r.shadowed {
            println!(
                "  {}  {}/{} files overridden by {}",
                s.pack,
                s.overridden,
                s.files,
                s.by.join(", ")
            );
        }
    }
    if !r.not_indexed.is_empty() {
        println!(
            "Not indexed (run `warp sync`): {}",
            r.not_indexed.join(", ")
        );
    }
}
