# W.A.R.P.

A mod manager for **Total War: WARHAMMER III** that works out the load order for
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
- **Sets and profiles.** Build reusable sets (Base, SFO, Chaos...) and stack them
  into profiles.
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

Needs Rust (stable) and Node 20+. On Windows, also the Visual Studio C++ Build Tools.

```sh
cargo test --workspace
cargo run -p warp-cli -- --help
```

## License

MIT
