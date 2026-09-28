<script lang="ts">
  // Pick exactly which mods a profile has. Sets are optional: a mod ticked here is
  // added on top of the profile's sets; a set mod unticked here is left out.
  import { onMount, untrack } from "svelte";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { shortPair, tierColor } from "../lib/format";
  import type { ProfileDef, WorkshopId } from "../lib/types";

  let { profile, onsave, onclose }: { profile: ProfileDef; onsave: (p: ProfileDef) => void; onclose: () => void } = $props();

  // Work on a draft copied once when the picker opens; nothing is saved until Done.
  let include = $state(new Set<WorkshopId>(untrack(() => profile.include)));
  let exclude = $state(new Set<WorkshopId>(untrack(() => profile.exclude)));
  let query = $state("");
  let tier = $state("");
  let onlyChosen = $state(false);

  // What the profile's sets give it: its own copy of each set, not the live set.
  const fromSets = $derived(
    new Set(
      profile.sets.flatMap(
        (name) => profile.set_members?.[name] ?? app.sets.find((s) => s.name.toLowerCase() === name.toLowerCase())?.members ?? [],
      ),
    ),
  );
  const chosen = (id: WorkshopId) => (fromSets.has(id) || include.has(id)) && !exclude.has(id);
  const maxPriority = $derived(Math.max(1, ...app.taxonomy.tier.map((t) => t.priority)));
  const priority = (key: string) => app.tier(key)?.priority ?? -1;

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return app.library
      .filter((e) => e.subscribed || chosen(e.info.id))
      .filter((e) => !tier || e.knowledge.tier === tier)
      .filter((e) => !onlyChosen || chosen(e.info.id))
      .filter((e) => !q || `${e.info.title} ${e.packs.join(" ")} ${e.knowledge.tags.join(" ")}`.toLowerCase().includes(q))
      .sort((a, b) => priority(b.knowledge.tier) - priority(a.knowledge.tier) || a.info.title.localeCompare(b.info.title));
  });

  const count = $derived(app.library.filter((e) => chosen(e.info.id)).length);

  // Mods that look like another version of a mod: each id's counterparts.
  let versions = $state(new Map<WorkshopId, WorkshopId[]>());
  onMount(async () => {
    const pairs = (await app.attempt(async () => (await api()).eitherOrPairs())) ?? [];
    const map = new Map<WorkshopId, WorkshopId[]>();
    for (const { a, b } of pairs) {
      map.set(a, [...(map.get(a) ?? []), b]);
      map.set(b, [...(map.get(b) ?? []), a]);
    }
    versions = map;
  });
  const chosenVersions = (id: WorkshopId) => (versions.get(id) ?? []).filter(chosen);

  function set(id: WorkshopId, on: boolean) {
    const inc = new Set(include);
    const exc = new Set(exclude);
    if (on) {
      exc.delete(id);
      if (!fromSets.has(id)) inc.add(id);
    } else {
      inc.delete(id);
      if (fromSets.has(id)) exc.add(id);
    }
    include = inc;
    exclude = exc;
  }

  function setAll(on: boolean) {
    for (const e of rows) set(e.info.id, on);
  }

  function done() {
    onsave({ ...profile, include: [...include], exclude: [...exclude] });
  }
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="picker card forged" role="dialog" aria-modal="true" aria-label="Pick mods">
    <header>
      <div>
        <h2>Mods in {profile.name}</h2>
        <p class="muted">
          {count} chosen{#if profile.sets.length}&nbsp;· sets: {profile.sets.join(" + ")}{/if}
        </p>
      </div>
      <div class="actions">
        <button class="ghost" onclick={onclose}>Cancel</button>
        <button class="primary" onclick={done}>Done</button>
      </div>
    </header>

    <div class="filters">
      <!-- svelte-ignore a11y_autofocus -->
      <input class="search" bind:value={query} placeholder="Search mods, packs, tags…" autofocus />
      <select bind:value={tier}>
        <option value="">All tiers</option>
        {#each app.taxonomy.tier as t (t.key)}<option value={t.key}>{t.name}</option>{/each}
      </select>
      <label class="check"><input type="checkbox" bind:checked={onlyChosen} /> Only chosen</label>
      <span class="spacer"></span>
      <button class="small" onclick={() => setAll(true)}>Tick all shown</button>
      <button class="small ghost" onclick={() => setAll(false)}>Untick all shown</button>
    </div>

    <ul class="list scroll">
      {#each rows as e (e.info.id)}
        {@const on = chosen(e.info.id)}
        {@const t = app.tier(e.knowledge.tier)}
        <li class:on>
          <label>
            <input type="checkbox" checked={on} onchange={(ev) => set(e.info.id, ev.currentTarget.checked)} />
            <span class="names">
              <span class="title">{e.info.title || e.packs[0]}</span>
              <span class="mono faint">{e.packs.join(", ")}</span>
            </span>
            <span class="tier" style:--tier={tierColor(t?.priority ?? 0, maxPriority)}>{t?.name ?? e.knowledge.tier}</span>
            {#each chosenVersions(e.info.id) as other (other)}
              <span
                class="chip {on ? 'danger' : 'warn'}"
                title="{e.info.title} and {app.title(other)} look like two versions of the same mod. Keep one."
              >
                {on ? "clashes with" : "other version of"}
                {shortPair(e.info.title, app.title(other), 24)[1]}
              </span>
            {/each}
            {#if fromSets.has(e.info.id)}<span class="chip brass" title="Comes from one of this profile's sets">set</span>{/if}
            {#if !e.subscribed}<span class="chip warn">not installed</span>{/if}
          </label>
        </li>
      {:else}
        <li class="empty">No mods match.</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgb(0 0 0 / 0.6);
    display: grid;
    place-items: center;
    padding: 28px;
  }

  .picker {
    width: min(920px, 100%);
    height: min(760px, 100%);
    display: flex;
    flex-direction: column;
    background: rgb(10 14 9 / 0.97);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    padding: 18px 20px 12px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .filters {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 20px 12px;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--border);
  }

  .search {
    width: 260px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 13px;
    cursor: pointer;
  }

  .spacer {
    flex: 1;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 6px 10px 14px;
    flex: 1;
    min-height: 0;
  }

  li label {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  li label:hover {
    background: var(--surface);
  }

  li.on label {
    background: linear-gradient(90deg, rgb(168 242 63 / 0.07), transparent 70%);
  }

  .names {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .names > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  li:not(.on) .title {
    color: var(--muted);
  }

  .tier {
    color: var(--tier);
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
  }
</style>
