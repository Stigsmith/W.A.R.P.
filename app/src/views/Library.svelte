<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { date, tierColor } from "../lib/format";
  import ModPanel from "../components/ModPanel.svelte";
  import type { Source } from "../lib/types";

  let query = $state("");
  let tier = $state("");
  let set = $state("");
  let showUnsubscribed = $state(false);
  let selectedId = $state<string | null>(null);

  const selected = $derived(selectedId ? app.byId.get(selectedId) : undefined);
  const maxPriority = $derived(Math.max(1, ...app.taxonomy.tier.map((t) => t.priority)));
  const priority = (key: string) => app.tier(key)?.priority ?? -1;

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return app.library
      .filter((e) => showUnsubscribed || e.subscribed)
      .filter((e) => !tier || e.knowledge.tier === tier)
      .filter((e) => !set || (set === "__none" ? e.sets.length === 0 : e.sets.includes(set)))
      .filter((e) => !q || `${e.info.title} ${e.packs.join(" ")} ${e.knowledge.tags.join(" ")}`.toLowerCase().includes(q))
      .sort((a, b) => priority(b.knowledge.tier) - priority(a.knowledge.tier) || a.info.title.localeCompare(b.info.title));
  });

  const sourceMark: Record<Source, string> = { user: "you", community: "", heuristic: "guess", default: "?" };
</script>

<div class="library" class:with-panel={!!selected}>
  <div class="body">
    <header>
      <h1>Library</h1>
      <div class="filters">
        <input class="search" bind:value={query} placeholder="Search mods, packs, tags…" />
        <select bind:value={tier}>
          <option value="">All tiers</option>
          {#each app.taxonomy.tier as t (t.key)}<option value={t.key}>{t.name}</option>{/each}
        </select>
        <select bind:value={set}>
          <option value="">All sets</option>
          <option value="__none">In no set</option>
          {#each app.sets as s (s.name)}<option value={s.name}>{s.name}</option>{/each}
        </select>
        <label class="check"><input type="checkbox" bind:checked={showUnsubscribed} /> Unsubscribed</label>
        <span class="faint count">{rows.length} shown</span>
      </div>
    </header>

    <div class="table scroll">
      <table>
        <thead>
          <tr>
            <th>Mod</th>
            <th>Tier</th>
            <th>Role</th>
            <th>Sets</th>
            <th class="right">Updated</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as e (e.info.id)}
            {@const t = app.tier(e.knowledge.tier)}
            <tr class:active={e.info.id === selectedId} class:off={!e.subscribed} onclick={() => (selectedId = e.info.id)}>
              <td class="mod">
                <span class="title">{e.info.title || e.packs[0]}</span>
                <span class="mono faint">{e.packs[0] ?? "—"}</span>
              </td>
              <td>
                <span class="tier" style:--tier={tierColor(t?.priority ?? 0, maxPriority)}>{t?.name ?? e.knowledge.tier}</span>
                {#if sourceMark[e.knowledge.tier_source]}<span class="src">{sourceMark[e.knowledge.tier_source]}</span>{/if}
              </td>
              <td class="muted">{app.role(e.knowledge.role)?.name ?? e.knowledge.role}</td>
              <td class="sets muted">{e.sets.join(", ")}</td>
              <td class="right faint">{date(e.info.time_updated)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if rows.length === 0}<div class="empty">No mods match.</div>{/if}
    </div>
  </div>

  {#if selected}
    <ModPanel entry={selected} onclose={() => (selectedId = null)} />
  {/if}
</div>

<style>
  .library {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    height: 100%;
  }

  .library.with-panel {
    grid-template-columns: minmax(0, 1fr) 380px;
  }

  .body {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 20px 24px 0;
    gap: 14px;
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }

  .search {
    width: 280px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 13px;
    cursor: pointer;
  }

  .count {
    margin-left: auto;
    font-size: 12.5px;
  }

  .table {
    flex: 1;
    min-height: 0;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--bg);
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    z-index: 1;
  }

  td {
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }

  tr {
    cursor: pointer;
  }

  tbody tr:hover {
    background: var(--surface);
  }

  tr.active {
    background: var(--surface-2);
  }

  tr.off {
    opacity: 0.55;
  }

  .mod {
    display: flex;
    flex-direction: column;
    max-width: 520px;
  }

  .mod span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tier {
    color: var(--tier);
    font-weight: 600;
    font-size: 13px;
    white-space: nowrap;
  }

  td.muted {
    white-space: nowrap;
  }

  .src {
    margin-left: 6px;
    font-size: 11px;
    color: var(--info);
  }

  .sets {
    font-size: 12.5px;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .right {
    text-align: right;
    white-space: nowrap;
  }
</style>
