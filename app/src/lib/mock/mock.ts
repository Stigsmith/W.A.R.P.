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
  ShareList,
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
    taxonomy: { tier: [{ key: "core", name: "Core", priority: 0, description: "" }], role: [{ key: "content", name: "Content", priority: 20, description: "" }] },
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
      for (const e of fx.library) {
        const has = set.members.includes(e.info.id);
        e.sets = e.sets.filter((s) => s !== set.name).concat(has ? [set.name] : []);
      }
    },
    renameSet: async (from, to) => {
      const s = fx.sets.find((s) => s.name === from);
      if (s) s.name = to;
    },
    deleteSet: async (name) => {
      fx.sets = fx.sets.filter((s) => s.name !== name);
    },
    profiles: () => delay(fx.profiles),
    saveProfile: async (p) => {
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
