<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { tierColor } from "../lib/format";
  import type { Placement, Reason, RuleKind } from "../lib/types";

  let { placements, filter = "" }: { placements: Placement[]; filter?: string } = $props();

  let expanded = $state<string | null>(null);
  const maxPriority = $derived(Math.max(1, ...app.taxonomy.tier.map((t) => t.priority)));

  type Row = { kind: "band"; tier: string; count: number; index: number } | { kind: "pack"; p: Placement; index: number };

  const rows = $derived.by(() => {
    const out: Row[] = [];
    const q = filter.trim().toLowerCase();
    placements.forEach((p, index) => {
      // A band per run of one tier. Rules can lift a pack out of its tier, so a
      // tier may come back further down; each run gets its own band and count.
      if (index === 0 || placements[index - 1].tier !== p.tier) {
        let end = index;
        while (end < placements.length && placements[end].tier === p.tier) end++;
        out.push({ kind: "band", tier: p.tier, count: end - index, index });
      }
      if (!q || p.pack.toLowerCase().includes(q) || app.title(p.workshop_id, p.pack).toLowerCase().includes(q)) {
        out.push({ kind: "pack", p, index });
      }
    });
    return out;
  });

  const ruleWord: Record<RuleKind, string> = { requires: "requires", patches: "patches", pin: "is pinned above" };

  function explain(r: Reason): string | null {
    switch (r.kind) {
      case "default":
        return `Default position: tier ${app.tier(r.tier)?.name ?? r.tier}, role ${app.role(r.role)?.name ?? r.role}.`;
      case "above":
        return r.rule === "pin" ? `You pinned it above ${r.other}.` : `Sits above ${r.other} because it ${ruleWord[r.rule]} it.`;
      case "below":
        return r.rule === "pin" ? `You pinned ${r.other} above it.` : `Sits below ${r.other}, which ${ruleWord[r.rule]} it.`;
      case "raised":
        return `Moved up past its default spot to stay above ${r.other}.`;
      case "in_cycle":
        return "Caught in contradictory rules, so it was placed by the default order.";
    }
  }

  const has = (p: Placement, kind: Reason["kind"]) => p.reasons.some((r) => r.kind === kind);
  const ruleCount = (p: Placement) => p.reasons.filter((r) => r.kind === "above" || r.kind === "below").length;
</script>

<ol class="order">
  {#each rows as row (row.kind === "band" ? `band-${row.index}` : row.p.pack)}
    {#if row.kind === "band"}
      {@const t = app.tier(row.tier)}
      <li class="band" style:--tier={tierColor(t?.priority ?? 0, maxPriority)}>
        <span class="swatch"></span>
        <span class="band-name">{t?.name ?? row.tier}</span>
        <span class="faint">{row.count}</span>
      </li>
    {:else}
      {@const p = row.p}
      {@const t = app.tier(p.tier)}
      {@const entry = p.workshop_id ? app.byId.get(p.workshop_id) : undefined}
      <li class="pack" class:open={expanded === p.pack} style:--tier={tierColor(t?.priority ?? 0, maxPriority)}>
        <button class="row" onclick={() => (expanded = expanded === p.pack ? null : p.pack)} aria-expanded={expanded === p.pack}>
          <span class="pos mono">{row.index + 1}</span>
          <span class="names">
            <span class="pack-name mono">{p.pack}</span>
            <span class="title">{app.title(p.workshop_id, p.pack)}</span>
          </span>
          <span class="badges">
            {#if has(p, "in_cycle")}<span class="chip danger">rule conflict</span>{/if}
            {#if has(p, "raised")}<span class="chip info">raised</span>{/if}
            {#if ruleCount(p) > 0}<span class="chip">{ruleCount(p)} rule{ruleCount(p) > 1 ? "s" : ""}</span>{/if}
            {#if entry && !entry.subscribed}<span class="chip warn">unsubscribed</span>{/if}
            <span class="chip role">{app.role(p.role)?.name ?? p.role}</span>
          </span>
        </button>
        {#if expanded === p.pack}
          <ul class="why">
            {#each p.reasons as r, i (i)}
              {@const text = explain(r)}
              {#if text}<li class:strong={r.kind !== "default"}>{text}</li>{/if}
            {/each}
          </ul>
        {/if}
      </li>
    {/if}
  {/each}
</ol>

<style>
  .order {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .band {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 10px 6px;
    position: sticky;
    top: 0;
    background: var(--bg);
    z-index: 1;
  }

  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    background: var(--tier);
    box-shadow: 0 0 10px color-mix(in srgb, var(--tier) 45%, transparent);
  }

  .band-name {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--tier);
  }

  .pack {
    border-left: 3px solid color-mix(in srgb, var(--tier) 70%, transparent);
    margin-bottom: 1px;
    background: var(--surface);
  }

  .pack.open {
    background: var(--surface-2);
  }

  .row {
    width: 100%;
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 6px 12px 6px 6px;
    border: 0;
    border-radius: 0;
    background: transparent;
    text-align: left;
  }

  .row:hover:not(:disabled) {
    background: var(--surface-2);
    border: 0;
  }

  .pos {
    color: var(--faint);
    text-align: right;
  }

  .names {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .pack-name,
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 12px;
    color: var(--muted);
  }

  .badges {
    display: flex;
    gap: 5px;
  }

  .chip.role {
    min-width: 92px;
    justify-content: center;
  }

  .why {
    margin: 0;
    padding: 2px 16px 12px 62px;
    font-size: 13px;
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .why li.strong {
    color: var(--text);
  }
</style>
