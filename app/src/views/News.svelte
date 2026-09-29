<script lang="ts">
  // What's new and what's next: the roadmap as a timeline, then every release.
  import { onMount } from "svelte";
  import { RELEASES, ROADMAP } from "../lib/changelog";
  import { app } from "../lib/state.svelte";
  import { settings } from "../lib/settings.svelte";

  // Opening the page counts as having seen this version's news.
  onMount(() => {
    if (app.version && settings.seenVersion !== app.version) settings.set("seenVersion", app.version);
  });

  const date = (iso: string) =>
    new Intl.DateTimeFormat(undefined, { day: "numeric", month: "long", year: "numeric" }).format(new Date(`${iso}T12:00:00`));
</script>

<div class="news scroll">
  <header>
    <h1>What's new, what's next</h1>
    <p class="skaven">"Snikkit's engineering log. Every improvement, carefully recorded. The explosions, mostly not."</p>
  </header>

  <section class="card forged block">
    <span class="label">Roadmap</span>
    <ol class="timeline">
      {#each ROADMAP as stage (stage.stage)}
        <li class="stage {stage.stage}">
          <span class="node" aria-hidden="true"></span>
          <span class="stage-name">{stage.label}</span>
          <ul class="items">
            {#each stage.items as item (item.title)}
              <li>
                <strong>{item.title}</strong>
                <span class="muted">{item.what}</span>
              </li>
            {/each}
          </ul>
        </li>
      {/each}
    </ol>
  </section>

  {#each RELEASES as r (r.version)}
    <section class="card forged block release" class:current={r.version === app.version}>
      <div class="release-head">
        <h2>{r.version} <span class="release-name">· {r.name}</span></h2>
        <span class="faint">{date(r.date)}</span>
        {#if r.version === app.version}<span class="chip accent">you're on this one</span>{/if}
      </div>
      <ul class="changes">
        {#each r.changes as c, i (i)}<li>{c}</li>{/each}
      </ul>
    </section>
  {/each}
</div>

<style>
  .news {
    height: 100%;
    padding: 22px 28px 40px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 980px;
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  header p {
    margin: 0;
    color: var(--accent);
    text-shadow: 0 0 14px rgb(168 242 63 / 0.3);
  }

  .block {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* The roadmap: a glowing line with a node per stage. */
  .timeline {
    list-style: none;
    margin: 0;
    padding: 0 0 0 6px;
    display: flex;
    flex-direction: column;
  }

  .stage {
    position: relative;
    display: grid;
    grid-template-columns: 22px 70px 1fr;
    gap: 10px;
    padding-bottom: 16px;
  }

  /* The line between nodes. */
  .stage::before {
    content: "";
    position: absolute;
    left: 6px;
    top: 16px;
    bottom: -2px;
    width: 2px;
    background: linear-gradient(180deg, var(--accent-dim), var(--border));
  }

  .stage:last-child::before {
    display: none;
  }

  .node {
    width: 14px;
    height: 14px;
    margin-top: 3px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    background: var(--bg);
  }

  .now .node {
    border-color: var(--accent);
    background: var(--warpstone-glass);
    box-shadow: 0 0 10px rgb(140 255 50 / 0.6);
    animation: node-pulse 2.4s ease-in-out infinite;
  }

  .next .node {
    border-color: var(--accent-dim);
  }

  @keyframes node-pulse {
    50% {
      box-shadow: 0 0 18px rgb(140 255 50 / 0.85);
    }
  }

  .stage-name {
    font-family: var(--font-display);
    font-weight: var(--font-display-weight, 700);
    font-size: 17px;
    color: var(--muted);
  }

  .now .stage-name {
    color: var(--accent);
    text-shadow: var(--glow-text);
  }

  .items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .items li {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 13.5px;
  }

  .later .items strong {
    color: var(--muted);
  }

  /* Releases. */
  .release-head {
    display: flex;
    align-items: baseline;
    gap: 12px;
    flex-wrap: wrap;
  }

  .release-head h2 {
    font-size: 24px;
  }

  .release.current h2 {
    color: var(--accent);
    text-shadow: var(--glow-text);
  }

  .release-name {
    color: var(--brass);
  }

  .changes {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 13.5px;
    color: var(--muted);
  }

  .changes li::marker {
    color: var(--accent);
  }
</style>
