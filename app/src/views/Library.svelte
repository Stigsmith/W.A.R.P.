<script lang="ts">
  // All mods, as a table you can edit many rows of at once. "Sets" mode shows a
  // column per set: click a box to put a mod in a set, drag down a column to tick
  // many, and with rows selected one click fills them all, like a spreadsheet.
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { settings } from "../lib/settings.svelte";
  import { count, date, notUpdatedHint, notUpdatedLabel } from "../lib/format";
  import ModPanel from "../components/ModPanel.svelte";
  import TierSelect from "../components/TierSelect.svelte";
  import type { LibraryEntry, Source, WorkshopId } from "../lib/types";

  let query = $state("");
  let tier = $state("");
  let set = $state("");
  let showUnsubscribed = $state(false);
  let onlyNotUpdated = $state(false);
  /** Installed mods kept up to date until the latest game update but not since. */
  const notUpdated = $derived(app.library.filter((e) => e.subscribed && e.not_updated));
  const mode = $derived(settings.libraryMode);
  let openId = $state<string | null>(null);

  const opened = $derived(openId ? app.byId.get(openId) : undefined);
  const priority = (key: string) => app.tier(key)?.priority ?? -1;

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return app.library
      .filter((e) => showUnsubscribed || e.subscribed)
      .filter((e) => !onlyNotUpdated || e.not_updated)
      .filter((e) => !tier || e.knowledge.tier === tier)
      .filter((e) => !set || (set === "__none" ? e.sets.length === 0 : e.sets.includes(set)))
      .filter((e) => !q || `${e.info.title} ${e.packs.join(" ")} ${e.knowledge.tags.join(" ")}`.toLowerCase().includes(q))
      .sort((a, b) => sort.dir * compare(sort.key, a, b) || byTier(a, b) || a.info.title.localeCompare(b.info.title));
  });

  // --- Sorting: click a column's header; click again to reverse. ------------------
  type SortKey = "title" | "tier" | "from" | "role" | "sets" | "updated";
  let sort = $state<{ key: SortKey; dir: 1 | -1 }>({ key: "tier", dir: 1 });

  // Guesses first: they're the ones worth a look.
  const fromOrder: Record<Source, number> = { heuristic: 0, default: 1, user: 2, community: 3 };
  const byTier = (a: LibraryEntry, b: LibraryEntry) => priority(b.knowledge.tier) - priority(a.knowledge.tier);

  function compare(key: SortKey, a: LibraryEntry, b: LibraryEntry): number {
    switch (key) {
      case "title":
        return (a.info.title || a.packs[0] || "").localeCompare(b.info.title || b.packs[0] || "");
      case "tier":
        return byTier(a, b);
      case "from":
        return fromOrder[a.knowledge.tier_source] - fromOrder[b.knowledge.tier_source];
      case "role":
        return (app.role(b.knowledge.role)?.priority ?? 0) - (app.role(a.knowledge.role)?.priority ?? 0);
      case "sets":
        // Mods in no set go last.
        return (a.sets.length ? 0 : 1) - (b.sets.length ? 0 : 1) || a.sets.join(", ").localeCompare(b.sets.join(", "));
      case "updated":
        return b.info.time_updated - a.info.time_updated;
    }
  }

  function sortBy(key: SortKey) {
    sort = { key, dir: sort.key === key ? (-sort.dir as 1 | -1) : 1 };
  }

  const arrow = (key: SortKey) => (sort.key === key ? (sort.dir === 1 ? "▾" : "▴") : "");

  const sourceMark: Record<Source, string> = { user: "you", community: "", heuristic: "guess", default: "?" };
  const sourceName: Record<Source, string> = { user: "you", community: "community", heuristic: "guess", default: "default" };

  /** Where a mod's tier came from, in words; for guesses, why. */
  function fromTitle(e: LibraryEntry): string {
    switch (e.knowledge.tier_source) {
      case "heuristic":
        return `Guessed because ${e.guess_why ?? "of its Steam tags"}. Pick a tier to set it yourself.`;
      case "community":
        return "From the community's list of sorted mods";
      case "user":
        return "Your own choice. Pick the community's tier again to undo it.";
      case "default":
        return "Nobody has sorted this mod and W.A.R.P. couldn't guess, so it sits at the bottom";
    }
  }

  // --- Selection: tick boxes, shift-click for a range. ---------------------------
  const selection = new SvelteSet<WorkshopId>();
  let anchor: WorkshopId | null = null;
  const shownIds = $derived(rows.map((e) => e.info.id));
  const allShownSelected = $derived(shownIds.length > 0 && shownIds.every((id) => selection.has(id)));

  function select(e: MouseEvent, id: WorkshopId) {
    const on = !selection.has(id);
    const from = anchor ? shownIds.indexOf(anchor) : -1;
    const to = shownIds.indexOf(id);
    const range = e.shiftKey && from >= 0 ? shownIds.slice(Math.min(from, to), Math.max(from, to) + 1) : [id];
    for (const x of range) on ? selection.add(x) : selection.delete(x);
    anchor = id;
  }

  function selectAllShown() {
    if (allShownSelected) for (const id of shownIds) selection.delete(id);
    else for (const id of shownIds) selection.add(id);
  }

  // --- Set membership, with the change shown before the backend confirms it. -----
  const memberOf = $derived(new Map(app.sets.map((s) => [s.name, new Set(s.members)])));

  async function editSet(name: string, add: WorkshopId[], remove: WorkshopId[]) {
    if (!add.length && !remove.length) return;
    const s = app.sets.find((x) => x.name === name);
    if (s) s.members = [...new Set([...s.members, ...add])].filter((id) => !remove.includes(id));
    const ok = await app.run(async () => (await api()).editSet(name, add, remove));
    await app.refresh();
    if (!ok) return;
    const asking = Object.values(app.setUpdates).filter((us) => us.some((u) => u.set === name)).length;
    app.notify(
      (add.length ? `Put ${count(add.length, "mod")} in ${name}` : `Took ${count(remove.length, "mod")} out of ${name}`) +
        (asking ? `. ${count(asking, "profile")} using it will ask before taking the change.` : ""),
    );
  }

  // Painting: press on a box, drag along the column, let go to save.
  let paint = $state<{ set: string; value: boolean; ids: SvelteSet<WorkshopId> } | null>(null);

  const isMember = (name: string, id: WorkshopId) =>
    paint && paint.set === name && paint.ids.has(id) ? paint.value : (memberOf.get(name)?.has(id) ?? false);

  function startPaint(e: PointerEvent, name: string, id: WorkshopId) {
    if (e.button !== 0) return;
    e.preventDefault();
    const ids = new SvelteSet<WorkshopId>([id]);
    // A box in a selected row fills the whole selection.
    if (selection.has(id)) for (const x of selection) ids.add(x);
    paint = { set: name, value: !(memberOf.get(name)?.has(id) ?? false), ids };
  }

  function extendPaint(name: string, id: WorkshopId) {
    if (paint?.set === name) paint.ids.add(id);
  }

  async function finishPaint() {
    if (!paint) return;
    const { set: name, value, ids } = paint;
    const members = memberOf.get(name) ?? new Set();
    const changed = [...ids].filter((id) => members.has(id) !== value);
    await editSet(name, value ? changed : [], value ? [] : changed);
    paint = null;
  }

  onMount(() => {
    const onUp = () => finishPaint();
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (menuFor) menuFor = null;
      else if (!(e.target instanceof HTMLInputElement)) selection.clear();
    };
    // A click anywhere outside the open set menu (or its header) closes it.
    const onDown = (e: PointerEvent) => {
      if (menuFor && !(e.target as Element).closest?.("th.setcol.open")) menuFor = null;
    };
    window.addEventListener("pointerup", onUp);
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onDown, true);
    return () => {
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onDown, true);
    };
  });

  // --- Bulk actions on the selection. -------------------------------------------
  async function addSelected(name: string) {
    const members = memberOf.get(name) ?? new Set();
    await editSet(name, [...selection].filter((id) => !members.has(id)), []);
  }

  async function removeSelected(name: string) {
    const members = memberOf.get(name) ?? new Set();
    await editSet(name, [], [...selection].filter((id) => members.has(id)));
  }

  /** Moves mods to a tier as the user's own choice (the selection, unless given). */
  async function setTier(key: string, ids: WorkshopId[] = [...selection]) {
    const a = await api();
    const ok = await app.run(async () => {
      for (const id of ids) {
        const e = app.byId.get(id);
        if (e) await a.setKnowledge(id, { ...(e.user ?? {}), tier: key });
      }
    });
    await app.refresh();
    if (ok) app.notify(`${ids.length === 1 ? app.title(ids[0]) : count(ids.length, "mod")} now in ${app.tier(key)?.name ?? key}`);
  }

  /** A row's tier dropdown; on a selected row it moves the whole selection. */
  function rowTier(id: WorkshopId, key: string) {
    setTier(key, selection.has(id) ? [...selection] : [id]);
  }

  // --- Making, renaming and deleting sets. ----------------------------------------
  let newSet = $state<string | null>(null);
  let menuFor = $state<string | null>(null);
  /** The menu opens leftwards when the column is too close to the window's right edge. */
  let menuLeftwards = $state(false);
  let renameTo = $state("");

  async function createSet() {
    const name = newSet?.trim();
    if (!name) return;
    const members = [...selection];
    if (await app.run(async () => (await api()).createSet(name, members))) {
      await app.refresh();
      app.notify(members.length ? `Made the set ${name} with ${count(members.length, "mod")}` : `Made the set ${name}. Tick its mods in the ${name} column.`);
      newSet = null;
      settings.set("libraryMode", "sets");
    }
  }

  async function renameSet(from: string) {
    const to = renameTo.trim();
    if (!to || to === from) return (menuFor = null);
    if (await app.run(async () => (await api()).renameSet(from, to))) {
      if (set === from) set = to;
      await app.refresh();
      app.notify(`Renamed ${from} to ${to}`);
    }
    menuFor = null;
  }

  async function deleteSet(name: string) {
    menuFor = null;
    const a = await api();
    const using = app.profiles.filter((p) => p.sets.some((s) => s.toLowerCase() === name.toLowerCase()));
    const ok = await a.confirm(
      `Delete the set "${name}"? Its mods stay installed.` +
        (using.length ? `\n\nProfiles using it (${using.map((p) => p.name).join(", ")}) keep their copy and will ask what to do.` : ""),
    );
    if (!ok) return;
    if (await app.run(() => a.deleteSet(name))) {
      if (set === name) set = "";
      await app.refresh();
      app.notify(`Deleted the set ${name}`);
    }
  }

  function openMenu(name: string, header: HTMLElement) {
    menuFor = menuFor === name ? null : name;
    renameTo = name;
    // Bring a column that's scrolled half out of view in first, then pick a side.
    header.scrollIntoView({ block: "nearest", inline: "nearest" });
    menuLeftwards = header.getBoundingClientRect().left + 250 > window.innerWidth - 12;
  }

  // --- Filters. ---------------------------------------------------------------------
  const filtered = $derived(!!(query.trim() || tier || set || onlyNotUpdated));

  function showAll() {
    query = "";
    tier = "";
    set = "";
    onlyNotUpdated = false;
  }
