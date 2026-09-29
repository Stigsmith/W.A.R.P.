<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { KOFI_URL } from "../lib/links";
  import { HEADING_FONTS, settings } from "../lib/settings.svelte";
  import { copyReport, playtest } from "../lib/playtest.svelte";
  import WarpShard from "../components/WarpShard.svelte";

  let { ontour }: { ontour: () => void } = $props();
</script>

<div class="settings scroll">
  <h1>Settings</h1>

  <section>
    <span class="label">Heading font</span>
    <div class="fonts" role="radiogroup" aria-label="Heading font">
      {#each HEADING_FONTS as f (f.key)}
        <button
          class="font card"
          class:on={settings.headingFont === f.key}
          role="radio"
          aria-checked={settings.headingFont === f.key}
          onclick={() => settings.set("headingFont", f.key)}
        >
          <span class="sample" style:font-family={f.family} style:font-weight={f.weight}>W.A.R.P. · Solo Chaos</span>
          <span class="name">{f.name}</span>
          <span class="faint note">{f.note}</span>
        </button>
      {/each}
    </div>
    <p class="faint">The Warlock-Engineer's speech always uses IM Fell English.</p>
  </section>

  <section>
    <span class="label">Background</span>
    <label class="toggle">
      <input type="checkbox" checked={settings.motes} onchange={(e) => settings.set("motes", e.currentTarget.checked)} />
      <span>
        <strong>Warpstone motes</strong>
        <small class="muted">Glowing dust drifting through the dark. They pause while the window is hidden, so they never cost you frames in-game.</small>
      </span>
    </label>
  </section>

  <section>
    <span class="label">Tutorial</span>
    <div class="row">
      <button onclick={ontour}><WarpShard size={15} /> Replay the Warlock-Engineer's tour</button>
    </div>
  </section>

  <section>
    <span class="label">Where things live</span>
    <dl>
      <dt>Game</dt>
      <dd class="mono">{app.install?.game_dir ?? app.installError ?? "not found"}</dd>
      <dt>WARP's data</dt>
      <dd class="mono">{app.dataDir}</dd>
      <dt>Load order file</dt>
      <dd class="mono">{app.install ? `${app.install.game_dir}\\warp_mods.txt` : "-"} <span class="faint">(written when you press Play)</span></dd>
    </dl>
  </section>

  <section>
    <span class="label">Something wrong?</span>
    <div class="row">
      <button class="primary" onclick={copyReport}>Copy report</button>
      <span class="muted">What W.A.R.P. sees on this PC (game, mods, profiles, recent errors), ready to paste into Discord.</span>
    </div>
    {#if playtest.hidden}
      <div class="row">
        <button class="small" onclick={() => playtest.update({ hidden: false })}>Show the playtest checklist on the start page</button>
      </div>
    {/if}
  </section>

  <section>
    <span class="label">Support</span>
    <div class="row">
      <button class="kofi" onclick={async () => (await api()).openUrl(KOFI_URL)}><WarpShard size={15} /> Feed the Warlock-Engineer on Ko-fi</button>
      <span class="skaven faint">"Every coin buys one (1) warpstone shard. Probably."</span>
    </div>
  </section>

  <section class="about">
    <span class="label">About</span>
    <p>
      <strong>W.A.R.P.</strong> <span class="muted">· Warhammer Advanced Resource Platform · version {app.version}</span>
    </p>
    <p class="faint">A mod manager for Total War: WARHAMMER III, made by Stigsmith. Not affiliated with Creative Assembly or Games Workshop.</p>
  </section>
</div>

<style>
  .settings {
    height: 100%;
    padding: 22px 28px 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    max-width: 980px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .about p {
    margin: 0;
  }

  .fonts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 10px;
  }

  .font {
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 14px 16px;
    text-align: left;
    white-space: normal;
  }

  .font.on {
    border-color: var(--accent);
    box-shadow: var(--glow);
  }

  .sample {
    font-size: 24px;
    line-height: 1.15;
    color: var(--text);
  }

  .font.on .sample {
    color: var(--accent);
    text-shadow: var(--glow-text);
  }

  .name {
    font-weight: 600;
  }

  .note {
    font-size: 12.5px;
  }

  .toggle {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    cursor: pointer;
    max-width: 640px;
  }

  .toggle input {
    margin-top: 4px;
    width: 16px;
    height: 16px;
  }

  .toggle small {
    display: block;
    font-size: 13px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }

  dl {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 6px 16px;
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    word-break: break-all;
  }

  .kofi {
    border-color: var(--brass-dim);
    color: var(--brass);
  }
</style>
