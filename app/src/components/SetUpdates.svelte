<script lang="ts">
  // A profile keeps its own copy of each set. When a set changes, this notice asks
  // what to do: take the change, dismiss it, decide later, or stop asking.
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { count } from "../lib/format";
  import WarpShard from "./WarpShard.svelte";
  import type { ProfileDef, SetChanges, SetUpdate, WorkshopId } from "../lib/types";

  let { profile, updates }: { profile: ProfileDef; updates: SetUpdate[] } = $props();

  let open = $state(false);

  const summary = (u: SetUpdate) =>
    u.deleted ? "deleted" : [u.added.length && `+${u.added.length}`, u.removed.length && `−${u.removed.length}`].filter(Boolean).join(" ");

  const names = (ids: WorkshopId[], max = 8) => {
    const titles = ids.map((id) => app.title(id));
    return titles.length > max ? `${titles.slice(0, max).join(", ")} and ${titles.length - max} more` : titles.join(", ");
  };

  async function take(sets: string[]) {
    if (await app.run(async () => (await api()).applySetUpdates(profile.name, sets))) {
      await app.refresh();
      app.notify(sets.length === 1 ? `${profile.name} now has the new ${sets[0]}` : `${profile.name} now has the new versions of its sets`);
    }
  }

  async function dismiss(sets: string[]) {
    if (await app.run(async () => (await api()).dismissSetUpdates(profile.name, sets))) await app.refresh();
  }

  async function setPolicy(set_changes: SetChanges) {
    const next = { ...$state.snapshot(profile), set_changes };
    if (await app.run(async () => (await api()).saveProfile(next))) {
      await app.refresh();
      app.notify(set_changes === "follow" ? `${profile.name} now takes set changes automatically` : `${profile.name} won't ask about set changes any more`);
    }
  }
</script>

<div class="notice" class:open>
  <div class="bar">
    <WarpShard size={18} pulse />
    <span class="text">
      <strong>{updates.length === 1 ? "A set changed" : `${updates.length} sets changed`}</strong> since this profile took
      {updates.length === 1 ? "it" : "them"}:
      {#each updates as u, i (u.set)}{i ? " · " : ""}<span class="set">{u.set}</span> <span class="delta">{summary(u)}</span>{/each}
    </span>
    {#if open}
      <button class="ghost small" onclick={() => (open = false)}>Later</button>
    {:else}
      <button class="small" onclick={() => (open = true)}>Review</button>
      <button class="primary small" onclick={() => take([])}>Update</button>
    {/if}
  </div>

  {#if open}
    <ul class="changes">
      {#each updates as u (u.set)}
        <li>
          <div class="what">
            <span class="set">{u.set}</span>
            {#if u.deleted}
              <p class="muted">
                This set was deleted. Updating takes it out of the profile, with its {count(u.removed.length, "mod")} (unless you
                picked them separately).
              </p>
            {:else}
              {#if u.added.length}<p><span class="plus">+{u.added.length}</span> {names(u.added)}</p>{/if}
              {#if u.removed.length}<p><span class="minus">−{u.removed.length}</span> {names(u.removed)}</p>{/if}
            {/if}
          </div>
          <div class="buttons">
            <button class="primary small" onclick={() => take([u.set])}>{u.deleted ? "Take it out" : "Update"}</button>
            <button class="ghost small" onclick={() => dismiss([u.set])} title="Keep the profile as it is and hide this change. It shows again if the set changes again.">
              Dismiss
            </button>
          </div>
        </li>
      {/each}
    </ul>
    <div class="foot">
      <button class="small" onclick={() => dismiss([])}>Dismiss all</button>
      <span class="spacer"></span>
      <button class="ghost small" onclick={() => setPolicy("follow")}>Always update this profile automatically</button>
      <button class="ghost small" onclick={() => setPolicy("ignore")}>Stop asking for this profile</button>
    </div>
  {/if}
</div>

<style>
  .notice {
    border: 1px solid var(--brass-dim);
    border-left: 3px solid var(--brass);
    border-radius: var(--radius-sm);
    background: var(--brass-faint);
    box-shadow: 0 0 14px rgb(207 159 77 / 0.12);
    font-size: 13px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 12px;
  }

  .text {
    flex: 1;
    color: #f0dcb4;
  }

  .set {
    color: var(--brass);
    font-weight: 600;
  }

  .delta {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .changes {
    list-style: none;
    margin: 0;
    padding: 0 12px;
    max-height: 240px;
    overflow: auto;
  }

  .changes li {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 9px 0;
    border-top: 1px solid var(--brass-dim);
  }

  .what {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .what p {
    margin: 0;
    color: var(--muted);
  }

  .plus {
    color: var(--accent);
    font-weight: 600;
  }

  .minus {
    color: var(--danger);
    font-weight: 600;
  }

  .buttons,
  .foot {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .foot {
    padding: 8px 12px;
    border-top: 1px solid var(--brass-dim);
  }

  .spacer {
    flex: 1;
  }
</style>
