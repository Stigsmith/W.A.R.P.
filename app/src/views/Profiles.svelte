<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { count } from "../lib/format";
  import ConflictMap from "../components/ConflictMap.svelte";
  import OrderList from "../components/OrderList.svelte";
  import SharePanel from "../components/SharePanel.svelte";
  import type { ConflictReport, ProfileDef, ResolvedProfile, ShareList } from "../lib/types";

  let selected = $state<string | null>(app.profiles[0]?.name ?? null);
  const profile = $derived(app.profiles.find((p) => p.name === selected) ?? null);

  let resolved = $state<ResolvedProfile | null>(null);
  let loading = $state(false);
  let filter = $state("");
  let sharing = $state<ShareList | null>(null);
  let newName = $state<string | null>(null);
  let tab = $state<"order" | "conflicts">("order");
  let conflicts = $state<ConflictReport | null>(null);
  let launching = $state(false);

  let token = 0;
  $effect(() => {
    const p = profile;
    sharing = null;
    if (!p) {
      resolved = null;
      return;
    }
    const mine = ++token;
    loading = true;
    conflicts = null;
    const snapshot = $state.snapshot(p);
    api()
      .then((a) => {
        a.conflicts(snapshot)
          .then((c) => {
            if (mine === token) conflicts = c;
          })
          .catch(() => {});
        return a.resolveProfile(snapshot);
      })
      .then((r) => {
        if (mine === token) resolved = r;
      })
      .catch((e) => app.notify(String(e), "error"))
      .finally(() => {
        if (mine === token) loading = false;
      });
  });

  async function save(p: ProfileDef) {
    if ((await app.attempt(async () => (await api()).saveProfile(p))) === undefined) return;
    await app.refresh();
  }

  function toggleSet(name: string) {
    if (!profile) return;
    const sets = profile.sets.includes(name) ? profile.sets.filter((s) => s !== name) : [...profile.sets, name];
    save({ ...$state.snapshot(profile), sets });
  }

  async function create() {
    const name = newName?.trim();
    if (!name) return;
    if (app.profiles.some((p) => p.name.toLowerCase() === name.toLowerCase())) {
      app.notify(`A profile named "${name}" already exists`, "error");
      return;
    }
    await save({ name, sets: [], include: [], exclude: [], pins: [] });
    selected = name;
    newName = null;
  }

  async function remove() {
    if (!profile) return;
    const a = await api();
    if (!(await a.confirm(`Delete the profile "${profile.name}"? Its sets and mods are kept.`))) return;
    await app.attempt(() => a.deleteProfile(profile.name));
    await app.refresh();
    selected = app.profiles[0]?.name ?? null;
  }

  async function exportKaedrin() {
    if (!profile || !resolved) return;
    const a = await api();
    const ok = await a.confirm(
      `Write "${profile.name}" as a Kaedrin Mod Manager profile?\n\nAn existing Kaedrin profile with the same name is replaced.`,
    );
    if (!ok) return;
    const path = await app.attempt(() => a.exportKaedrin(resolved!.order.placements.map((p) => p.pack), profile.name));
    if (path) app.notify(`Saved to ${path}`);
  }

  async function play() {
    if (!profile || !resolved) return;
    const a = await api();
    const ok = await a.confirm(
      `Start Warhammer III with "${profile.name}" (${count(resolved.order.placements.length, "pack")})?\n\n` +
        "WARP writes its own modlist (warp_mods.txt) in the game folder and starts the game directly.",
      "Play",
    );
    if (!ok) return;
    launching = true;
    const path = await app.attempt(() => a.play($state.snapshot(profile)));
    launching = false;
    if (path) app.notify("Starting Warhammer III… May the Horned Rat smile upon you.");
  }

  /** Adds installed mods to this profile on top of its sets. */
  async function addMods(ids: string[]) {
    if (!profile) return;
    const include = [...new Set([...profile.include, ...ids])];
    const exclude = profile.exclude.filter((id) => !ids.includes(id));
    await save({ ...$state.snapshot(profile), include, exclude });
    app.notify(`Added ${count(ids.length, "mod")} to ${profile.name}`);
  }

  /** Leaves mods that are no longer installed out of this profile. */
  async function dropUninstalled() {
    if (!profile || !resolved) return;
    const exclude = [...new Set([...profile.exclude, ...resolved.unsubscribed])];
    await save({ ...$state.snapshot(profile), exclude });
    app.notify(`Left out ${count(resolved.unsubscribed.length, "uninstalled mod")}`);
  }

  async function share() {
    if (!profile) return;
    sharing = (await app.attempt(async () => (await api()).shareList($state.snapshot(profile)))) ?? null;
  }

  const warnings = $derived.by(() => {
    if (!resolved) return [];
    const out: { kind: "danger" | "warn"; text: string; fix?: { label: string; run: () => void } }[] = [];
    if (resolved.missing_requirements.length) {
      const missing = [...new Set(resolved.missing_requirements.map(([, req]) => req))];
      const addable = missing.filter((id) => app.byId.get(id)?.subscribed);
      for (const [mod, req] of resolved.missing_requirements) {
        out.push({ kind: "warn", text: `${app.title(mod)} depends on ${app.title(req)}, which isn't in this profile.` });
      }
      if (addable.length) {
        out[out.length - 1].fix = { label: `Add ${count(addable.length, "missing mod")}`, run: () => addMods(addable) };
      }
    }
    for (const [a, b] of resolved.incompatibilities) {
      out.push({ kind: "danger", text: `${app.title(a)} and ${app.title(b)} are known not to work together.` });
    }
    for (const cycle of resolved.order.cycles) {
      out.push({ kind: "danger", text: `Contradictory rules: ${cycle.map((r) => `${r.above} above ${r.below}`).join(", ")}.` });
    }
    if (resolved.unsubscribed.length) {
      out.push({
        kind: "warn",
        text: `${count(resolved.unsubscribed.length, "mod")} in this profile ${resolved.unsubscribed.length === 1 ? "isn't" : "aren't"} installed any more: ${resolved.unsubscribed.map((id) => app.title(id)).join(", ")}.`,
        fix: { label: "Leave them out", run: dropUninstalled },
      });
    }
    if (resolved.mods_without_packs.length) {
      out.push({ kind: "warn", text: `${count(resolved.mods_without_packs.length, "mod")} with no known pack file (left out).` });
    }
    if (resolved.unknown_mods.length) {
      out.push({ kind: "warn", text: `${count(resolved.unknown_mods.length, "mod")} not in your library.` });
    }
    return out;
  });

  const raisedCount = $derived(resolved?.order.placements.filter((p) => p.reasons.some((r) => r.kind === "raised")).length ?? 0);
