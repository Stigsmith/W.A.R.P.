//! `warp`: W.A.R.P. from the command line.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use warp_core::knowledge::KnowledgeBase;
use warp_core::library::Library;
use warp_core::mp::{self, ShareList};
use warp_core::order::Reason;
use warp_core::store::Store;
use warp_core::{import_v1, kaedrin};

#[derive(Parser)]
#[command(name = "warp", version, about = "W.A.R.P. - mod manager for Total War: WARHAMMER III")]
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db = cli.db.clone().unwrap_or_else(warp_core::default_db_path);
    let kb = match &cli.kb {
        Some(path) => KnowledgeBase::parse(&std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?)?,
        None => warp_core::builtin_kb(),
    };

    if let Command::ImportV1 { workbook, write_kb } = &cli.command {
        return import(&db, kb, workbook, write_kb.as_deref());
    }
    let mut lib = Library::new(Store::open(&db).with_context(|| format!("opening {}", db.display()))?, kb);

    match cli.command {
        Command::ImportV1 { .. } => unreachable!("handled above"),
        Command::Mods { filter } => {
            let filter = filter.map(|f| f.to_lowercase());
            for e in lib.entries()? {
                let hay = format!("{} {}", e.info.title, e.packs.join(" ")).to_lowercase();
                if filter.as_ref().is_none_or(|f| hay.contains(f)) {
                    let flag = if e.subscribed { ' ' } else { 'x' };
                    println!("{flag} {:>10}  {:<13} {:<10} {}", e.info.id, e.knowledge.tier, e.knowledge.role, e.info.title);
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
                None => kaedrin::profiles_dir().context("no Kaedrin folder")?.join(kaedrin::profile_file_name(&def.name)),
            };
            std::fs::write(&out, kaedrin::write_profile(&resolved.order.packs()))?;
            println!("Wrote {} packs to {}", resolved.order.placements.len(), out.display());
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
    }
    Ok(())
}

fn import(db: &Path, mut kb: KnowledgeBase, workbook: &Path, write_kb: Option<&Path>) -> Result<()> {
    let data = import_v1::read(workbook).with_context(|| format!("reading {}", workbook.display()))?;
    if let Some(path) = write_kb {
        let mut file_kb = match std::fs::read_to_string(path) {
            Ok(text) => KnowledgeBase::parse(&text)?,
            Err(_) => KnowledgeBase::new(),
        };
        for m in &data.mods {
            file_kb.mods.insert(m.info.id, m.knowledge.clone());
        }
        std::fs::write(path, file_kb.to_json())?;
        println!("Knowledge base: {} mods written to {}", file_kb.mods.len(), path.display());
        kb = file_kb;
    }
    let mut lib = Library::new(Store::open(db)?, kb);
    let s = lib.import_v1(&data)?;
    println!("Imported {} mods into {}", s.mods, db.display());
    println!("Sets: {}", s.sets.join(", "));
    if let Some(p) = s.profile {
        println!("Profile: {p}");
    }
    println!("Your own overrides (differ from the knowledge base): {}", s.overrides);
    if !s.unresolved_dependencies.is_empty() {
        println!("Dependencies that don't match a mod in the workbook:");
        for (pack, text) in s.unresolved_dependencies {
            println!("  {pack} -> {text}");
        }
    }
    Ok(())
}

fn profile_def(lib: &Library, name: &str) -> Result<warp_core::store::ProfileDef> {
    lib.store.profile(name)?.with_context(|| format!("no profile named '{name}'"))
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
                    Reason::Raised { other, rule } => println!("        RAISED above {other} ({rule:?})"),
                    Reason::InCycle => println!("        in a rule cycle - placed by default order"),
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
    for c in &r.order.cycles {
        println!("! contradictory rules: {}", c.iter().map(|r| format!("{} > {}", r.above, r.below)).collect::<Vec<_>>().join(", "));
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
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("warp")) {
            return Ok(mp::from_warp_file(&text)?);
        }
        let name = path.file_stem().map(|s| s.to_string_lossy().trim_start_matches("profile_").to_owned()).unwrap_or_default();
        return Ok(lib.share_list_from_packs(&name, &kaedrin::read_profile(&text))?);
    }
    match lib.store.profile(source)? {
        Some(def) => Ok(lib.share_list(&def)?),
        None => bail!("'{source}' is not a share code, a file, or a profile name"),
    }
}

fn print_diff(a: &ShareList, b: &ShareList, d: &mp::ListDiff) {
    println!("A: {} ({} packs)   B: {} ({} packs)   shared: {}", a.name, a.entries.len(), b.name, b.entries.len(), d.common);
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
