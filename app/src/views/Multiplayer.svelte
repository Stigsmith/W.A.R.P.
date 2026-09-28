<script lang="ts">
  import { api, isDesktop } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { count, date, steamClientUrl } from "../lib/format";
  import type { CompareResult, ShareList, VersionMismatch, WorkshopId } from "../lib/types";

  const LIST_FILES = [{ name: "Modlist (.warp or Kaedrin profile)", extensions: ["warp", "txt"] }];

  let mineProfile = $state(app.profiles[0]?.name ?? "");
  let mineFile = $state<string | null>(null);
  let theirCode = $state("");
  let theirFile = $state<string | null>(null);
  let checkSteam = $state(true);
  let busy = $state(false);
  let result = $state<CompareResult | null>(null);

  const canCompare = $derived((mineFile || mineProfile) && (theirFile || theirCode.trim()));

  async function pick(side: "mine" | "theirs") {
    const path = await (await api()).pickFile(LIST_FILES);
    if (!path) return;
    if (side === "mine") mineFile = path;
    else {
      theirFile = path;
      theirCode = "";
    }
  }

  async function compare() {
    busy = true;
    result = null;
    const a = await api();
    result =
      (await app.attempt(async () => {
        const mine: ShareList = await a.loadList(mineFile ? { kind: "file", path: mineFile } : { kind: "profile", name: mineProfile });
        const theirs: ShareList = await a.loadList(theirFile ? { kind: "file", path: theirFile } : { kind: "code", text: theirCode });
        return a.compare(mine, theirs, checkSteam);
      })) ?? null;
    busy = false;
  }

  async function demo() {
    if (!import.meta.env.DEV) return;
    const { mockFriendCode } = await import("../lib/mock/mock");
    theirCode = await mockFriendCode();
    theirFile = null;
  }

  async function open(id: WorkshopId | null) {
    if (id) await (await api()).openUrl(steamClientUrl(id));
  }

  async function useTheirs() {
    if (!result) return;
    const a = await api();
    const name = `MP - ${result.b.name || "friend"}`;
    const missing = result.diff.only_in_b.length;
    const ok = await a.confirm(
      `Save their exact list as the Kaedrin profile "${name}"?` +
        (missing ? `\n\n${count(missing, "mod")} you don't have yet won't load until you subscribe.` : ""),
    );
    if (!ok) return;
    const packs = result.b.entries.map((e) => e.pack).filter(Boolean);
    const path = await app.attempt(() => a.exportKaedrin(packs, name));
    if (path) app.notify(`Saved to ${path}`);
  }

  const STALE: Record<NonNullable<VersionMismatch["stale"]>, string> = {
    a: "You have an older copy",
    b: "They have an older copy",
    both: "Neither matches Steam's current version",
  };
  const staleText = (v: VersionMismatch) => (v.stale ? STALE[v.stale] : "Can't tell who's behind");

  const label = (id: WorkshopId | null, pack: string) => (id && result?.titles[id]) || app.title(id, pack);
  /** The pack file, when it says something the title doesn't. */
  const detail = (id: WorkshopId | null, pack: string) => (pack && pack !== label(id, pack) && !pack.startsWith("workshop item") ? pack : "");
  const fixes = $derived(
    result ? result.diff.only_in_a.length + result.diff.only_in_b.length + result.diff.moves.length + result.diff.version_mismatches.length : 0,
  );
</script>

