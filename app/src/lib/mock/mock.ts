// Browser-only stand-in for the Rust backend. Serves `fixture.json`, which
// `warp dev-fixture` generates from a real library:
//   cargo run -p warp-cli -- dev-fixture app/src/lib/mock/fixture.json

import type { Api } from "../api";
import type {
  Bootstrap,
  CompareResult,
  ConflictReport,
  DbOverlap,
  LibraryEntry,
  ModSet,
  ProfileDef,
  ResolvedProfile,
  SetUpdate,
  ShareList,
  WorkshopId,
} from "../types";

interface Fixture {
  bootstrap: Bootstrap;
  library: LibraryEntry[];
  sets: ModSet[];
  profiles: ProfileDef[];
  resolved: Record<string, ResolvedProfile>;
  conflicts: Record<string, ConflictReport>;
  either_or: DbOverlap[];
  share: { list: ShareList; code: string };
  compare: { code: string; result: CompareResult };
}

const EMPTY: Fixture = {
  bootstrap: {
    version: "0.0.0-mock",
    restorable: null,
    taxonomy: { tier: [{ key: "core", name: "Core", priority: 0, description: "", hint: "" }], role: [{ key: "content", name: "Content", priority: 20, description: "", hint: "" }] },
    mod_count: 0,
    data_dir: "(mock)",
    kaedrin_dir: null,
    install: { Err: "mock: no game" },
  },
  library: [],
  sets: [],
  profiles: [],
  resolved: {},
  conflicts: {},
  either_or: [],
  share: { list: { name: "", entries: [] }, code: "" },
  compare: { code: "", result: { diff: { identical: true, common: 0, only_in_a: [], only_in_b: [], moves: [], version_mismatches: [] }, a: { name: "", entries: [] }, b: { name: "", entries: [] }, titles: {} } },
};

const delay = <T>(v: T, ms = 120): Promise<T> => new Promise((r) => setTimeout(() => r(structuredClone(v)), ms));

