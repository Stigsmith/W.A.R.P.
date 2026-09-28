# Architecture

```
app/ (Tauri + Svelte)          crates/warp-cli
        │  commands                    │
        └──────────────┬───────────────┘
                       ▼
                crates/warp-core
   library ── order (solver) ── mp (share codes, diff)
      │          │
   store     knowledge ── taxonomy
  (SQLite)   (user > community > guessed)
      │
   steam (Web API) · kaedrin (profiles) · import_v1
```

All behaviour lives in `warp-core`. The app and the CLI are thin shells over it, so
anything the UI can do can be tested from Rust.

## Data

| What | Where | Who owns it |
|---|---|---|
| Tiers and roles | `knowledge/taxonomy.toml` (built in) | WARP |
| Mod categories and relations | `knowledge/mods.json` (built in) | the community |
| Steam metadata cache, packs, subscriptions | `%APPDATA%\WARP\warp.db` | the user's machine |
| The user's overrides, sets, profiles | `%APPDATA%\WARP\warp.db` | the user |

A user's overrides are stored as *differences* from the community knowledge base,
so later knowledge-base updates still reach every field the user hasn't changed.

`WARP_HOME` points WARP at a different data folder, which is useful for testing.

## Key decisions

- **Mods and packs are separate.** A Workshop item can ship several `.pack` files,
  and the load order is per pack.
- **Sets are many-to-many.** A mod can be in any number of sets, and profiles stack sets.
- **The solver is deterministic.** The same mods and knowledge always give the same order,
  which keeps multiplayer lists stable.
- **Unknown mods get a guessed tier.** When neither the user nor the community has
  sorted a mod, WARP reads what's inside its packs (animations, UI, DB tables, art)
  and guesses a tier, falling back to its Steam tags. On 453 hand-sorted mods the
  guess matches about two times in three (`warp classify-report`).
- **Share codes are compact.** Mods travel as workshop ids with a 16-bit version
  fingerprint, and the receiver fills in pack names from its own library. A 150-pack
  list fits in one Discord message. Which side is outdated is settled by asking Steam.

## Working on the UI

`npm run dev` in `app/` serves the UI in a plain browser against a mock backend.
Give the mock real data first:

```sh
cargo run -p warp-cli -- dev-fixture app/src/lib/mock/fixture.json
```

To run the real app, use `npm run tauri dev` in `app/`.
