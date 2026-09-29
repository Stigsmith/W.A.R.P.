<script lang="ts">
  import { onMount } from "svelte";
  import { isDesktop } from "./lib/api";
  import { app } from "./lib/state.svelte";
  import { settings } from "./lib/settings.svelte";
  import { copyReport, playtest } from "./lib/playtest.svelte";
  import type { View } from "./lib/views";
  import Backdrop from "./components/Backdrop.svelte";
  import Tour from "./components/Tour.svelte";
  import WarpShard from "./components/WarpShard.svelte";
  import Home from "./views/Home.svelte";
  import Guide from "./views/Guide.svelte";
  import Library from "./views/Library.svelte";
  import News from "./views/News.svelte";
  import Profiles from "./views/Profiles.svelte";
  import Multiplayer from "./views/Multiplayer.svelte";
  import Settings from "./views/Settings.svelte";

  let view = $state<View>("home");
  let error = $state<string | null>(null);
  let touring = $state<"full" | "profile" | null>(null);

  const nav: { key: View; label: string; hint: string }[] = [
    { key: "home", label: "Home", hint: "The burrow" },
    { key: "profiles", label: "Profiles", hint: "Build load orders" },
    { key: "multiplayer", label: "Multiplayer", hint: "Share & compare lists" },
    { key: "library", label: "Library", hint: "All your mods" },
    { key: "guide", label: "Load order", hint: "How the tiers work" },
  ];

  const hasMods = $derived(app.loaded && app.library.length > 0);

  onMount(() => {
    app.load().catch((e) => (error = String(e)));
  });

  // First time there are mods to show, Snikkit gives the tour (once).
  $effect(() => {
    if (hasMods && !settings.tourDone && !touring) startTour();
  });

  function startTour(part: "full" | "profile" = "full") {
    touring = part;
  }

  function endTour() {
    touring = null;
    settings.set("tourDone", true);
  }
</script>

<Backdrop motes={settings.motes} />

