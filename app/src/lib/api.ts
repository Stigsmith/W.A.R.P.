// The one door to the backend. Inside the desktop app it calls the Rust commands;
// in a plain browser (`npm run dev`) it uses a mock backed by sample data, so the
// UI can be developed and reviewed without building the app.

import type {
  Bootstrap,
  CompareResult,
  FileFilter,
  ImportSummary,
  LibraryEntry,
  ListSource,
  ModKnowledge,
  ModSet,
  ProfileDef,
  ResolvedProfile,
  ShareList,
  WorkshopId,
} from "./types";

export interface Api {
  bootstrap(): Promise<Bootstrap>;
  library(): Promise<LibraryEntry[]>;
  setKnowledge(id: WorkshopId, knowledge: ModKnowledge): Promise<void>;
  sets(): Promise<ModSet[]>;
  saveSet(set: ModSet): Promise<void>;
  renameSet(from: string, to: string): Promise<void>;
  deleteSet(name: string): Promise<void>;
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
    renameSet: (from, to) => invoke("rename_set", { from, to }),
    deleteSet: (name) => invoke("delete_set", { name }),
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
