<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import type { ImportSummary } from "../lib/types";

  let busy = $state(false);
  let summary = $state<ImportSummary | null>(null);

  async function importWorkbook() {
    const a = await api();
    const path = await a.pickFile([{ name: "W.A.R.P. v1 workbook", extensions: ["xlsm", "xlsx", "zip"] }]);
    if (!path) return;
    busy = true;
    summary = (await app.attempt(() => a.importV1(path))) ?? null;
    busy = false;
    if (summary) await app.refresh();
  }
</script>

<div class="welcome">
  <img src="/logo.png" alt="W.A.R.P." />
  <h1>Welcome to W.A.R.P.</h1>
  <p class="muted lead">
    Load orders worked out from what your mods <em>are</em>, and multiplayer lists that match on the
    first try.
  </p>

  <div class="card options">
    <div class="option">
      <div>
        <h3>Import a W.A.R.P. v1 workbook</h3>
        <p class="muted">Brings in your hand-sorted categories, Components (as sets) and your Profile Builder profile.</p>
      </div>
      <button onclick={importWorkbook} disabled={busy}>
        {busy ? "Importing…" : "Choose workbook…"}
      </button>
    </div>
    <div class="option">
      <div>
        <h3>Read my installed mods</h3>
        <p class="muted">
          {#if app.install}Finds every Warhammer III mod Steam has installed and reads what's inside each one.
          {:else}The game wasn't found{app.installError ? `: ${app.installError}` : ""}.{/if}
        </p>
      </div>
      <button class="primary" onclick={() => app.sync()} disabled={!app.install || app.syncing}>
        {app.syncing ? "Reading…" : "Read mods"}
      </button>
    </div>
  </div>

  {#if summary}
    <p class="muted">Imported {summary.mods} mods and {summary.sets.length} sets.</p>
  {/if}
</div>

<style>
  .welcome {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 40px;
    text-align: center;
  }

  img {
    width: 120px;
    height: 120px;
    border-radius: 24px;
    box-shadow: 0 0 60px var(--accent-glow);
    margin-bottom: 8px;
  }

  .lead {
    max-width: 460px;
    font-size: 15px;
  }

  .options {
    width: min(560px, 100%);
    margin-top: 14px;
    text-align: left;
  }

  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 18px 20px;
  }

  .option + .option {
    border-top: 1px solid var(--border);
  }

  .option p {
    margin-top: 3px;
    font-size: 13px;
  }

</style>
