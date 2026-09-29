<script lang="ts">
  // For playtesters: what to try, and one button to send back what happened.
  import { STEPS, copyReport, playtest } from "../lib/playtest.svelte";
  import WarpShard from "./WarpShard.svelte";

  let copying = $state(false);

  const doneCount = $derived(STEPS.filter((s) => playtest.isDone(s.key)).length);

  async function copy() {
    copying = true;
    await copyReport();
    copying = false;
  }
</script>

<section class="card forged playtest" aria-label="Playtest checklist">
  <header>
    <span class="sigil"><WarpShard size={24} pulse /></span>
    <div class="intro">
      <span class="label">Playtest · {doneCount}/{STEPS.length} done</span>
      <p class="skaven">"A volunteer! Brave-brave. Try each thing, then send Snikkit's report to the man-thing who gave you W.A.R.P."</p>
      <p class="muted plain">You're testing W.A.R.P. Work through these, then press Copy report and paste it to whoever sent it to you.</p>
    </div>
    <button class="ghost small hide" onclick={() => playtest.update({ hidden: true })} title="Bring it back from Settings">Hide</button>
  </header>

  <ol class="steps">
    {#each STEPS as s, i (s.key)}
      {@const done = playtest.isDone(s.key)}
      <li class:done>
        <label>
          <input type="checkbox" checked={done} onchange={(e) => playtest.mark(s.key, e.currentTarget.checked)} />
          <span class="num">{i + 1}</span>
          <span class="text">
            <strong>{s.title}</strong>
            <small class="muted">{s.how}</small>
          </span>
        </label>
        {#if s.key === "play" && done}
          <div class="answer">
            <span class="faint">Did it start with your mods?</span>
            <button class="small" class:yes={playtest.playWorked === true} onclick={() => playtest.update({ playWorked: true })}>Yes</button>
            <button class="small" class:no={playtest.playWorked === false} onclick={() => playtest.update({ playWorked: false })}>No</button>
          </div>
        {/if}
      </li>
    {/each}
  </ol>

  <div class="foot">
    <textarea
      rows="2"
      placeholder="Anything odd, confusing or broken? Write it here; it goes into the report."
      value={playtest.notes}
      oninput={(e) => playtest.update({ notes: e.currentTarget.value })}
    ></textarea>
    <button class="primary" onclick={copy} disabled={copying}>{copying ? "Gathering…" : "Copy report"}</button>
  </div>
</section>

<style>
  .playtest {
    width: min(980px, 100%);
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-color: var(--accent-dim);
    box-shadow: var(--glow-sm);
  }

  header {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }

  .sigil {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    flex: none;
    border-radius: 50%;
    border: 1px solid var(--brass-dim);
    background: radial-gradient(circle, rgb(168 242 63 / 0.18), transparent 70%);
  }

  .intro {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .intro p {
    margin: 0;
  }

  .plain {
    font-size: 13px;
  }

  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 8px;
  }

  .steps li {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: rgb(0 0 0 / 0.2);
  }

  .steps li.done {
    border-color: var(--accent-dim);
    background: var(--ok-dim);
  }

  label {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    cursor: pointer;
  }

  label input {
    margin-top: 3px;
  }

  .num {
    display: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .text strong {
    font-size: 13px;
  }

  .done .text strong {
    color: var(--accent);
  }

  .text small {
    font-size: 12px;
    line-height: 1.35;
  }

  .answer {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
    font-size: 12px;
  }

  .answer .yes {
    border-color: var(--accent);
    color: var(--accent);
  }

  .answer .no {
    border-color: var(--danger);
    color: var(--danger);
  }

  .foot {
    display: flex;
    gap: 10px;
    align-items: stretch;
  }

  textarea {
    flex: 1;
    resize: vertical;
    min-height: 40px;
    font: inherit;
    font-size: 13px;
  }
</style>