export async function mockApi(): Promise<Api> {
  const files = import.meta.glob<{ default: Fixture }>("./fixture.json");
  const loader = files["./fixture.json"];
  const fx: Fixture = loader ? (await loader()).default : EMPTY;
  if (!loader) console.warn("mock: no fixture.json - run `warp dev-fixture` to generate one");

  const firstResolved = (): ResolvedProfile => Object.values(fx.resolved)[0];

  // Sets and profile copies, following the same rules as warp-core's `sets` module.
  const findSet = (name: string) => fx.sets.find((s) => s.name.toLowerCase() === name.toLowerCase());
  const seen = (name: string) => {
    const s = findSet(name);
    return s ? [...s.members].sort().join(",") : "deleted";
  };
  const relabel = () => {
    for (const e of fx.library) e.sets = fx.sets.filter((s) => s.members.includes(e.info.id)).map((s) => s.name);
  };
  const take = (p: ProfileDef, name: string) => {
    if (!p.sets.includes(name)) return;
    p.set_members ??= {};
    delete p.dismissed?.[name];
    const s = findSet(name);
    if (s) p.set_members[name] = [...s.members];
    else if (name in p.set_members) {
      p.sets = p.sets.filter((x) => x !== name);
      delete p.set_members[name];
    } else p.set_members[name] = [];
  };
  const fill = (p: ProfileDef) => {
    for (const name of [...p.sets]) if (p.set_changes === "follow" || !(name in (p.set_members ?? {}))) take(p, name);
    for (const name of Object.keys(p.set_members ?? {})) if (!p.sets.includes(name)) delete p.set_members![name];
  };
  const pending = (p: ProfileDef): SetUpdate[] => {
    if ((p.set_changes ?? "ask") !== "ask") return [];
    return p.sets.flatMap((name) => {
      const copy = p.set_members?.[name];
      if (!copy || p.dismissed?.[name] === seen(name)) return [];
      const live = findSet(name);
      const now = new Set<WorkshopId>(live?.members ?? []);
      const had = new Set(copy);
      const added = [...now].filter((id) => !had.has(id));
      const removed = [...had].filter((id) => !now.has(id));
      return live && !added.length && !removed.length ? [] : [{ set: name, added, removed, deleted: !live }];
    });
  };
  const setsChanged = () => {
    relabel();
    for (const p of fx.profiles) if (p.set_changes === "follow") fill(p);
  };
  const changeProfile = (name: string, sets: string[], change: (p: ProfileDef, set: string) => void) => {
    const p = fx.profiles.find((x) => x.name === name);
    if (!p) throw new Error(`there is no profile called "${name}"`);
    for (const s of sets.length ? sets : pending(p).map((u) => u.set)) change(p, s);
    fill(p);
    return delay(p);
  };

  return {
    bootstrap: () => delay(fx.bootstrap),
    library: () => delay(fx.library),
    setKnowledge: async (id, knowledge) => {
      const e = fx.library.find((m) => m.info.id === id);
      if (e) {
        e.user = knowledge;
        if (knowledge.tier) Object.assign(e.knowledge, { tier: knowledge.tier, tier_source: "user" });
        if (knowledge.role) Object.assign(e.knowledge, { role: knowledge.role, role_source: "user" });
      }
    },
    sets: () => delay(fx.sets),
    saveSet: async (set) => {
      const i = fx.sets.findIndex((s) => s.name.toLowerCase() === set.name.toLowerCase());
      if (i >= 0) fx.sets[i] = set;
      else fx.sets.push(set);
      setsChanged();
    },
    createSet: async (name, members) => {
      if (findSet(name)) throw new Error(`there is already a set called "${name.trim()}"`);
      fx.sets.push({ name: name.trim(), members: [...members] });
      fx.sets.sort((a, b) => a.name.localeCompare(b.name));
      setsChanged();
    },
    editSet: async (name, add, remove) => {
      const s = findSet(name);
      if (!s) throw new Error(`there is no set called "${name}"`);
      s.members = [...new Set([...s.members, ...add])].filter((id) => !remove.includes(id));
      setsChanged();
    },
    renameSet: async (from, to) => {
      const s = findSet(from);
      if (!s) throw new Error(`there is no set called "${from}"`);
      if (from.toLowerCase() !== to.toLowerCase() && findSet(to)) throw new Error(`there is already a set called "${to}"`);
      s.name = to.trim();
      for (const p of fx.profiles) {
        const old = p.sets.find((x) => x.toLowerCase() === from.toLowerCase());
        if (!old) continue;
        p.sets = p.sets.map((x) => (x === old ? s.name : x));
        if (p.set_members?.[old]) {
          p.set_members[s.name] = p.set_members[old];
          delete p.set_members[old];
        }
      }
      setsChanged();
    },
    deleteSet: async (name) => {
      fx.sets = fx.sets.filter((s) => s.name !== name);
      setsChanged();
    },
    setUpdates: () => delay(Object.fromEntries(fx.profiles.map((p) => [p.name, pending(p)]).filter(([, u]) => u.length))),
    applySetUpdates: (profile, sets) => changeProfile(profile, sets, take),
    dismissSetUpdates: (profile, sets) =>
      changeProfile(profile, sets, (p, s) => {
        p.dismissed = { ...p.dismissed, [s]: seen(s) };
      }),
    profiles: () => delay(fx.profiles),
    saveProfile: async (p) => {
      fill(p);
      const i = fx.profiles.findIndex((x) => x.name.toLowerCase() === p.name.toLowerCase());
      if (i >= 0) fx.profiles[i] = p;
      else fx.profiles.push(p);
    },
    deleteProfile: async (name) => {
      fx.profiles = fx.profiles.filter((p) => p.name !== name);
    },
    resolveProfile: (p) => delay({ ...(fx.resolved[p.name] ?? firstResolved()), profile: p }, 250),
    shareList: () => delay(fx.share.list),
    encodeShare: () => delay(fx.share.code),
    saveWarpFile: async (_list, path) => console.info("mock: would save", path),
    loadList: (source) => delay(source.kind === "code" ? fx.compare.result.b : fx.share.list),
    compare: () => delay({ ...fx.compare.result, titles: { "3100000001": "A Mod Only Your Friend Has" } }, 400),
    exportKaedrin: async (_packs, name) => `${fx.bootstrap.kaedrin_dir}\\profile_${name}.txt`,
    importV1: () => delay({ mods: fx.library.length, sets: fx.sets.map((s) => s.name), profile: fx.profiles[0]?.name ?? null, overrides: 0, unresolved_dependencies: [] }, 600),
    refreshSteam: () => delay(fx.library.length, 800),
    syncInstall: () =>
      delay(
        { installed: 428, new_mods: [], unsubscribed: [], resubscribed: [], packs_indexed: 0, packs_cached: 428, pack_errors: [], steam_refreshed: 0 },
        900,
      ),
    conflicts: (p) => delay(fx.conflicts[p.name] ?? Object.values(fx.conflicts)[0] ?? { pairs: [], shadowed: [], not_indexed: [] }, 300),
    eitherOrPairs: () => delay(fx.either_or ?? []),
    play: async () => {
      console.info("mock: would start the game");
      return "Ok" in fx.bootstrap.install ? `${fx.bootstrap.install.Ok.game_dir}/warp_mods.txt` : "warp_mods.txt";
    },
    diagnostics: async (extra) =>
      `W.A.R.P. ${fx.bootstrap.version ?? "mock"} report (browser preview)\nLibrary: ${fx.library.length} mods, ${fx.sets.length} sets, ${fx.profiles.length} profiles\n\n${extra}\n`,
    logUi: async (line) => console.info("mock log:", line),
    backups: () =>
      delay([
        { path: "C:\\mock\\backups\\warp-1790000000.db", at: Math.floor(Date.now() / 1000) - 3600, mods: fx.library.length, sets: fx.sets.length, profiles: fx.profiles.length },
        { path: "C:\\mock\\backups\\warp-1789900000.db", at: Math.floor(Date.now() / 1000) - 90000, mods: fx.library.length, sets: fx.sets.length - 1, profiles: 1 },
      ]),
    restoreBackup: async (path) => console.info("mock: would restore", path),
    pickFile: async () => "C:\\demo\\WARP Database.zip",
    pickSavePath: async (name) => `C:\\demo\\${name}`,
    confirm: async (message) => window.confirm(message),
    openUrl: async (url) => console.info("mock: open", url),
  };
}

/** The staged friend's code, for trying the Multiplayer screen in the browser. */
export async function mockFriendCode(): Promise<string> {
  const loader = import.meta.glob<{ default: Fixture }>("./fixture.json")["./fixture.json"];
  return loader ? (await loader()).default.compare.code : "";
}
