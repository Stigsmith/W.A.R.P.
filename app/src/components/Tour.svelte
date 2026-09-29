<script lang="ts">
  // The guided tour. Warlock-Engineer Snikkit explains the app in Skaven speech,
  // with a plain-words line under each step. Steps point at elements marked
  // `data-tour="<target>"`; if one isn't on screen, the step is shown centred.
  // Steps about things the user doesn't have yet (a profile, any mods) are left
  // out, so Snikkit never explains an empty page.
  import { onMount, tick, untrack } from "svelte";
  import { app } from "../lib/state.svelte";
  import { settings } from "../lib/settings.svelte";
  import type { View } from "../lib/views";
  import WarpShard from "./WarpShard.svelte";

  type Step = { target?: string; view?: View; title: string; speech: string; plain: string };

  let {
    part = "full",
    onview,
    onclose,
  }: {
    /** "profile" is the follow-up for a tour that ran before there was a profile. */
    part?: "full" | "profile";
    onview: (view: View) => void;
    onclose: () => void;
  } = $props();

  // Decided once, when the tour starts.
  const hasMods = untrack(() => app.library.length > 0);
  const hasProfile = untrack(() => app.profiles.length > 0);

  // A profile's load order, conflicts and Play. The full tour shows them when there
  // is a profile; otherwise they come as a follow-up once the first one exists.
  const profileSteps: Step[] = [
    {
      target: "nav-profiles",
      view: "profiles",
      title: "War-lists",
      speech:
        "Here you build war-lists: stack your sets, add a few single mod-things, leave some out. My engine sorts them: small clever things on top, big heavy foundations at the bottom. Like a proper Skaven hierarchy. Me on top.",
      plain:
        "A profile is a stack of sets plus single mods you pick. WARP works out the load order. When you change a set later, profiles using it ask before taking the change.",
    },
    {
      target: "order-tab",
      view: "profiles",
      title: "Every rat in its place",
      speech:
        "See? Every mod-thing has its place, and Snikkit tells you WHY. Click one! No more guessing-guessing like a dim-witted clanrat.",
      plain: "Click any mod in the load order to see why it sits where it does. The Load order page in the menu explains every tier.",
    },
    {
      target: "conflicts-tab",
      view: "profiles",
      title: "Pack-fights",
      speech:
        "When two packs fight over the same file, only the top one survives. The other gets eaten! I show you which fights matter and which are only pretty-pretty paint.",
      plain: "Conflicts lists mods that overwrite each other, and how risky each overlap is.",
    },
    {
      target: "play",
      view: "profiles",
      title: "The glowing button",
      speech:
        "Press the warpstone button and the game starts with your war-list, exactly as sorted. Do not lick the button. The last apprentice licked the button.",
      plain: "Play writes the load order and starts Warhammer III with it.",
    },
  ];

  const all: (Step | false)[] = [
    {
      view: "home",
      title: "Yes-yes, welcome!",
      speech:
        "Welcome, man-thing, to W.A.R.P.! I am Warlock-Engineer Snikkit, most-clever servant of the Horned Rat, and this is my magnificent machine. Listen-listen, quick-quick. My time is very valuable.",
      plain: "A short tour of W.A.R.P. Skip it any time with Esc.",
    },
    {
      target: "sync",
      title: "The Sync-lever",
      speech:
        "Pull this, and my machine sniffs out every mod-thing Steam has buried in your burrow, then peeks inside every pack-scroll. Nothing escapes my whiskers. Nothing!",
      plain: "Sync reads your installed mods and what's inside each one. Do it after subscribing to new mods.",
    },
    hasMods && {
      target: "tier-col",
      view: "library",
      title: "Sniffed-out guesses",
      speech:
        "Most mod-things, the clan has already sorted. The rest? Snikkit sniffs inside every pack! Many animation-scrolls: animations. Mostly UI pictures: UI. Unit tables: units. Right two times in three... and the third time, you fix it. Yes-yes, even Snikkit is not perfect. Almost.",
      plain:
        "Tiers come from a community list of sorted mods. For mods not on it, W.A.R.P. guesses from what's inside the packs, or from their Steam tags; those say \"guess\". Hover to see why, and pick another tier in the dropdown if it's wrong. Sort by the From column to see all guesses together.",
    },
    hasMods && {
      target: "sets-mode",
      view: "library",
      title: "Clan sets",
      speech:
        "Sort your mod-things into sets, like a clever engineer sorts his warp-parts! One set for the basics, one for each faction you conquer with. Skaven campaign? Make a Skaven set, yes-yes! Another campaign? THERE IS NO OTHER CAMPAIGN! Skaven or nothing, man-thing! ...Fine-fine. If you MUST play lesser races, leave the Skaven set out. No picking through a hundred mods like a starving clanrat.",
      plain:
        "Sets are groups of mods. In the Library, switch to Sets and tick mods into a set's column; drag down a column to tick many. Tip: make one set per faction campaign, so you can leave it out of the next one.",
    },
    !hasProfile && {
      target: "new-profile",
      view: "profiles",
      title: "Your first war-list",
      speech:
        "No war-list yet?! A warlord without a war-list is just a rat with a hat. Press New, name it, pick your mod-things. Then Snikkit shows you the clever parts: the sorting, the pack-fights, the glowing button.",
      plain:
        "A profile is a stack of sets plus single mods you pick, and W.A.R.P. works out the load order. Press + New to make one, then replay this tour from Settings to see the rest.",
    },
    ...(hasProfile ? profileSteps : []),
    {
      target: "nav-multiplayer",
      view: "multiplayer",
      title: "Scheming with allies",
      speech:
        "Playing with another warlord? Share your list-code, paste theirs, and I find every difference: missing, extra, wrong order, stale version. No more reading sixty names aloud like fools-fools!",
      plain: "Multiplayer compares your list with a friend's and tells you exactly what to fix.",
    },
    {
      target: "kofi",
      view: "home",
      title: "Warpstone for science",
      speech:
        "My experiments need warpstone. Much-much warpstone. If W.A.R.P. saves you from crashes, feed the Warlock-Engineer a tip. For science! Not for eating. Mostly.",
      plain: "W.A.R.P. is free. Tips on Ko-fi keep it going.",
    },
    {
      view: "home",
      title: "Go, conquer!",
      speech: "Now go, man-thing. Conquer! And if anything explodes... it was not Snikkit's fault. Yes-yes.",
      plain: hasProfile
        ? "You can replay this tour from Settings."
        : "Once you've made a profile, replay this tour from Settings to see its load order, conflicts and Play.",
    },
  ];
  const followUp: Step[] = [
    {
      view: "profiles",
      title: "A war-list! At last!",
      speech:
        "You made a war-list! Snikkit is almost proud. Now, as promised, the clever parts. Snikkit always keeps his promises. Mostly.",
      plain: "The rest of the tour: what you can do with a profile.",
    },
    ...profileSteps,
    {
      view: "profiles",
      title: "Go, conquer!",
      speech: "Now you know everything. Well, everything Snikkit allows. Go-go, conquer!",
      plain: "You can replay the whole tour from Settings.",
    },
  ];
  const steps = untrack(() => part) === "profile" ? followUp : all.filter((s): s is Step => !!s);

  let index = $state(0);
  let rect = $state<DOMRect | null>(null);
  const step = $derived(steps[index]);

  async function locate() {
    if (step.view) onview(step.view);
    await tick();
    // Views load data asynchronously; give the target a moment to appear.
    for (let i = 0; i < 12; i++) {
      const el = step.target ? document.querySelector<HTMLElement>(`[data-tour="${step.target}"]`) : null;
      if (el || !step.target) {
        el?.scrollIntoView({ block: "nearest" });
        rect = el ? el.getBoundingClientRect() : null;
        return;
      }
      await new Promise((r) => setTimeout(r, 80));
    }
    rect = null;
  }

  $effect(() => {
    void index;
    locate();
  });

  // Snikkit promised to show the profile part once there is a profile.
  $effect(() => {
    if (step.target === "new-profile") untrack(() => settings.set("profileTourPending", true));
  });

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onclose();
      if (e.key === "ArrowRight" || e.key === "Enter") next();
      if (e.key === "ArrowLeft") back();
    };
    const onResize = () => locate();
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onResize);
    };
  });

  function next() {
    if (index < steps.length - 1) index++;
    else onclose();
  }

  function back() {
    if (index > 0) index--;
  }

  // Place the card beside the highlighted element, or centred when there is none.
  const cardStyle = $derived.by(() => {
    if (!rect) return "left: 50%; top: 50%; transform: translate(-50%, -50%);";
    const width = 420;
    const gap = 18;
    const right = rect.right + gap + width < window.innerWidth;
    const left = right ? rect.right + gap : Math.max(16, Math.min(rect.left, window.innerWidth - width - 16));
    const top = right
      ? Math.max(16, Math.min(rect.top - 10, window.innerHeight - 300))
      : rect.bottom + gap + 260 < window.innerHeight
        ? rect.bottom + gap
        : Math.max(16, rect.top - 280);
    return `left: ${left}px; top: ${top}px;`;
  });