</script>

<div class="profiles">
  <section class="list">
    <div class="list-head">
      <span class="label">Profiles</span>
      <button class="ghost small" onclick={() => (newName = "")}>+ New</button>
    </div>
    {#if newName !== null}
      <form
        class="new"
        onsubmit={(e) => {
          e.preventDefault();
          create();
        }}
      >
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={newName} placeholder="Profile name" autofocus onkeydown={(e) => e.key === "Escape" && (newName = null)} />
      </form>
    {/if}
    {#each app.profiles as p (p.name)}
      <button class="item" class:active={p.name === selected} onclick={() => (selected = p.name)}>
        <span>{p.name}</span>
        <small class="faint">{p.sets.join(" + ") || "no sets yet"}</small>
      </button>
    {:else}
      <p class="faint hint">No profiles yet. Create one and stack some sets.</p>
    {/each}
  </section>

  <section class="main">
    {#if !profile}
      <div class="empty">Pick or create a profile.</div>
    {:else}
      <header>
        <div class="title">
          <h1>{profile.name}</h1>
          <p class="muted">
            {#if resolved}
              {count(resolved.order.placements.length, "pack")}{#if raisedCount}&nbsp;· {raisedCount} moved by rules{/if}
            {:else}&nbsp;{/if}
          </p>
        </div>
        <div class="actions">
          <button class="ghost small" onclick={remove}>Delete</button>
          <button onclick={exportKaedrin} disabled={!resolved}>Export to Kaedrin</button>
          <button onclick={share} disabled={!resolved}>Share</button>
          <button class="primary play" onclick={play} disabled={!resolved || !app.install || launching} title={app.install ? "" : "The game wasn't found"}>
            {launching ? "Starting…" : "▶ Play"}
          </button>
        </div>
      </header>

      {#if sharing}
        <SharePanel list={sharing} onclose={() => (sharing = null)} />
      {/if}

      <div class="layers">
        <span class="label">Sets in this profile</span>
        <div class="chips">
          {#each app.sets as s (s.name)}
            {@const at = profile.sets.indexOf(s.name)}
            <button class="set" class:on={at >= 0} onclick={() => toggleSet(s.name)} title={`${s.members.length} mods`}>
              {#if at >= 0}<span class="n">{at + 1}</span>{/if}
              {s.name}
              <span class="faint">{s.members.length}</span>
            </button>
          {/each}
        </div>
      </div>

      {#if warnings.length}
        <ul class="warnings">
          {#each warnings as w, i (i)}
            <li class={w.kind}>
              <span>{w.text}</span>
              {#if w.fix}<button class="small" onclick={w.fix.run}>{w.fix.label}</button>{/if}
            </li>
          {/each}
        </ul>
      {/if}

      <div class="order-head">
        <div class="tabs" role="tablist">
          <button role="tab" class:active={tab === "order"} aria-selected={tab === "order"} onclick={() => (tab = "order")}>
            Load order <span class="faint">· top wins</span>
          </button>
          <button role="tab" class:active={tab === "conflicts"} aria-selected={tab === "conflicts"} onclick={() => (tab = "conflicts")}>
            Conflicts
            {#if conflicts}
              {@const serious = conflicts.pairs.filter((p) => !p.intended && p.severity !== "low").length}
              <span class="chip" class:danger={conflicts.pairs.some((p) => !p.intended && p.severity === "high")} class:warn={serious > 0}>{serious || "✓"}</span>
            {/if}
          </button>
        </div>
        {#if tab === "order"}<input class="search" bind:value={filter} placeholder="Find a pack…" />{/if}
      </div>
      <div class="order scroll" class:loading>
        {#if tab === "conflicts"}
          {#if conflicts}
            <ConflictMap report={conflicts} />
          {:else}
            <div class="empty">Reading pack contents…</div>
          {/if}
        {:else if resolved}
          {#if resolved.order.placements.length === 0}
            <div class="empty">Add a set above to fill this profile.</div>
          {:else}
            <OrderList placements={resolved.order.placements} {filter} />
          {/if}
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  .profiles {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    height: 100%;
  }

  .list {
    border-right: 1px solid var(--border);
    padding: 18px 10px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow: auto;
  }

  .list-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 6px 8px;
  }

  .new input {
    width: 100%;
    margin-bottom: 6px;
  }

  .item {
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    background: transparent;
    border-color: transparent;
    text-align: left;
    white-space: normal;
  }

  .item.active {
    background: var(--surface-2);
    border-color: var(--border-strong);
  }

  .item.active span {
    color: var(--accent);
  }

  .item small {
    font-size: 11.5px;
  }

  .hint {
    padding: 6px;
    font-size: 13px;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px 24px 0;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .layers {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .set {
    padding: 4px 11px;
    border-radius: 999px;
    font-size: 13px;
    background: var(--surface);
    color: var(--muted);
  }

  .set.on {
    color: var(--text);
    border-color: var(--accent-dim);
    background: var(--ok-dim);
  }

  .set .n {
    display: inline-grid;
    place-items: center;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: var(--accent);
    color: #0b1406;
    font-size: 11px;
    font-weight: 700;
  }

  .warnings {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 120px;
    overflow: auto;
  }

  .warnings li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 12px;
    border-radius: var(--radius-sm);
    font-size: 13px;
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  .tabs button {
    background: transparent;
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 6px 12px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .tabs button:hover:not(:disabled) {
    background: transparent;
    border: 0;
    border-bottom: 2px solid var(--brass-dim);
    color: var(--text);
  }

  .tabs button.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .play {
    padding-left: 18px;
    padding-right: 18px;
    box-shadow: 0 0 16px var(--accent-glow);
  }

  .warnings .danger {
    background: var(--danger-dim);
    color: #ffc9c1;
    border-left: 3px solid var(--danger);
  }

  .warnings .warn {
    background: var(--warn-dim);
    color: #f5dfae;
    border-left: 3px solid var(--warn);
  }

  .order-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .search {
    width: 240px;
    padding: 5px 10px;
  }

  .order {
    flex: 1;
    min-height: 0;
    padding-bottom: 24px;
    transition: opacity 0.15s;
  }

  .order.loading {
    opacity: 0.5;
  }
</style>
