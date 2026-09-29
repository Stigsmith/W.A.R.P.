<script lang="ts">
  // W.A.R.P.'s automatic backups, newest first, each restorable. Used in Settings,
  // on the "couldn't start" screen and when the library turns up empty.
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { count } from "../lib/format";
  import type { BackupInfo } from "../lib/types";

  let { limit = 7 }: { limit?: number } = $props();

  let backups = $state<BackupInfo[] | null>(null);
  let restoring = $state<string | null>(null);

  onMount(async () => {
    backups = (await app.attempt(async () => (await api()).backups())) ?? [];
  });

  const when = (unix: number) =>
    new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }).format(
      new Date(unix * 1000),
    );

  async function restore(b: BackupInfo) {
    const a = await api();
    const ok = await a.confirm(
      `Go back to the backup from ${when(b.at)}?\n\n${count(b.mods, "mod")}, ${count(b.sets, "set")}, ${count(b.profiles, "profile")}.\n` +
        "Your current library is kept in the backups folder, not deleted.",
      "Restore backup",
    );
    if (!ok) return;
    restoring = b.path;
    if (await app.run(() => a.restoreBackup(b.path))) {
      // Start over on the restored library.
      location.reload();
    }
    restoring = null;
  }
</script>

{#if backups === null}
  <p class="faint">Looking for backups…</p>
{:else if backups.length === 0}
  <p class="faint">No backups yet. W.A.R.P. makes one each time it starts with your mods in it.</p>
{:else}
  <ul class="backups">
    {#each backups.slice(0, limit) as b (b.path)}
      <li>
        <span class="when">{when(b.at)}</span>
        <span class="muted what">{count(b.mods, "mod")} · {count(b.sets, "set")} · {count(b.profiles, "profile")}</span>
        <button class="small" onclick={() => restore(b)} disabled={restoring !== null}>
          {restoring === b.path ? "Restoring…" : "Restore"}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .backups {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: 620px;
  }

  li {
    display: grid;
    grid-template-columns: 170px 1fr auto;
    align-items: center;
    gap: 12px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: rgb(0 0 0 / 0.2);
    font-size: 13px;
  }

  .when {
    font-variant-numeric: tabular-nums;
  }

  p {
    margin: 0;
    font-size: 13px;
  }
</style>