</script>

<div class="tour" role="dialog" aria-modal="true" aria-label="Tour">
  {#if rect}
    <div
      class="spot"
      style:left="{rect.left - 8}px"
      style:top="{rect.top - 8}px"
      style:width="{rect.width + 16}px"
      style:height="{rect.height + 16}px"
    ></div>
  {:else}
    <div class="dim"></div>
  {/if}

  <div class="card forged bubble" style={cardStyle}>
    <div class="head">
      <span class="sigil"><WarpShard size={30} pulse /></span>
      <div>
        <span class="label">Warlock-Engineer Snikkit · {index + 1}/{steps.length}</span>
        <h3>{step.title}</h3>
      </div>
    </div>
    <p class="skaven">"{step.speech}"</p>
    <p class="plain muted">{step.plain}</p>
    <div class="actions">
      <button class="ghost small" onclick={onclose}>Skip tour</button>
      <span class="spacer"></span>
      {#if index > 0}<button class="small" onclick={back}>Back</button>{/if}
      <button class="primary small" onclick={next}>{index < steps.length - 1 ? "Next, yes-yes!" : "Go, conquer!"}</button>
    </div>
  </div>
</div>

<style>
  .tour {
    position: fixed;
    inset: 0;
    z-index: 100;
  }

  .dim {
    position: absolute;
    inset: 0;
    background: rgb(0 0 0 / 0.74);
  }

  /* The spotlight: everything else goes dark; the target gets a warpstone ring. */
  .spot {
    position: absolute;
    border-radius: 10px;
    box-shadow:
      0 0 0 9999px rgb(0 0 0 / 0.74),
      0 0 0 2px var(--accent),
      0 0 30px var(--accent-glow);
    transition: all 0.28s ease;
    pointer-events: none;
    animation: spot-pulse 1.8s ease-in-out infinite;
  }

  @keyframes spot-pulse {
    50% {
      box-shadow:
        0 0 0 9999px rgb(0 0 0 / 0.74),
        0 0 0 2px var(--accent-strong),
        0 0 46px rgb(168 242 63 / 0.55);
    }
  }

  .bubble {
    position: absolute;
    width: 420px;
    max-width: calc(100vw - 32px);
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: rgb(12 16 10 / 0.96);
    transition:
      left 0.28s ease,
      top 0.28s ease;
  }

  .head {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .sigil {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: 1px solid var(--brass-dim);
    background: radial-gradient(circle, rgb(168 242 63 / 0.18), transparent 70%);
  }

  .skaven {
    color: var(--text);
  }

  .plain {
    font-size: 13px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .spacer {
    flex: 1;
  }
</style>
