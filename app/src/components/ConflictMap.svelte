<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { count } from "../lib/format";
  import type { ConflictReport, ContentKind, PairConflict, Severity } from "../lib/types";

  let { report }: { report: ConflictReport } = $props();

  let open = $state<string | null>(null);
  let showIntended = $state(false);
  let showLow = $state(false);

  const kindName: Record<ContentKind, [string, string]> = {
    db_table: ["DB table", "DB tables"],
    startpos: ["start position", "start positions"],
    script: ["script", "scripts"],
    text: ["text file", "text files"],
    map: ["map file", "map files"],
    ui: ["UI file", "UI files"],
    art: ["art file", "art files"],
    other: ["other file", "other files"],
  };

  const severityText: Record<Severity, string> = {
    high: "Content from the lower pack is thrown away: a whole DB table or campaign start. Check this one.",
    medium: "A script, text or map file is replaced. Often deliberate, but it can switch off a feature.",
    low: "Looks only: the top pack's models, textures or UI win.",
  };

  const problems = $derived(report.pairs.filter((p) => !p.intended));
  const intended = $derived(report.pairs.filter((p) => p.intended));
  const bySeverity = (s: Severity) => problems.filter((p) => p.severity === s);
  const serious = $derived([...bySeverity("high"), ...bySeverity("medium")]);
  const low = $derived(bySeverity("low"));

  const key = (p: PairConflict) => `${p.winner}>${p.loser}`;
  const kinds = (p: PairConflict) =>
    (Object.entries(p.by_kind) as [ContentKind, number][]).map(([k, n]) => `${n} ${kindName[k][n === 1 ? 0 : 1]}`).join(", ");
</script>

