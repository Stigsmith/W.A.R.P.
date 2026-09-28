<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { count } from "../lib/format";
  import { KOFI_URL } from "../lib/links";
  import type { ImportSummary } from "../lib/types";
  import WarpShard from "../components/WarpShard.svelte";

  import type { View } from "../lib/views";

  let { onnavigate, ontour }: { onnavigate: (view: View) => void; ontour: () => void } = $props();

  let importing = $state(false);
  let imported = $state<ImportSummary | null>(null);

  const firstRun = $derived(app.library.length === 0);
  const installed = $derived(app.library.filter((e) => e.subscribed).length);
  const pending = $derived(
    app.library.filter((e) => e.subscribed && e.installed_version !== null && e.installed_version < e.info.time_updated).length,
  );
  const lastProfile = $derived(app.profiles[0] ?? null);

  const greetings = [
    "Yes-yes! The warp-machine hums. What do we conquer today, man-thing?",
    "Welcome back, most-clever warlord. The burrow is swept, the warpstone polished.",
    "Quick-quick! Mods to sort, enemies to crush, crashes to blame on someone else!",
    "The Horned Rat watches. Try not to load two campaign maps this time.",
  ];
  const greeting = greetings[Math.floor(Math.random() * greetings.length)];

  async function importWorkbook() {
    const a = await api();
    const path = await a.pickFile([{ name: "W.A.R.P. v1 workbook", extensions: ["xlsm", "xlsx", "zip"] }]);
    if (!path) return;
    importing = true;
    imported = (await app.attempt(() => a.importV1(path))) ?? null;
    importing = false;
    if (imported) await app.refresh();
  }

  async function kofi() {
    await (await api()).openUrl(KOFI_URL);
  }
</script>

<div class="home scroll">
  <div class="hero">
    <img class="logo" src="/logo.png" alt="W.A.R.P." />
    <p class="expansion">Warhammer Advanced Resource Platform</p>
    <p class="skaven greeting">
      {firstRun ? "Yes-yes! A new warlord! Let the Warlock-Engineer find your mod-things first." : greeting}
    </p>
  </div>

  {#if firstRun}
    <div class="card forged setup">
      <div class="option">
        <div>
          <h3>Read my installed mods</h3>
          <p class="muted">
            {#if app.install}Finds every Warhammer III mod Steam has installed and reads what's inside each one.
            {:else}The game wasn't found{app.installError ? `: ${app.installError}` : ""}.{/if}
          </p>
        </div>
        <button class="primary" data-tour="sync" onclick={() => app.sync()} disabled={!app.install || app.syncing}>
          {app.syncing ? "Sniffing…" : "Read mods"}
        </button>
      </div>
      <div class="option">
        <div>
          <h3>Import a W.A.R.P. v1 workbook</h3>
          <p class="muted">Only if you used the old spreadsheet: brings in your categories and Components as sets.</p>
        </div>
        <button onclick={importWorkbook} disabled={importing}>{importing ? "Importing…" : "Choose workbook…"}</button>
      </div>
    </div>
    {#if imported}<p class="muted">Imported {imported.mods} mods and {imported.sets.length} sets.</p>{/if}
  {:else}
    <div class="tiles">
      {#if lastProfile}
        <div class="card forged tile main-tile">
          <span class="label">Continue</span>
          <h2>{lastProfile.name}</h2>
          <p class="muted">{lastProfile.sets.length ? lastProfile.sets.join(" + ") : "your own pick of mods"}</p>
          <div class="row">
            <button class="primary" onclick={() => onnavigate("profiles")}>Open profile</button>
          </div>
        </div>
      {/if}
      <button class="card tile link" onclick={() => onnavigate("multiplayer")}>
        <span class="label">Multiplayer</span>
        <h3>Compare with a friend</h3>
        <p class="muted">Paste their code; see exactly what differs.</p>
      </button>
      <button class="card tile link" onclick={() => onnavigate("library")}>
        <span class="label">Library</span>
        <h3>{count(installed, "mod")} installed</h3>
        <p class="muted">
          {#if pending}<span class="chip warn">{pending} waiting for a Steam update</span>{:else}All up to date with Steam.{/if}
        </p>
      </button>
    </div>
  {/if}

  <div class="footer">
    <button class="ghost" onclick={ontour}>
      <WarpShard size={16} /> Take the tour, guided by a Warlock-Engineer
    </button>
    <button class="kofi" data-tour="kofi" onclick={kofi}>
      <WarpShard size={26} pulse />
      <span>
        <strong>Feed the Warlock-Engineer</strong>
        <small class="skaven">"Warpstone does not grow on trees, man-thing. Yes-yes, tip on Ko-fi!"</small>
      </span>
    </button>
  </div>
</div>

<style>
  .home {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 22px;
    padding: 28px 32px 32px;
  }

  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .expansion {
    margin: -6px 0 0;
    font-family: var(--font-display);
    font-size: 17px;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--brass);
    text-shadow: 0 0 14px rgb(207 159 77 / 0.35);
  }

  /* No box: the glow follows the logo's own outline. */
  .logo {
    width: min(360px, 42vh);
    height: auto;
    filter: brightness(1.18) contrast(1.06) drop-shadow(0 0 14px rgb(168 242 63 / 0.35)) drop-shadow(0 0 46px rgb(168 242 63 / 0.18));
    animation: logo-pulse 5s ease-in-out infinite;
  }

  @keyframes logo-pulse {
    50% {
      filter: brightness(1.25) contrast(1.06) drop-shadow(0 0 20px rgb(168 242 63 / 0.5)) drop-shadow(0 0 70px rgb(168 242 63 / 0.26));
    }
  }

  .greeting {
    max-width: 560px;
    text-align: center;
    color: var(--accent);
    text-shadow: 0 0 14px rgb(168 242 63 / 0.35);
  }

  .setup {
    width: min(620px, 100%);
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

  .tiles {
    width: min(980px, 100%);
    display: grid;
    grid-template-columns: 1.3fr 1fr 1fr;
    gap: 12px;
  }

  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 18px;
    text-align: left;
    white-space: normal;
  }

  .tile p {
    font-size: 13px;
  }

  .tile .row {
    margin-top: auto;
    padding-top: 8px;
  }

  button.link:hover:not(:disabled) {
    border-color: var(--accent-dim);
    box-shadow: var(--glow);
  }

  .footer {
    width: min(980px, 100%);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: auto;
  }

  .kofi {
    gap: 12px;
    padding: 10px 16px;
    border-color: var(--brass-dim);
    background: linear-gradient(180deg, rgb(207 159 77 / 0.16), rgb(207 159 77 / 0.05));
    text-align: left;
    white-space: normal;
    max-width: 440px;
  }

  .kofi strong {
    display: block;
    color: var(--brass);
  }

  .kofi small {
    font-size: 14px;
    color: var(--muted);
  }


  @media (max-width: 900px) {
    .tiles {
      grid-template-columns: 1fr;
    }
  }
</style>
