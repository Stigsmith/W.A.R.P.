<script lang="ts">
  import { onMount } from "svelte";
  import { isDesktop } from "./lib/api";
  import { app } from "./lib/state.svelte";
  import Library from "./views/Library.svelte";
  import Profiles from "./views/Profiles.svelte";
  import Multiplayer from "./views/Multiplayer.svelte";
  import Welcome from "./views/Welcome.svelte";

  type View = "library" | "profiles" | "multiplayer";
  let view = $state<View>("profiles");
  let error = $state<string | null>(null);

  const nav: { key: View; label: string; hint: string }[] = [
    { key: "profiles", label: "Profiles", hint: "Build load orders" },
    { key: "multiplayer", label: "Multiplayer", hint: "Share & compare lists" },
    { key: "library", label: "Library", hint: "All your mods" },
  ];

  onMount(() => {
    app.load().catch((e) => (error = String(e)));
  });

</script>

<div class="shell">
  <aside>
    <div class="brand">
      <img src="/logo.png" alt="" />
      <div>
        <div class="name">W.A.R.P.</div>
        <div class="sub">Warhammer III mod manager</div>
      </div>
    </div>

    {#if app.loaded && app.library.length > 0}
      <nav>
        {#each nav as item (item.key)}
          <button class="nav" class:active={view === item.key} onclick={() => (view = item.key)}>
            <span>{item.label}</span>
            <small>{item.hint}</small>
          </button>
        {/each}
      </nav>
    {/if}

    <div class="foot">
      {#if app.loaded && app.library.length > 0}
        <button class="sync" onclick={() => app.sync()} disabled={app.syncing || !app.install} title={app.installError ?? "Read your installed mods and what's inside them"}>
          <span class="dot" class:busy={app.syncing}></span>
          {app.syncing ? "Syncing…" : "Sync with game"}
        </button>
      {/if}
      <div class="faint small-print">
        {#if app.install}Game found{:else if app.loaded}Game not found{/if}
        · {app.library.length} mods{#if !isDesktop}&nbsp;· preview{/if}
      </div>
    </div>
  </aside>

  <main>
    {#if error}
      <div class="empty">
        <h2>W.A.R.P. couldn't start</h2>
        <p class="mono">{error}</p>
      </div>
    {:else if !app.loaded}
      <div class="empty">Loading…</div>
    {:else if app.library.length === 0}
      <Welcome />
    {:else if view === "library"}
      <Library />
    {:else if view === "profiles"}
      <Profiles />
    {:else}
      <Multiplayer />
    {/if}
  </main>

  {#if app.toast}
    <div class="toast" class:error={app.toast.kind === "error"} role="status">{app.toast.text}</div>
  {/if}
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100vh;
  }

  aside {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding: 18px 12px;
    border-right: 1px solid var(--brass-dim);
    background: linear-gradient(180deg, rgb(17 22 15 / 0.95) 0%, rgb(11 14 10 / 0.9) 70%);
    box-shadow: inset -1px 0 0 rgb(0 0 0 / 0.6);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px;
  }

  .brand img {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    box-shadow: 0 0 22px var(--accent-glow);
  }

  .name {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 18px;
    letter-spacing: 0.14em;
    color: var(--accent);
  }

  .sub {
    font-size: 11.5px;
    color: var(--muted);
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
  }

  .nav span {
    font-weight: 600;
  }

  .nav small {
    color: var(--faint);
    font-size: 11.5px;
  }

  .nav:hover:not(.active) {
    background: var(--surface);
    border-color: transparent;
  }

  .nav.active {
    background: linear-gradient(90deg, var(--ok-dim), transparent);
    border-left-color: var(--accent);
  }

  .nav.active span {
    color: var(--accent);
  }

  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 0 6px;
  }

  .small-print {
    font-size: 11.5px;
  }

  .sync {
    width: 100%;
    justify-content: center;
    border-color: var(--brass-dim);
    background: var(--surface);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
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
    background: var(--surface-3);
    border: 1px solid var(--accent-dim);
    box-shadow: 0 8px 30px rgb(0 0 0 / 0.5);
    animation: rise 0.18s ease-out;
    z-index: 50;
  }

  .toast.error {
    border-color: rgb(240 101 82 / 0.5);
    color: #ffd9d3;
  }

  @keyframes rise {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
  }
</style>
