// App-wide state, loaded once and refreshed after changes.

import { api } from "./api";
import type { Install, LibraryEntry, ModSet, ProfileDef, Role, SyncSummary, Taxonomy, Tier, WorkshopId } from "./types";

type Toast = { text: string; kind: "ok" | "error" };

class AppState {
  taxonomy = $state<Taxonomy>({ tier: [], role: [] });
  library = $state<LibraryEntry[]>([]);
  sets = $state<ModSet[]>([]);
  profiles = $state<ProfileDef[]>([]);
  dataDir = $state("");
  kaedrinDir = $state<string | null>(null);
  /** The installed game, or why it wasn't found. */
  install = $state<Install | null>(null);
  installError = $state<string | null>(null);
  syncing = $state(false);
  loaded = $state(false);
  toast = $state<Toast | null>(null);

  byId = $derived(new Map(this.library.map((e) => [e.info.id, e])));
  byPack = $derived(new Map(this.library.flatMap((e) => e.packs.map((p) => [p.toLowerCase(), e] as const))));

  #toastTimer: ReturnType<typeof setTimeout> | undefined;

  async load() {
    const a = await api();
    const boot = await a.bootstrap();
    this.taxonomy = boot.taxonomy;
    this.dataDir = boot.data_dir;
    this.kaedrinDir = boot.kaedrin_dir;
    if ("Ok" in boot.install) this.install = boot.install.Ok;
    else this.installError = boot.install.Err;
    await this.refresh();
    this.loaded = true;
  }

  async refresh() {
    const a = await api();
    const [library, sets, profiles] = await Promise.all([a.library(), a.sets(), a.profiles()]);
    this.library = library;
    this.sets = sets;
    this.profiles = profiles;
  }

  /** Reads the installed game (subscriptions, versions, pack contents), then reloads. */
  async sync(checkSteam = true): Promise<SyncSummary | undefined> {
    this.syncing = true;
    try {
      const summary = await this.attempt(async () => (await api()).syncInstall(checkSteam));
      if (summary) {
        await this.refresh();
        const changes = [
          summary.new_mods.length && `${summary.new_mods.length} new`,
          summary.unsubscribed.length && `${summary.unsubscribed.length} unsubscribed`,
          summary.packs_indexed && `${summary.packs_indexed} packs read`,
        ].filter(Boolean);
        this.notify(`Synced ${summary.installed} installed mods${changes.length ? ` · ${changes.join(", ")}` : " · nothing changed"}`);
      }
      return summary;
    } finally {
      this.syncing = false;
    }
  }

  tier(key: string): Tier | undefined {
    return this.taxonomy.tier.find((t) => t.key === key);
  }

  role(key: string): Role | undefined {
    return this.taxonomy.role.find((r) => r.key === key);
  }

  /** A mod's display title, falling back to its pack or id. */
  title(id: WorkshopId | null, pack = ""): string {
    const e = id ? this.byId.get(id) : this.byPack.get(pack.toLowerCase());
    return e?.info.title || pack || (id ? `Workshop item ${id}` : "Unknown");
  }

  notify(text: string, kind: Toast["kind"] = "ok") {
    this.toast = { text, kind };
    clearTimeout(this.#toastTimer);
    this.#toastTimer = setTimeout(() => (this.toast = null), kind === "error" ? 7000 : 3500);
  }

  /** Runs an action, turning failures into an error toast. */
  async attempt<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      return await action();
    } catch (e) {
      this.notify(String(e), "error");
      return undefined;
    }
  }
}

export const app = new AppState();
