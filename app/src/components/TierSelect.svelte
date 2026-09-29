<script lang="ts">
  // A tier picker that explains itself: the tiers in load-order order, each in
  // its colour, and a few words about a tier after hovering it for half a second.
  import { onMount } from "svelte";
  import { app } from "../lib/state.svelte";
  import { tierColor } from "../lib/format";

  let {
    value,
    onpick,
    note = "",
    label = "Tier",
  }: {
    value: string;
    onpick: (key: string) => void;
    /** Extra words for the hover hint, e.g. that the whole selection changes. */
    note?: string;
    label?: string;
  } = $props();

  const HINT_DELAY_MS = 500;

  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();
  let pos = $state({ left: 0, top: 0, up: false });
  let active = $state(-1);
  let tip = $state<{ text: string; x: number; y: number; flip: boolean } | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const tiers = $derived([...app.taxonomy.tier].sort((a, b) => b.priority - a.priority));
  const maxPriority = $derived(Math.max(1, ...tiers.map((t) => t.priority)));
  const current = $derived(app.tier(value));
  const color = (priority: number) => tierColor(priority, maxPriority);

  /** Moves a node to <body>, so no scrolling table or panel can clip it. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function hintLater(text: string, el: HTMLElement, beside: boolean) {
    clearTimeout(timer);
    if (!text) return;
    timer = setTimeout(() => {
      const r = el.getBoundingClientRect();
      if (beside) {
        const flip = r.right + 280 > window.innerWidth;
        tip = { text, x: flip ? r.left - 10 : r.right + 10, y: r.top + r.height / 2, flip };
      } else {
        tip = { text, x: r.left, y: r.bottom + 6, flip: false };
      }
    }, HINT_DELAY_MS);
  }

  function hideHint() {
    clearTimeout(timer);
    tip = null;
  }

  function show() {
    if (!trigger) return;
    hideHint();
    const r = trigger.getBoundingClientRect();
    const up = r.bottom + 400 > window.innerHeight && r.top > 400;
    pos = { left: r.left, top: up ? r.top - 4 : r.bottom + 4, up };
    active = tiers.findIndex((t) => t.key === value);
    open = true;
  }

  function close() {
    open = false;
    hideHint();
  }

  function pick(key: string) {
    close();
    if (key !== value) onpick(key);
    trigger?.focus();
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (!open && (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      show();
    }
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!open) return;
      if (e.key === "Escape" || e.key === "Tab") close();
      else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        active = (active + (e.key === "ArrowDown" ? 1 : -1) + tiers.length) % tiers.length;
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (tiers[active]) pick(tiers[active].key);
      }
    };
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (open && !menu?.contains(t) && !trigger?.contains(t)) close();
    };
    const onScroll = (e: Event) => {
      if (open && !menu?.contains(e.target as Node)) close();
      else if (!open) hideHint();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", close);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", close);
    };
  });
</script>

<button
  bind:this={trigger}
  class="trigger"
  class:open
  type="button"
  style:--tier={color(current?.priority ?? 0)}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-label="{label}: {current?.name ?? value}"
  onclick={(e) => (e.stopPropagation(), open ? close() : show())}
  onkeydown={onTriggerKey}
  onpointerenter={(e) => !open && hintLater([current?.hint, note].filter(Boolean).join(" · "), e.currentTarget, false)}
  onpointerleave={hideHint}
>
  {current?.name ?? value}
  <span class="chev" aria-hidden="true">▾</span>
</button>

{#if open}
  <div
    use:portal
    bind:this={menu}
    class="menu card forged"
    class:up={pos.up}
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    role="listbox"
    aria-label={label}
  >
    <span class="edge top">wins collisions</span>
    {#each tiers as t, i (t.key)}
      <button
        type="button"
        role="option"
        aria-selected={t.key === value}
        class="opt"
        class:active={i === active}
        class:current={t.key === value}
        style:--tier={color(t.priority)}
        onclick={() => pick(t.key)}
        onpointerenter={(e) => ((active = i), hintLater(t.hint, e.currentTarget, true))}
        onpointerleave={hideHint}
      >
        <span class="swatch"></span>
        <span class="name">{t.name}</span>
        {#if t.key === value}<span class="tick" aria-hidden="true">●</span>{/if}
      </button>
    {/each}
    <span class="edge bottom">foundation</span>
  </div>
{/if}

{#if tip}
  <div use:portal class="tip" class:flip={tip.flip} class:beside={open} style:left="{tip.x}px" style:top="{tip.y}px" role="tooltip">
    {tip.text}
  </div>
{/if}

<style>
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 6px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--tier);
    font-weight: 600;
    font-size: 13px;
    white-space: nowrap;
    cursor: pointer;
  }

  .trigger:hover,
  .trigger:focus-visible,
  .trigger.open {
    background: rgb(8 11 7 / 0.75);
    border-color: var(--border-strong);
  }

  .chev {
    font-size: 10px;
    color: var(--faint);
  }

  .menu {
    position: fixed;
    z-index: 200;
    min-width: 190px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: rgb(10 14 9 / 0.98);
    box-shadow:
      0 12px 30px rgb(0 0 0 / 0.6),
      var(--glow-sm);
  }

  .menu.up {
    transform: translateY(-100%);
  }

  .edge {
    padding: 2px 8px;
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--faint);
  }

  .edge.top {
    color: var(--accent);
  }

  .edge.bottom {
    color: var(--brass);
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 5px 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .opt.active {
    background: var(--surface-2);
    border-color: var(--border);
  }

  .opt.current .name {
    color: var(--tier);
    font-weight: 600;
  }

  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: var(--tier);
    box-shadow: 0 0 6px var(--tier);
  }

  .name {
    flex: 1;
  }

  .tick {
    font-size: 8px;
    color: var(--tier);
  }

  .tip {
    position: fixed;
    z-index: 210;
    max-width: 260px;
    padding: 6px 10px;
    border: 1px solid var(--brass-dim);
    border-radius: var(--radius-sm);
    background: rgb(14 18 12 / 0.97);
    color: var(--text);
    font-size: 12.5px;
    line-height: 1.35;
    pointer-events: none;
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.5);
  }

  .tip.beside {
    transform: translateY(-50%);
  }

  .tip.beside.flip {
    transform: translate(-100%, -50%);
  }
</style>
