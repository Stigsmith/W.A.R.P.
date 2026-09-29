<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { date, notUpdatedHint, notUpdatedLabel, size, steamClientUrl } from "../lib/format";
  import TierSelect from "./TierSelect.svelte";
  import type { LibraryEntry, ModKnowledge, Source, WorkshopId } from "../lib/types";

  let { entry, onclose }: { entry: LibraryEntry; onclose: () => void } = $props();

  let showAll = $state(false);
  const k = $derived(entry.knowledge);

  const sourceText: Record<Source, string> = {
    user: "your choice",
    community: "community",
    heuristic: "guessed",
    default: "default",
  };

  async function setField(field: "tier" | "role", value: string) {
    const next: ModKnowledge = { ...(entry.user ?? {}), [field]: value };
    await app.attempt(async () => (await api()).setKnowledge(entry.info.id, next));
    await app.refresh();
  }

  async function resetToCommunity() {
    await app.attempt(async () => (await api()).setKnowledge(entry.info.id, {}));
    await app.refresh();
  }

  async function toggleSet(name: string, on: boolean) {
    const set = app.sets.find((s) => s.name === name);
    if (!set) return;
    const members = on ? [...set.members, entry.info.id] : set.members.filter((m) => m !== entry.info.id);
    await app.attempt(async () => (await api()).saveSet({ name, members }));
    await app.refresh();
  }

  async function openSteam(id: WorkshopId) {
    await (await api()).openUrl(steamClientUrl(id));
  }
</script>

<aside class="panel scroll">
  <div class="top">
    <button class="ghost small close" onclick={onclose} aria-label="Close">✕</button>
    {#if entry.info.preview_url}
      <img src={entry.info.preview_url} alt="" loading="lazy" />
    {/if}
    <h2>{entry.info.title || entry.packs[0] || entry.info.id}</h2>
    <div class="meta faint mono">{entry.packs.join(", ") || "no pack known"}</div>
    <div class="links">
      <button class="small" onclick={() => openSteam(entry.info.id)}>Open in Steam</button>
      {#if !entry.subscribed}<span class="chip warn">not installed</span>{/if}
      {#if entry.subscribed && entry.installed_version !== null && entry.installed_version < entry.info.time_updated}
        <span class="chip warn" title="Steam has a newer version than the one on disk. In multiplayer, both players need the same one.">update pending</span>
      {/if}
      {#if entry.not_updated}<span class="chip warn" title={notUpdatedHint(entry)}>{notUpdatedLabel(entry)}</span>{/if}
      {#if !entry.info.available}<span class="chip danger">removed from Workshop</span>{/if}
    </div>
  </div>

  <section>
    <span class="label">Where it goes</span>
    <div class="field">
      <span class="field-label">Tier</span>
      <span><TierSelect value={k.tier} onpick={(key) => setField("tier", key)} /></span>
      <span class="chip" class:accent={k.tier_source === "user"}>{sourceText[k.tier_source]}</span>
    </div>
    <div class="field">
      <label for="role">Role</label>
      <select id="role" value={k.role} onchange={(e) => setField("role", e.currentTarget.value)}>
        {#each app.taxonomy.role as r (r.key)}<option value={r.key}>{r.name}</option>{/each}
      </select>
      <span class="chip" class:accent={k.role_source === "user"}>{sourceText[k.role_source]}</span>
    </div>
    {#if k.tier_source === "heuristic"}
      <p class="hint guess">
        Nobody has sorted this mod yet, so W.A.R.P. guessed{entry.guess_why ? `: ${entry.guess_why}` : " from its Steam tags"}. Pick a
        tier to set it yourself.
      </p>
    {/if}
    <p class="hint faint">{app.tier(k.tier)?.description}</p>
    {#if entry.user && (entry.user.tier || entry.user.role)}
      <button class="ghost small" onclick={resetToCommunity}>Reset to community values</button>
    {/if}
  </section>

  {#if k.requires.length || k.patches.length || k.incompatible_with.length}
    <section>
      <span class="label">Relations</span>
      {#each [["Requires", k.requires], ["Patches", k.patches], ["Incompatible with", k.incompatible_with]] as [name, ids] (name)}
        {#if (ids as string[]).length}
          <div class="rel">
            <span class="muted">{name}</span>
            {#each ids as string[] as id (id)}
              <span>{app.title(id)}</span>
            {/each}
          </div>
        {/if}
      {/each}
    </section>
  {/if}

  <section>
    <span class="label">Sets</span>
    <div class="sets">
      {#each app.sets as s (s.name)}
        <label class="check">
          <input type="checkbox" checked={entry.sets.includes(s.name)} onchange={(e) => toggleSet(s.name, e.currentTarget.checked)} />
          {s.name}
        </label>
      {/each}
    </div>
  </section>

  {#if k.tags.length || k.factions.length}
    <section>
      <span class="label">Tags</span>
      <div class="chips">
        {#each k.tags as t (t)}<span class="chip">{t}</span>{/each}
        {#each k.factions as f (f)}<span class="chip info">{f}</span>{/each}
      </div>
    </section>
  {/if}

  <section>
    <span class="label">About</span>
    <div class="facts faint">
      <span>Updated {date(entry.info.time_updated)}</span>
      {#if entry.installed_version !== null && entry.installed_version !== entry.info.time_updated}
        <span>installed copy {date(entry.installed_version)}</span>
      {/if}
      <span>{size(entry.info.file_size)}</span>
      {#if entry.files}<span>{entry.files.toLocaleString()} files</span>{/if}
      <span>{entry.info.subscriptions.toLocaleString()} subscribers</span>
    </div>
    {#if entry.info.description}
      <p class="desc" class:clamp={!showAll}>{entry.info.description}</p>
      {#if entry.info.description.length > 400}
        <button class="ghost small" onclick={() => (showAll = !showAll)}>{showAll ? "Less" : "More"}</button>
      {/if}
    {/if}
  </section>
</aside>

<style>
  .panel {
    height: 100%;
    border-left: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
  }

  .top {
    position: relative;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .close {
    position: absolute;
    top: 10px;
    right: 10px;
    z-index: 1;
    background: rgb(0 0 0 / 0.5);
  }

  img {
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    margin-bottom: 6px;
  }

  .meta {
    word-break: break-all;
  }

  .links {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 4px;
  }

  section {
    padding: 14px 18px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field {
    display: grid;
    grid-template-columns: 48px 1fr auto;
    align-items: center;
    gap: 8px;
  }

  .field label,
  .field-label {
    color: var(--muted);
  }

  .hint {
    font-size: 12.5px;
  }

  .hint.guess {
    color: var(--info);
  }

  .rel {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
  }

  .sets {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 12px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    cursor: pointer;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 12.5px;
  }

  .desc {
    font-size: 13px;
    color: var(--muted);
    white-space: pre-line;
  }

  .desc.clamp {
    display: -webkit-box;
    -webkit-line-clamp: 8;
    line-clamp: 8;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
