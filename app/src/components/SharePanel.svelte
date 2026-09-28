<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import type { ShareList } from "../lib/types";

  let { list, onclose }: { list: ShareList; onclose: () => void } = $props();

  let code = $state("");
  let copied = $state(false);
  const DISCORD_LIMIT = 2000;

  $effect(() => {
    api()
      .then((a) => a.encodeShare(list))
      .then((c) => (code = c));
  });

  async function copy() {
    await navigator.clipboard.writeText(code);
    copied = true;
    setTimeout(() => (copied = false), 1800);
  }

  async function saveFile() {
    const a = await api();
    const path = await a.pickSavePath(`${list.name}.warp`, [{ name: "W.A.R.P. modlist", extensions: ["warp"] }]);
    if (path && (await app.attempt(() => a.saveWarpFile(list, path))) !== undefined) {
      app.notify(`Saved ${path}`);
    }
  }
</script>

<div class="share card">
  <div class="head">
    <div>
      <h3>Share "{list.name}"</h3>
      <p class="muted">Send this code to a friend. Their WARP shows exactly what differs.</p>
    </div>
    <button class="ghost small" onclick={onclose} aria-label="Close">✕</button>
  </div>
  <textarea readonly rows="4" value={code || "…"} onfocus={(e) => e.currentTarget.select()}></textarea>
  <div class="actions">
    <span class="faint">
      {list.entries.length} packs · {code.length} characters
      {#if code.length > DISCORD_LIMIT}<span class="chip warn">too long for one Discord message - use a file</span>{/if}
    </span>
    <span class="spacer"></span>
    <button onclick={saveFile}>Save .warp file…</button>
    <button class="primary" onclick={copy} disabled={!code}>{copied ? "Copied" : "Copy code"}</button>
  </div>
</div>

<style>
  .share {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-color: var(--accent-dim);
    box-shadow: 0 0 30px var(--accent-glow);
  }

  .head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .head p {
    font-size: 13px;
    margin-top: 2px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
  }

  .spacer {
    flex: 1;
  }
</style>