<div class="shell">
  <aside>
    <button class="brand" onclick={() => (view = "home")} aria-label="Home" title="W.A.R.P.: Warhammer Advanced Resource Platform">
      <img src="/logo.png" alt="" />
      <span>
        <span class="name">W.A.R.P.</span>
        <span class="sub">Warhammer III mod manager</span>
      </span>
    </button>

    {#if hasMods}
      <nav>
        {#each nav as item (item.key)}
          <button class="nav" class:active={view === item.key} data-tour="nav-{item.key}" onclick={() => (view = item.key)}>
            <span>{item.label}</span>
            <small>{item.hint}</small>
          </button>
        {/each}
      </nav>
    {/if}

    <div class="foot">
      {#if hasMods}
        <button
          class="sync"
          data-tour="sync"
          onclick={() => app.sync()}
          disabled={app.syncing || !app.install}
          title={app.installError ?? "Read your installed mods and what's inside them"}
        >
          <span class="dot" class:busy={app.syncing}></span>
          {app.syncing ? "Syncing…" : "Sync with game"}
        </button>
      {/if}
      <button class="nav settings-link" class:active={view === "news"} onclick={() => (view = "news")}>
        <span>
          What's new
          {#if app.version && settings.seenVersion !== app.version}<span class="new-dot" title="New in {app.version}"></span>{/if}
        </span>
      </button>
      <button class="nav settings-link" class:active={view === "settings"} onclick={() => (view = "settings")}>
        <span>Settings</span>
      </button>
      <div class="faint small-print">
        {#if app.loaded}
          v{app.version} · {app.install ? "Game found" : "Game not found"} · {app.library.length} mods{#if !isDesktop}&nbsp;· preview{/if}
        {/if}
      </div>
    </div>
  </aside>

  <main>
    {#if error}
      <div class="empty broken">
        <WarpShard size={40} pulse />
        <h2>W.A.R.P. couldn't start</h2>
        <p class="skaven">"Something went boom-boom in the warp-engine. Not Snikkit's fault! Tell the man-thing who gave you this."</p>
        <p class="mono">{error}</p>
        <p class="muted">
          Close any other W.A.R.P. windows and start it again. If it keeps happening, press Copy report and send it along.
        </p>
        <button class="primary" onclick={copyReport}>Copy report</button>
      </div>
    {:else if !app.loaded}
      <div class="empty skaven">Sharpening whiskers…</div>
    {:else if view === "settings"}
      <Settings ontour={() => startTour()} />
    {:else if view === "news"}
      <News />
    {:else if !hasMods || view === "home"}
      <Home onnavigate={(v) => (view = v)} ontour={() => startTour()} />
    {:else if view === "library"}
      <Library />
    {:else if view === "guide"}
      <Guide onnavigate={(v) => (view = v)} />
    {:else if view === "profiles"}
      <Profiles onprofiletour={() => !touring && startTour("profile")} />
    {:else}
      <Multiplayer />
    {/if}
  </main>

  {#if app.toast}
    <div class="toast" class:error={app.toast.kind === "error"} role="status">{app.toast.text}</div>
  {/if}
</div>

{#if playtest.unsent !== null}
  <div class="report-overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && (playtest.unsent = null)}>
    <div class="card forged report" role="dialog" aria-modal="true" aria-label="Report">
      <h3>Your report</h3>
      <p class="muted">The clipboard wouldn't take it. Click in the box, press Ctrl+A then Ctrl+C, and paste it to whoever sent you W.A.R.P.</p>
      <!-- svelte-ignore a11y_autofocus -->
      <textarea readonly autofocus onfocus={(e) => e.currentTarget.select()}>{playtest.unsent}</textarea>
      <div class="row"><button class="primary" onclick={() => (playtest.unsent = null)}>Done</button></div>
    </div>
  </div>
{/if}

{#if touring}
  <Tour part={touring} onview={(v) => (view = v)} onclose={endTour} />
{/if}

<style>
  .report-overlay {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 28px;
    background: rgb(0 0 0 / 0.6);
  }

  .report {
    width: min(760px, 100%);
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: rgb(10 14 9 / 0.97);
  }

  .report p {
    margin: 0;
  }

  .report textarea {
    height: min(420px, 55vh);
    font-family: var(--mono);
    font-size: 12px;
    resize: none;
  }

  .report .row {
    display: flex;
    justify-content: flex-end;
  }

  .broken {
    max-width: 620px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .broken p {
    margin: 0;
  }

  .broken .mono {
    color: var(--danger);
    word-break: break-word;
  }

  .shell {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100vh;
  }

  aside {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding: 16px 12px;
    border-right: 1px solid var(--brass-dim);
    background: linear-gradient(180deg, rgb(10 14 9 / 0.86), rgb(5 7 5 / 0.72));
    backdrop-filter: blur(4px);
    box-shadow:
      inset -1px 0 0 rgb(0 0 0 / 0.6),
      8px 0 30px rgb(0 0 0 / 0.35);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px;
    background: transparent;
    border: 0;
    text-align: left;
    white-space: normal;
  }

  .brand:hover:not(:disabled) {
    background: transparent;
    border: 0;
    box-shadow: none;
  }

  /* No box: the glow follows the logo's own outline. */
  .brand img {
    width: 58px;
    height: 58px;
    filter: brightness(1.2) drop-shadow(0 0 10px rgb(168 242 63 / 0.4));
    transition: filter 0.3s;
  }

  .brand:hover img {
    filter: brightness(1.3) drop-shadow(0 0 16px rgb(168 242 63 / 0.65));
  }

  .name {
    display: block;
    font-family: var(--font-display);
    font-weight: var(--font-display-weight, 700);
    font-size: 24px;
    line-height: 1;
    letter-spacing: 0.08em;
    color: var(--accent);
    text-shadow: var(--glow-text);
  }

  .sub {
    display: block;
    font-size: 11.5px;
    color: var(--muted);
    margin-top: 3px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .nav {
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
    padding: 9px 12px;
    background: transparent;
    border: 1px solid transparent;
    border-left: 2px solid transparent;
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    text-align: left;
    box-shadow: none;
  }

  .nav span {
    font-family: var(--font-display);
    font-weight: var(--font-display-weight, 700);
    font-size: 17px;
    letter-spacing: 0.03em;
  }

  .nav small {
    color: var(--faint);
    font-size: 11.5px;
  }

  .nav:hover:not(.active):not(:disabled) {
    background: var(--surface);
    border-color: transparent;
    border-left-color: var(--accent-dim);
    box-shadow: none;
  }

  .nav:hover:not(.active) span {
    color: var(--accent);
    text-shadow: 0 0 10px rgb(168 242 63 / 0.35);
  }

  .nav.active {
    background: linear-gradient(90deg, rgb(168 242 63 / 0.16), transparent 85%);
    border-left-color: var(--accent);
    box-shadow: -4px 0 16px -4px var(--accent-glow);
  }

  .nav.active span {
    color: var(--accent);
    text-shadow: var(--glow-text);
  }

  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
  }

  .settings-link {
    padding: 6px 12px;
  }

  .settings-link span {
    font-size: 15px;
  }

  .new-dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-left: 5px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
    vertical-align: middle;
    animation: dot-pulse 2s ease-in-out infinite;
  }

  @keyframes dot-pulse {
    50% {
      box-shadow: 0 0 14px var(--accent);
    }
  }

  .small-print {
    font-size: 11.5px;
    padding: 0 6px;
  }

  .sync {
    justify-content: center;
    border-color: var(--accent-dim);
    background: linear-gradient(180deg, rgb(168 242 63 / 0.1), rgb(168 242 63 / 0.03));
    box-shadow: 0 0 14px var(--accent-haze);
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow:
      0 0 6px var(--accent),
      0 0 14px var(--accent-glow);
  }

  .dot.busy {
    animation: pulse 0.9s ease-in-out infinite alternate;
  }

  @keyframes pulse {
    to {
      opacity: 0.25;
      box-shadow: 0 0 2px var(--accent);
    }
  }

  main {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .toast {
    position: fixed;
    right: 20px;
    bottom: 20px;
    max-width: 440px;
    padding: 11px 16px;
    border-radius: var(--radius);
    background: rgb(14 19 12 / 0.96);
    border: 1px solid var(--accent-dim);
    box-shadow:
      0 8px 30px rgb(0 0 0 / 0.5),
      0 0 24px var(--accent-haze);
    animation: rise 0.18s ease-out;
    z-index: 50;
  }

  .toast.error {
    border-color: rgb(221 83 59 / 0.5);
    color: #ffd9d3;
    box-shadow: 0 8px 30px rgb(0 0 0 / 0.5);
  }

  @keyframes rise {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
  }
</style>