{#snippet pair(p: PairConflict)}
  <li class="pair {p.intended ? 'intended' : p.severity}">
    <button class="head" onclick={() => (open = open === key(p) ? null : key(p))} aria-expanded={open === key(p)}>
      <span class="sev">{p.intended ? "intended" : p.severity}</span>
      <span class="who">
        <span><strong class="mono">{p.winner}</strong> <span class="faint">overrides</span> <span class="mono">{p.loser}</span></span>
        <span class="faint small">{app.title(null, p.winner)} · over · {app.title(null, p.loser)}</span>
      </span>
      <span class="what">{kinds(p)}</span>
    </button>
    {#if open === key(p)}
      <div class="detail">
        <p class="muted">
          {#if p.intended}{p.winner} is meant to override {p.loser}: it patches or needs it.{:else}{severityText[p.severity]}{/if}
          The pack higher in the load order wins.
        </p>
        <ul class="files mono">
          {#each p.files as f (f)}<li>{f}</li>{/each}
          {#if p.total > p.files.length}<li class="faint">… and {p.total - p.files.length} more</li>{/if}
        </ul>
      </div>
    {/if}
  </li>
{/snippet}

<div class="map">
  <div class="summary">
    <div class="stat" class:alert={bySeverity("high").length > 0}>
      <strong>{bySeverity("high").length}</strong><span>high risk</span>
    </div>
    <div class="stat" class:warnish={bySeverity("medium").length > 0}>
      <strong>{bySeverity("medium").length}</strong><span>medium</span>
    </div>
    <div class="stat"><strong>{low.length}</strong><span>cosmetic</span></div>
    <div class="stat ok"><strong>{intended.length}</strong><span>intended</span></div>
    <p class="muted explain">
      Packs that ship the same file: only the one higher in the load order is used. Cosmetic overlaps are normal
      with big modlists; high and medium ones are worth a look.
    </p>
  </div>

  {#if report.not_indexed.length}
    <div class="notice">
      {count(report.not_indexed.length, "pack")} in this profile {report.not_indexed.length === 1 ? "isn't" : "aren't"} installed, so
      {report.not_indexed.length === 1 ? "it" : "they"} can't be checked: <span class="mono">{report.not_indexed.join(", ")}</span>
    </div>
  {/if}

  {#if serious.length}
    <h3 class="section">Worth a look</h3>
    <ul class="pairs">{#each serious as p (key(p))}{@render pair(p)}{/each}</ul>
  {:else}
    <p class="clean">No high or medium risk overlaps. Nothing gets silently thrown away.</p>
  {/if}

  {#if report.shadowed.length}
    <h3 class="section">Mostly overridden packs</h3>
    <ul class="shadowed">
      {#each report.shadowed as s (s.pack)}
        <li>
          <span class="mono">{s.pack}</span>
          <span class="muted">{s.overridden} of {s.files} files come from {s.by.join(", ")}{s.overridden === s.files ? " - it adds nothing" : ""}</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if low.length}
    <button class="ghost small toggle" onclick={() => (showLow = !showLow)}>
      {showLow ? "Hide" : "Show"} {count(low.length, "cosmetic overlap")}
    </button>
    {#if showLow}<ul class="pairs">{#each low as p (key(p))}{@render pair(p)}{/each}</ul>{/if}
  {/if}

  {#if intended.length}
    <button class="ghost small toggle" onclick={() => (showIntended = !showIntended)}>
      {showIntended ? "Hide" : "Show"} {count(intended.length, "intended overlap")}
    </button>
    {#if showIntended}<ul class="pairs">{#each intended as p (key(p))}{@render pair(p)}{/each}</ul>{/if}
  {/if}
</div>

<style>
  .map {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-bottom: 24px;
  }

  .summary {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .stat {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 8px 14px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .stat strong {
    font-family: var(--font-display);
    font-size: 20px;
  }

  .stat span {
    color: var(--muted);
    font-size: 12.5px;
  }

  .stat.alert {
    border-color: rgb(216 80 58 / 0.5);
    background: var(--danger-dim);
  }

  .stat.alert strong {
    color: var(--danger);
  }

  .stat.warnish strong {
    color: var(--warn);
  }

  .stat.ok strong {
    color: var(--accent);
  }

  .explain {
    flex: 1;
    min-width: 260px;
    font-size: 12.5px;
  }

  .notice {
    padding: 8px 12px;
    border-left: 3px solid var(--warn);
    background: var(--warn-dim);
    border-radius: var(--radius-sm);
    font-size: 13px;
  }

  .section {
    margin-top: 6px;
    color: var(--brass);
    font-size: 12px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .clean {
    padding: 14px 16px;
    border-radius: var(--radius);
    border: 1px solid var(--accent-dim);
    background: var(--ok-dim);
    color: var(--accent);
  }

  .pairs,
  .shadowed,
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .pair {
    border-left: 3px solid var(--faint);
    background: var(--surface);
    margin-bottom: 2px;
  }

  .pair.high {
    border-left-color: var(--danger);
  }

  .pair.medium {
    border-left-color: var(--warn);
  }

  .pair.intended {
    border-left-color: var(--accent-dim);
  }

  .head {
    width: 100%;
    display: grid;
    grid-template-columns: 78px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border: 0;
    border-radius: 0;
    background: transparent;
    text-align: left;
  }

  .head:hover:not(:disabled) {
    background: var(--surface-2);
    border: 0;
  }

  .sev {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .high .sev {
    color: var(--danger);
  }

  .medium .sev {
    color: var(--warn);
  }

  .intended .sev {
    color: var(--accent);
  }

  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .who > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 12px;
  }

  .what {
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
  }

  .detail {
    padding: 0 12px 12px 102px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
  }

  .files {
    max-height: 220px;
    overflow: auto;
    font-size: 12px;
    color: var(--muted);
  }

  .shadowed li {
    display: flex;
    flex-direction: column;
    padding: 7px 12px;
    background: var(--surface);
    border-left: 3px solid var(--brass-dim);
    margin-bottom: 2px;
    font-size: 13px;
  }

  .toggle {
    align-self: flex-start;
  }
</style>