</script>

<div class="library" class:with-panel={!!opened}>
  <div class="body">
    <header>
      <div class="title-row">
        <h1>Library</h1>
        <div class="modes" role="tablist" data-tour="sets-mode">
          <button role="tab" class:active={mode === "details"} aria-selected={mode === "details"} onclick={() => settings.set("libraryMode", "details")}>Details</button>
          <button role="tab" class:active={mode === "sets"} aria-selected={mode === "sets"} onclick={() => settings.set("libraryMode", "sets")}>Sets</button>
        </div>
        <span class="spacer"></span>
        {#if newSet === null}
          <button class="small" onclick={() => (newSet = "")}>+ New set{selection.size ? ` from ${selection.size} selected` : ""}</button>
        {:else}
          <form
            class="new-set"
            onsubmit={(e) => {
              e.preventDefault();
              createSet();
            }}
          >
            <!-- svelte-ignore a11y_autofocus -->
            <input
              bind:value={newSet}
              placeholder={selection.size ? `Name for ${count(selection.size, "mod")}…` : "Set name, e.g. Skaven campaign"}
              autofocus
              onkeydown={(e) => e.key === "Escape" && (newSet = null)}
            />
            <button class="primary small" type="submit">Make set</button>
            <button class="ghost small" type="button" onclick={() => (newSet = null)}>Cancel</button>
          </form>
        {/if}
      </div>
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
        {#if notUpdated.length}
          <label class="check" title="Mods kept up to date for the previous game update, but not updated since the latest. Likely suspects if the game crashes, until their authors catch up.">
            <input type="checkbox" bind:checked={onlyNotUpdated} /> {notUpdatedLabel(notUpdated[0]).replace(/^n/, "N")} ({notUpdated.length})
          </label>
        {/if}
        {#if filtered}<button class="small" onclick={showAll}>✕ Show all mods</button>{/if}
        <span class="faint count">{rows.length} shown</span>
      </div>

      {#if selection.size}
        <div class="bulk">
          <span class="picked">{count(selection.size, "mod")} selected</span>
          <select onchange={(e) => (addSelected(e.currentTarget.value), (e.currentTarget.value = ""))} disabled={!app.sets.length}>
            <option value="">Put in set…</option>
            {#each app.sets as s (s.name)}<option value={s.name}>{s.name}</option>{/each}
          </select>
          <select onchange={(e) => (removeSelected(e.currentTarget.value), (e.currentTarget.value = ""))} disabled={!app.sets.length}>
            <option value="">Take out of set…</option>
            {#each app.sets as s (s.name)}<option value={s.name}>{s.name}</option>{/each}
          </select>
          <select onchange={(e) => (setTier(e.currentTarget.value), (e.currentTarget.value = ""))}>
            <option value="">Move to tier…</option>
            {#each app.taxonomy.tier as t (t.key)}<option value={t.key}>{t.name}</option>{/each}
          </select>
          <span class="spacer"></span>
          <button class="ghost small" onclick={() => selection.clear()}>Clear selection</button>
        </div>
      {:else if mode === "sets"}
        <p class="hint faint">
          Click a box to put a mod in a set. Drag down a column to tick many at once. With rows selected, one click fills them
          all. Click a set's name to rename or delete it.
        </p>
      {/if}
    </header>

    <div class="table scroll">
      <table class:painting={!!paint} class:sets-mode={mode === "sets"}>
        <thead>
          <tr>
            <th class="pick">
              <input type="checkbox" checked={allShownSelected} onchange={selectAllShown} aria-label="Select all shown" />
            </th>
            <th class="mod-col"><button class="sort" onclick={() => sortBy("title")}>Mod {arrow("title")}</button></th>
            <th data-tour="tier-col"><button class="sort" onclick={() => sortBy("tier")}>Tier {arrow("tier")}</button></th>
            {#if mode === "details"}
              <th><button class="sort" onclick={() => sortBy("from")} title="Where the tier comes from: the community's list, a guess, or you">From {arrow("from")}</button></th>
              <th><button class="sort" onclick={() => sortBy("role")}>Role {arrow("role")}</button></th>
              <th><button class="sort" onclick={() => sortBy("sets")}>Sets {arrow("sets")}</button></th>
              <th class="right"><button class="sort" onclick={() => sortBy("updated")}>Updated {arrow("updated")}</button></th>
            {:else}
              {#each app.sets as s (s.name)}
                <th class="setcol" class:open={menuFor === s.name}>
                  <button class="set-name" onclick={(e) => openMenu(s.name, e.currentTarget)} title="{s.name}: {count(s.members.length, 'mod')}">
                    <span class="rot">{s.name}</span>
                    <span class="n">{s.members.length}</span>
                  </button>
                  {#if menuFor === s.name}
                    <div class="menu card forged" class:leftwards={menuLeftwards}>
                      <form
                        onsubmit={(e) => {
                          e.preventDefault();
                          renameSet(s.name);
                        }}
                      >
                        <!-- svelte-ignore a11y_autofocus -->
                        <input bind:value={renameTo} aria-label="Set name" autofocus onkeydown={(e) => e.key === "Escape" && (menuFor = null)} />
                        <button class="small" type="submit">Rename</button>
                      </form>
                      {#if selection.size}
                        <button class="small" onclick={() => (addSelected(s.name), (menuFor = null))}>Put {count(selection.size, "selected mod")} in</button>
                        <button class="small" onclick={() => (removeSelected(s.name), (menuFor = null))}>Take {count(selection.size, "selected mod")} out</button>
                      {/if}
                      <button class="small" onclick={() => ((set = s.name), (menuFor = null))}>Show only its mods</button>
                      <button class="small danger" onclick={() => deleteSet(s.name)}>Delete set</button>
                    </div>
                  {/if}
                </th>
              {:else}
                <th class="muted">No sets yet: make one with “+ New set”.</th>
              {/each}
            {/if}
          </tr>
        </thead>
        <tbody>
          {#each rows as e (e.info.id)}
            {@const id = e.info.id}
            <tr class:active={id === openId} class:picked={selection.has(id)} class:off={!e.subscribed} onclick={() => (openId = id)}>
              <td class="pick" onclick={(ev) => ev.stopPropagation()}>
                <input type="checkbox" checked={selection.has(id)} onclick={(ev) => select(ev, id)} aria-label="Select {e.info.title}" />
              </td>
              <td class="mod">
                <span class="title">
                  <span class="name">{e.info.title || e.packs[0]}</span>
                  {#if e.subscribed && e.installed_version !== null && e.installed_version < e.info.time_updated}
                    <span class="chip warn" title="Steam has a newer version than the one installed">update pending</span>
                  {/if}
                  {#if e.not_updated}<span class="chip warn" title={notUpdatedHint(e)}>{notUpdatedLabel(e)}</span>{/if}
                </span>
                <span class="mono faint">{e.packs[0] ?? "—"}</span>
              </td>
              <td class="tier-cell" onclick={(ev) => ev.stopPropagation()}>
                <TierSelect
                  value={e.knowledge.tier}
                  onpick={(key) => rowTier(id, key)}
                  label="Tier of {e.info.title}"
                  note={selection.has(id) && selection.size > 1 ? `changes all ${selection.size} selected mods` : ""}
                />
                {#if mode === "sets" && sourceMark[e.knowledge.tier_source]}
                  <span class="src" title={fromTitle(e)}>{sourceMark[e.knowledge.tier_source]}</span>
                {/if}
              </td>
              {#if mode === "details"}
                <td class="from {e.knowledge.tier_source}" title={fromTitle(e)}>{sourceName[e.knowledge.tier_source]}</td>
                <td class="muted">{app.role(e.knowledge.role)?.name ?? e.knowledge.role}</td>
                <td class="sets muted">{e.sets.join(", ")}</td>
                <td class="right faint">{date(e.info.time_updated)}</td>
              {:else}
                {#each app.sets as s (s.name)}
                  {@const on = isMember(s.name, id)}
                  <td
                    class="cell"
                    role="checkbox"
                    aria-checked={on}
                    aria-label="{e.info.title} in {s.name}"
                    tabindex="-1"
                    onclick={(ev) => ev.stopPropagation()}
                    onpointerdown={(ev) => startPaint(ev, s.name, id)}
                    onpointerenter={() => extendPaint(s.name, id)}
                  >
                    <span class="box" class:on></span>
                  </td>
                {/each}
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
      {#if rows.length === 0}
        <div class="empty">
          {#if set && set !== "__none" && !query.trim() && !tier}
            <p>Nothing in {set} yet. Switch to Sets and tick mods into its column.</p>
          {:else}
            <p>No mods match.</p>
          {/if}
          {#if filtered}<button onclick={showAll}>Show all mods</button>{/if}
        </div>
      {/if}
    </div>
  </div>

  {#if opened}
    <ModPanel entry={opened} onclose={() => (openId = null)} />
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

  .title-row {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .spacer {
    flex: 1;
  }

  .modes {
    display: flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .modes button {
    border: 0;
    background: transparent;
    padding: 4px 14px;
    font-size: 13px;
    color: var(--muted);
  }

  .modes button.active {
    background: var(--ok-dim);
    color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent-dim), var(--glow-sm);
  }

  .new-set {
    display: flex;
    gap: 6px;
  }

  .new-set input {
    width: 260px;
  }

  .filters,
  .bulk {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }

  .bulk {
    padding: 8px 12px;
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius);
    background: var(--ok-dim);
    box-shadow: var(--glow-sm);
  }

  .picked {
    color: var(--accent);
    font-weight: 600;
    font-size: 13px;
    margin-right: 4px;
  }

  .hint {
    margin: 0;
    font-size: 12.5px;
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

  table.painting {
    user-select: none;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--bg);
    text-align: left;
    vertical-align: bottom;
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

  .pick {
    width: 30px;
    padding-right: 0;
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

  tr.picked {
    background: rgb(168 242 63 / 0.06);
  }

  tr.off {
    opacity: 0.55;
  }

  /* The mod column takes whatever the others leave, and cuts long names short. */
  .mod {
    width: 100%;
    max-width: 0;
  }

  th.mod-col {
    min-width: 220px;
  }

  .mod > span {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The title, then its chips; a long title is cut short, the chips stay whole. */
  .mod .title {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .mod .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mod .title .chip {
    flex-shrink: 0;
  }

  .tier-cell {
    white-space: nowrap;
  }



  td.muted {
    white-space: nowrap;
  }

  .src {
    margin-left: 6px;
    font-size: 11px;
    color: var(--info);
    cursor: help;
  }

  /* Header labels are buttons: click to sort, again to reverse. */
  .sort {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: inherit;
    cursor: pointer;
    white-space: nowrap;
  }

  .sort:hover {
    color: var(--accent);
  }

  td.from {
    font-size: 12.5px;
    white-space: nowrap;
    cursor: help;
    color: var(--faint);
  }

  td.from.heuristic {
    color: var(--info);
  }

  td.from.user {
    color: var(--accent);
  }

  td.from.default {
    color: var(--warn);
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

  /* Set columns: names written upwards so a dozen sets fit side by side. */
  th.setcol {
    width: 34px;
    padding: 6px 2px;
    text-align: center;
    text-transform: none;
    letter-spacing: 0.02em;
  }

  .set-name {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 30px;
    padding: 6px 0 4px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
  }

  .set-name:hover,
  th.open .set-name {
    color: var(--accent);
    border-color: var(--accent-dim);
    box-shadow: var(--glow-sm);
  }

  .rot {
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    max-height: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .n {
    font-size: 10.5px;
    color: var(--faint);
    font-weight: 400;
  }

  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 5;
    width: 230px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    background: rgb(12 16 10 / 0.97);
  }

  .menu.leftwards {
    left: auto;
    right: 0;
  }

  .menu form {
    display: flex;
    gap: 6px;
  }

  .menu input {
    flex: 1;
    min-width: 0;
  }

  .menu .danger {
    color: var(--danger);
  }

  td.cell {
    padding: 0;
    text-align: center;
    cursor: cell;
  }

  td.cell:hover .box {
    border-color: var(--accent);
  }

  .box {
    display: inline-block;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    border: 1px solid var(--border-strong);
    background: rgb(0 0 0 / 0.25);
    vertical-align: middle;
    transition:
      background 0.1s,
      box-shadow 0.1s;
  }

  /* Ticked: a warpstone core behind frosted glass. */
  .box.on {
    border-color: var(--warpstone-rim);
    background:
      radial-gradient(circle, rgb(190 255 90 / 0.75) 0 3px, rgb(168 242 63 / 0) 6px),
      var(--warpstone-glass);
    box-shadow:
      var(--warpstone-inner),
      0 0 8px rgb(140 255 50 / 0.45);
  }
</style>
