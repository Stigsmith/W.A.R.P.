// The one door to the backend. Inside the desktop app it calls the Rust commands;
// in a plain browser (`npm run dev`) it uses a mock backed by sample data, so the
// UI can be developed and reviewed without building the app.

import type {
  Bootstrap,
  CompareResult,
  ConflictReport,
  DbOverlap,
  FileFilter,
  ImportSummary,
  LibraryEntry,
  ListSource,
  ModKnowledge,
  ModSet,
  ProfileDef,
  ResolvedProfile,
  SetUpdate,
  ShareList,
  SyncSummary,
  WorkshopId,
} from "./types";

export interface Api {
  bootstrap(): Promise<Bootstrap>;
  library(): Promise<LibraryEntry[]>;
  setKnowledge(id: WorkshopId, knowledge: ModKnowledge): Promise<void>;
  sets(): Promise<ModSet[]>;
  saveSet(set: ModSet): Promise<void>;
  /** Fails if a set with that name (in any case) exists. */
  createSet(name: string, members: WorkshopId[]): Promise<void>;
  /** Adds and removes mods in one go. */
  editSet(name: string, add: WorkshopId[], remove: WorkshopId[]): Promise<void>;
  /** Renames a set, including in the profiles that use it. */
  renameSet(from: string, to: string): Promise<void>;
  deleteSet(name: string): Promise<void>;
  /** Set changes each profile hasn't taken or dismissed, by profile name. */
  setUpdates(): Promise<Record<string, SetUpdate[]>>;
  /** Takes the new contents of these sets (all changed ones if empty); returns the profile. */
  applySetUpdates(profile: string, sets: string[]): Promise<ProfileDef>;
  /** Stops showing these sets' current changes (all if empty); returns the profile. */
  dismissSetUpdates(profile: string, sets: string[]): Promise<ProfileDef>;
  profiles(): Promise<ProfileDef[]>;
  saveProfile(profile: ProfileDef): Promise<void>;
  deleteProfile(name: string): Promise<void>;
  resolveProfile(profile: ProfileDef): Promise<ResolvedProfile>;
  shareList(profile: ProfileDef): Promise<ShareList>;
  encodeShare(list: ShareList): Promise<string>;
  saveWarpFile(list: ShareList, path: string): Promise<void>;
  loadList(source: ListSource): Promise<ShareList>;
  compare(a: ShareList, b: ShareList, checkSteam: boolean): Promise<CompareResult>;
  exportKaedrin(packs: string[], name: string, path?: string): Promise<string>;
  importV1(path: string): Promise<ImportSummary>;
  refreshSteam(): Promise<number>;
  syncInstall(checkSteam: boolean): Promise<SyncSummary>;
  conflicts(profile: ProfileDef): Promise<ConflictReport>;
  /** Installed mods that look like two versions of the same mod. */
  eitherOrPairs(): Promise<DbOverlap[]>;
  /** Writes the profile's modlist and starts the game; resolves to the modlist path. */
  play(profile: ProfileDef): Promise<string>;
  /** Everything W.A.R.P. sees on this PC as plain text, with `extra` added. */
  diagnostics(extra: string): Promise<string>;
  /** Puts an error the user saw into the log. */
  logUi(line: string): Promise<void>;
  // Platform
  pickFile(filters: FileFilter[]): Promise<string | null>;
  pickSavePath(defaultName: string, filters: FileFilter[]): Promise<string | null>;
  confirm(message: string, title?: string): Promise<boolean>;
  openUrl(url: string): Promise<void>;
}

export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const dialog = await import("@tauri-apps/plugin-dialog");
  const opener = await import("@tauri-apps/plugin-opener");
  return {
    bootstrap: () => invoke("bootstrap"),
    library: () => invoke("library"),
    setKnowledge: (id, knowledge) => invoke("set_knowledge", { id, knowledge }),
    sets: () => invoke("sets"),
    saveSet: (set) => invoke("save_set", { set }),
    createSet: (name, members) => invoke("create_set", { name, members }),
    editSet: (name, add, remove) => invoke("edit_set", { name, add, remove }),
    renameSet: (from, to) => invoke("rename_set", { from, to }),
    deleteSet: (name) => invoke("delete_set", { name }),
    setUpdates: () => invoke("set_updates"),
    applySetUpdates: (profile, sets) => invoke("apply_set_updates", { profile, sets }),
    dismissSetUpdates: (profile, sets) => invoke("dismiss_set_updates", { profile, sets }),
    profiles: () => invoke("profiles"),
    saveProfile: (profile) => invoke("save_profile", { profile }),
    deleteProfile: (name) => invoke("delete_profile", { name }),
    resolveProfile: (profile) => invoke("resolve_profile", { profile }),
    shareList: (profile) => invoke("share_list", { profile }),
    encodeShare: (list) => invoke("encode_share", { list }),
    saveWarpFile: (list, path) => invoke("save_warp_file", { list, path }),
    loadList: (source) => invoke("load_list", { source }),
    compare: (a, b, checkSteam) => invoke("compare", { a, b, checkSteam }),
    exportKaedrin: (packs, name, path) => invoke("export_kaedrin", { packs, name, path: path ?? null }),
    importV1: (path) => invoke("import_v1", { path }),
    refreshSteam: () => invoke("refresh_steam"),
    syncInstall: (checkSteam) => invoke("sync_install", { checkSteam }),
    conflicts: (profile) => invoke("conflicts", { profile }),
    eitherOrPairs: () => invoke("either_or_pairs"),
    play: (profile) => invoke("play", { profile }),
    diagnostics: (extra) => invoke("diagnostics", { extra }),
    logUi: (line) => invoke("log_ui", { line }),
    pickFile: async (filters) => {
      const picked = await dialog.open({ multiple: false, directory: false, filters });
      return typeof picked === "string" ? picked : null;
    },
    pickSavePath: (defaultPath, filters) => dialog.save({ defaultPath, filters }),
    confirm: (message, title) => dialog.ask(message, { title: title ?? "W.A.R.P.", kind: "warning" }),
    openUrl: (url) => opener.openUrl(url),
  };
}

let instance: Promise<Api> | null = null;

export function api(): Promise<Api> {
  // The mock only exists in dev builds; release builds drop it (and its sample data) entirely.
  instance ??= isDesktop
    ? tauriApi()
    : import.meta.env.DEV
      ? import("./mock/mock").then((m) => m.mockApi())
      : Promise.reject(new Error("W.A.R.P. must run inside its desktop app"));
  return instance;
}
