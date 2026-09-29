<script lang="ts">
  // How the load order is built, for people who don't know what belongs in which
  // tier: the tiers as a stack from "wins" to "foundation", what goes where (with
  // mods from the user's own library), roles within a tier, and the rules on top.
  import { app } from "../lib/state.svelte";
  import { count, tierColor } from "../lib/format";
  import WarpShard from "../components/WarpShard.svelte";
  import type { View } from "../lib/views";

  let { onnavigate }: { onnavigate: (view: View) => void } = $props();

  let openTier = $state<string | null>(null);

  const tiers = $derived([...app.taxonomy.tier].sort((a, b) => b.priority - a.priority));
  const roles = $derived([...app.taxonomy.role].sort((a, b) => b.priority - a.priority));
  const maxPriority = $derived(Math.max(1, ...tiers.map((t) => t.priority)));
  const installed = $derived(app.library.filter((e) => e.subscribed));

  const inTier = (key: string) => installed.filter((e) => e.knowledge.tier === key);

  /** A few well-known mods from this tier in the user's library, to make it concrete. */
  const examples = (key: string) =>
    inTier(key)
      .filter((e) => e.info.title)
      .sort((a, b) => b.info.subscriptions - a.info.subscriptions)
      .slice(0, 5);
</script>

<div class="guide scroll">
  <header>
    <h1>How the load order works</h1>
    <p class="skaven">
      "Listen-listen, man-thing! In the warp-machine, the higher mod-thing wins. So Snikkit puts small clever things on top and
      big heavy foundations at the bottom. Simple! Brilliant! Like everything Snikkit does."
    </p>
  </header>

  <section class="card forged block">
    <span class="label">1 · The top mod wins</span>
    <div class="wins">
      <div class="collision">
        <div class="pack top">
          <span class="who">A UI mod</span>
          <span class="mono">ui/units/icons/knight.png</span>
          <span class="verdict ok">used</span>
        </div>
        <div class="pack">
          <span class="who">A faction overhaul</span>
          <span class="mono">ui/units/icons/knight.png</span>
          <span class="verdict lost">ignored</span>
        </div>
      </div>
      <p class="muted">
        When two mods ship the same file, the game only uses the copy from the mod <strong>higher</strong> in the load order. So
        the more specific mod has to sit higher: the UI tweak above the overhaul, the submod above the mod it changes. That's the
        whole trick, and it's what keeps a 100+ mod list from falling apart.
      </p>
    </div>
  </section>

  <section class="card forged block">
    <span class="label">2 · The tiers, top to bottom</span>
    <p class="muted intro">
      Every mod gets a tier. Tiers stack from the most specific (top, wins collisions) to the foundations (bottom). Click one to
      see what belongs there{installed.length ? ", with mods from your own library" : ""}.
    </p>
    <div class="stack">
      <div class="rail" aria-hidden="true">
        <span class="end top">wins</span>
        <span class="line"></span>
        <span class="end bottom">foundation</span>
      </div>
      <ol class="tiers">
        {#each tiers as t, i (t.key)}
          {@const n = inTier(t.key).length}
          {@const isOpen = openTier === t.key}
          <li style:--tier={tierColor(t.priority, maxPriority)} class:open={isOpen}>
            <button class="tier-row" onclick={() => (openTier = isOpen ? null : t.key)} aria-expanded={isOpen}>
              <span class="num">{i + 1}</span>
              <span class="swatch"></span>
              <span class="name">{t.name}</span>
              <span class="hint muted">{t.hint}</span>
              {#if installed.length}<span class="n faint">{n ? count(n, "of yours", "of yours") : "none of yours"}</span>{/if}
              <span class="chev" aria-hidden="true">{isOpen ? "▴" : "▾"}</span>
            </button>
            {#if isOpen}
              <div class="detail">
                <p>{t.description}</p>
                {#if examples(t.key).length}
                  <span class="label">In your library</span>
                  <ul class="examples">
                    {#each examples(t.key) as e (e.info.id)}<li>{e.info.title}</li>{/each}
                  </ul>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      </ol>
    </div>
  </section>

  <div class="two">
    <section class="card forged block">
      <span class="label">3 · Inside a tier</span>
      <p class="muted intro">Within a tier the same idea repeats, by the mod's role:</p>
      <ol class="roles">
        {#each roles as r, i (r.key)}
          <li>
            <strong>{r.name}</strong>
            <small class="muted">{r.hint}</small>
          </li>
          {#if i < roles.length - 1}<li class="arrow" aria-hidden="true">›</li>{/if}
        {/each}
      </ol>
      <p class="faint small">Anything still tied goes by pack name, like Kaedrin does, so the same mods always give the same order.</p>
    </section>

    <section class="card forged block">
      <span class="label">4 · Rules that move a mod</span>
      <ul class="rules">
        <li>
          <strong>Needs and patches.</strong>
          <span class="muted">A mod always sits directly above the mods it needs or patches, even if its tier says lower. The load order shows "moved by rules" when that happens.</span>
        </li>
        <li>
          <strong>Contradictions.</strong>
          <span class="muted">If rules go in a circle (A above B above A), W.A.R.P. says so and falls back to the tiers.</span>
        </li>
        <li>
          <strong>Collisions.</strong>
          <span class="muted">A profile's Conflicts tab shows every pair of mods that ship the same files, who wins, and how risky it is.</span>
        </li>
      </ul>
    </section>
  </div>

  <section class="card forged block">
    <span class="label">5 · Where a mod's tier comes from</span>
    <ol class="sources">
      <li><strong class="you">You</strong><small class="muted">Your own choice always wins.</small></li>
      <li class="arrow" aria-hidden="true">›</li>
      <li><strong>Community list</strong><small class="muted">Hundreds of mods sorted by hand.</small></li>
      <li class="arrow" aria-hidden="true">›</li>
      <li><strong class="guess">Guess</strong><small class="muted">Read from what's inside the packs, or the Steam tags.</small></li>
      <li class="arrow" aria-hidden="true">›</li>
      <li><strong class="default">Default</strong><small class="muted">Nothing known: Core, at the bottom.</small></li>
    </ol>
    <div class="row">
      <button onclick={() => onnavigate("library")}><WarpShard size={14} /> Review tiers in the Library</button>
      <span class="faint small">Sort by the From column to see all guesses together; change any tier with its dropdown.</span>
    </div>
  </section>
</div>

<style>
  .guide {
    height: 100%;
    padding: 22px 28px 40px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1040px;
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
    gap: 10px;
  }

  .intro,
  .block p {
    margin: 0;
  }

  .small {
    font-size: 12.5px;
  }

  /* 1: two packs, one file, the top one used. */
  .wins {
    display: grid;
    grid-template-columns: minmax(280px, 1fr) 1.3fr;
    gap: 18px;
    align-items: center;
  }

  .collision {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pack {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px 10px;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: rgb(0 0 0 / 0.25);
    opacity: 0.6;
  }

  .pack.top {
    border-color: var(--accent-dim);
    background: var(--ok-dim);
    box-shadow: var(--glow-sm);
    opacity: 1;
  }

  .pack .mono {
    grid-column: 1;
    font-size: 11.5px;
    color: var(--muted);
  }

  .verdict {
    grid-row: 1 / span 2;
    grid-column: 2;
    align-self: center;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .verdict.ok {
    color: var(--accent);
  }

  .verdict.lost {
    color: var(--danger);
    text-decoration: line-through;
  }

  /* 2: the stack. */
  .stack {
    display: grid;
    grid-template-columns: 44px 1fr;
    gap: 10px;
  }

  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .rail .end {
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .rail .top {
    color: var(--accent);
  }

  .rail .bottom {
    color: var(--brass);
  }

  .rail .line {
    flex: 1;
    width: 3px;
    border-radius: 3px;
    background: linear-gradient(180deg, var(--accent), var(--brass) 70%, #9c4a2b);
    box-shadow: 0 0 10px var(--accent-glow);
  }

  .tiers {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tiers > li {
    border: 1px solid var(--border);
    border-left: 3px solid var(--tier);
    border-radius: var(--radius-sm);
    background: rgb(0 0 0 / 0.2);
  }

  .tiers > li.open {
    border-color: var(--border-strong);
    border-left-color: var(--tier);
    background: var(--surface-2);
  }

  .tier-row {
    width: 100%;
    display: grid;
    grid-template-columns: 22px 12px 150px 1fr auto 16px;
    align-items: center;
    gap: 10px;
    padding: 7px 12px;
    border: 0;
    background: transparent;
    text-align: left;
    white-space: normal;
  }

  .tier-row:hover .name {
    text-shadow: 0 0 10px var(--tier);
  }

  .num {
    font-size: 11px;
    color: var(--faint);
    text-align: right;
  }

  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    background: var(--tier);
    box-shadow: 0 0 8px var(--tier);
  }

  .name {
    font-family: var(--font-display);
    font-weight: var(--font-display-weight, 700);
    font-size: 17px;
    color: var(--tier);
  }

  .hint {
    font-size: 13px;
  }

  .n {
    font-size: 12px;
    white-space: nowrap;
  }

  .chev {
    font-size: 10px;
    color: var(--faint);
  }

  .detail {
    padding: 0 12px 12px 66px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13.5px;
  }

  .examples {
    margin: 0;
    padding-left: 18px;
    color: var(--muted);
    font-size: 13px;
  }

  /* 3 and 5: sequences. */
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  .roles,
  .sources {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: stretch;
    gap: 6px;
  }

  .roles li:not(.arrow),
  .sources li:not(.arrow) {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: rgb(0 0 0 / 0.2);
    max-width: 180px;
  }

  .roles strong,
  .sources strong {
    font-size: 13px;
  }

  .roles small,
  .sources small {
    font-size: 12px;
    line-height: 1.35;
  }

  .arrow {
    align-self: center;
    color: var(--accent);
    font-size: 18px;
    text-shadow: var(--glow-text);
  }

  .sources .you {
    color: var(--accent);
  }

  .sources .guess {
    color: var(--info);
  }

  .sources .default {
    color: var(--warn);
  }

  .rules {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13.5px;
  }

  .rules li {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
</style>
