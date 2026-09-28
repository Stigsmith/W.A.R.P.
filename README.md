# W.A.R.P.

**Warhammer Advanced Resource Platform**: a mod manager for **Total War: WARHAMMER III** that works out the load order for
you, and gets you and your friends into multiplayer with matching modlists.

> Status: early development (M1). Nothing to download yet.

## What it does

- **Orders mods by what they are.** Specific mods go on top and foundations at the
  bottom: UI and submods win, frameworks and assets sit underneath. Hard rules
  (requires / patches / your pins) apply on top of that, and every placement
  tells you *why* it's there. See [docs/load-order.md](docs/load-order.md).
- **Multiplayer sync.** Share your modlist as a code that fits in one Discord
  message. WARP compares two lists and tells you exactly what's missing, what's
  extra, the fewest moves to fix the order, and whose copy of a mod is outdated.
- **Sets and profiles.** Build reusable sets (Base, SFO, one per faction campaign...)
  by ticking mods into set columns in the Library, a whole column at a time if you
  like. A profile stacks sets plus single mods. It keeps its own copy of each set,
  so changing a set later never changes a profile behind your back: the profile
  asks first.
- **Community knowledge.** Mod categories and relations live in a shared
  knowledge base ([knowledge/](knowledge/)), so nobody has to tag 400 mods alone.

## Layout

| Path | What |
|---|---|
| `crates/warp-core` | All logic: solver, multiplayer, knowledge, storage, Steam |
| `crates/warp-cli` | `warp` command-line tool |
| `app/` | Desktop app (Tauri + Svelte) |
| `knowledge/` | Taxonomy and the community knowledge base |
| `docs/` | How things work |

## Development

Needs Rust 1.90+ and Node 20+. On Windows, also the Visual Studio C++ Build Tools.

```sh
cargo test --workspace                # core tests
cargo run -p warp-cli -- --help       # the CLI
npm --prefix app install
npm --prefix app run tauri dev        # the desktop app
```

Checks run locally before every commit (format, lints, tests, UI types). Turn them on once per clone:

```sh
git config core.hooksPath .githooks
```

See [docs/architecture.md](docs/architecture.md) for how the pieces fit.

## License

MIT