<div class="mp scroll">
  <header>
    <h1>Multiplayer</h1>
    <p class="muted">Both players need the same mods, in the same order, at the same version. Paste your friend's code to see exactly what differs.</p>
  </header>

  <div class="inputs">
    <div class="card side">
      <span class="label">Your list</span>
      {#if mineFile}
        <div class="file">
          <span class="mono">{mineFile}</span>
          <button class="ghost small" onclick={() => (mineFile = null)}>Use a profile</button>
        </div>
      {:else}
        <select bind:value={mineProfile}>
          {#each app.profiles as p (p.name)}
            <option value={p.name}>{p.name}</option>
          {/each}
        </select>
        <button class="ghost small" onclick={() => pick("mine")}>…or open a file</button>
      {/if}
    </div>

    <div class="vs">vs</div>

    <div class="card side">
      <span class="label">Their list</span>
      {#if theirFile}
        <div class="file">
          <span class="mono">{theirFile}</span>
          <button class="ghost small" onclick={() => (theirFile = null)}>Paste a code instead</button>
        </div>
      {:else}
        <textarea bind:value={theirCode} rows="3" placeholder="Paste their WARP code, or the whole Discord message"></textarea>
        <div class="row">
          <button class="ghost small" onclick={() => pick("theirs")}>…or open a file</button>
          {#if import.meta.env.DEV && !isDesktop}<button class="ghost small" onclick={demo}>Use demo code</button>{/if}
        </div>
      {/if}
    </div>
  </div>

  <div class="go">
    <label class="check"><input type="checkbox" bind:checked={checkSteam} /> Ask Steam who has an outdated copy</label>
    <button class="primary" onclick={compare} disabled={!canCompare || busy}>{busy ? "Comparing…" : "Compare"}</button>
  </div>

  {#if result}
    {@const d = result.diff}
    {#if d.identical}
      <div class="verdict ok">
        <strong>You're in sync.</strong>
        <span>All {d.common} packs match, in the same order, at the same version. Good to play.</span>
      </div>
    {:else}
      <div class="verdict bad">
        <strong>Not in sync yet: {count(fixes, "thing")} to fix</strong>
        <span class="counts">
          {#if d.only_in_b.length}<span class="chip danger">{d.only_in_b.length} missing</span>{/if}
          {#if d.only_in_a.length}<span class="chip warn">{d.only_in_a.length} extra</span>{/if}
          {#if d.moves.length}<span class="chip info">{count(d.moves.length, "move")}</span>{/if}
          {#if d.version_mismatches.length}<span class="chip warn">{d.version_mismatches.length} version</span>{/if}
          <span class="faint">{d.common} packs already match</span>
        </span>
        <button onclick={useTheirs}>Use their list</button>
      </div>

      <div class="results">
        {#if d.only_in_b.length}
          <section class="card">
            <h2>You're missing {count(d.only_in_b.length, "mod")}</h2>
            <p class="muted">They run these; you don't. Subscribe in Steam.</p>
            <ul>
              {#each d.only_in_b as e (e.position)}
                <li>
                  <span class="names"><span>{label(e.workshop_id, e.pack)}</span><span class="mono faint">{detail(e.workshop_id, e.pack)}</span></span>
                  {#if e.workshop_id}<button class="small" onclick={() => open(e.workshop_id)}>Open in Steam</button>{/if}
                </li>
              {/each}
            </ul>
          </section>
        {/if}

        {#if d.only_in_a.length}
          <section class="card">
            <h2>Only you have {count(d.only_in_a.length, "mod")}</h2>
            <p class="muted">Turn these off for this session, or ask them to add them.</p>
            <ul>
              {#each d.only_in_a as e (e.position)}
                <li><span class="names"><span>{label(e.workshop_id, e.pack)}</span><span class="mono faint">{detail(e.workshop_id, e.pack)}</span></span></li>
              {/each}
            </ul>
          </section>
        {/if}

        {#if d.moves.length}
          <section class="card">
            <h2>Fix the order: {count(d.moves.length, "move")}</h2>
            <p class="muted">The fewest moves that make your order match theirs. Do them top to bottom.</p>
            <ol class="moves">
              {#each d.moves as m, i (i)}
                <li>
                  Move <strong class="mono">{m.pack}</strong>
                  {#if m.below}directly below <strong class="mono">{m.below}</strong>{:else}to the very top{/if}
                </li>
              {/each}
            </ol>
          </section>
        {/if}

        {#if d.version_mismatches.length}
          <section class="card">
            <h2>Different versions of {count(d.version_mismatches.length, "mod")}</h2>
            <p class="muted">The outdated side should make Steam re-download it: unsubscribe and subscribe again.</p>
            <ul>
              {#each d.version_mismatches as v (v.pack)}
                <li>
                  <span class="names">
                    <span>{label(v.workshop_id, v.pack)}</span>
                    <span class="faint">
                      {staleText(v)}{#if v.a_time_updated && v.b_time_updated}&nbsp;· yours {date(v.a_time_updated)}, theirs {date(v.b_time_updated)}{/if}
                    </span>
                  </span>
                  {#if v.workshop_id}<button class="small" onclick={() => open(v.workshop_id)}>Open in Steam</button>{/if}
                </li>
              {/each}
            </ul>
          </section>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .mp {
    height: 100%;
    padding: 22px 26px 40px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  header p {
    margin-top: 4px;
    max-width: 640px;
  }

  .inputs {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 12px;
    align-items: stretch;
  }

  .side {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  .side select,
  .side textarea {
    width: 100%;
  }

  .row {
    display: flex;
    gap: 6px;
  }

  .file {
    display: flex;
    flex-direction: column;
    gap: 4px;
    word-break: break-all;
  }

  .vs {
    align-self: center;
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--faint);
  }

  .go {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 16px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--muted);
    font-size: 13px;
    cursor: pointer;
  }

  .verdict {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 16px;
    padding: 16px 18px;
    border-radius: var(--radius);
    border: 1px solid;
  }

  .verdict strong {
    font-family: var(--font-display);
    font-size: 17px;
  }

  .verdict.ok {
    border-color: var(--accent-dim);
    background: var(--ok-dim);
    box-shadow: 0 0 40px var(--accent-glow);
  }

  .verdict.ok strong {
    color: var(--accent);
  }

  .verdict.bad {
    border-color: rgb(240 101 82 / 0.35);
    background: var(--danger-dim);
  }

  .counts {
    display: flex;
    gap: 6px;
    align-items: center;
    flex: 1;
    flex-wrap: wrap;
    font-size: 12.5px;
  }

  .results {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 12px;
    align-items: start;
  }

  section {
    padding: 16px;
  }

  section > p {
    font-size: 13px;
    margin: 3px 0 10px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  ul li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 7px 0;
    border-top: 1px solid var(--border);
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

  .names .faint {
    font-size: 12px;
  }

  .moves {
    margin: 0;
    padding-left: 22px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .moves strong {
    color: var(--accent);
    font-weight: 500;
  }
</style>
