<script lang="ts">
  // The living background: fog rolling along the bottom and warpstone motes drifting
  // up through the dark. Drawn on one canvas at ~30 fps; sleeps when the window is
  // hidden (e.g. while you play) and stays still for people who prefer less motion.
  import { onMount } from "svelte";

  let { motes = true }: { motes?: boolean } = $props();

  let canvas: HTMLCanvasElement;
  let restart: (() => void) | null = null;

  $effect(() => {
    void motes;
    restart?.();
  });

  type Mote = { x: number; y: number; r: number; vy: number; drift: number; phase: number; twinkle: number; hue: number; shard: boolean };

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let w = 0;
    let h = 0;
    let dpr = 1;
    let list: Mote[] = [];
    let raf = 0;
    let last = 0;

    // One pre-rendered glow, stamped for every mote: much cheaper than gradients per frame.
    const sprite = document.createElement("canvas");
    sprite.width = sprite.height = 64;
    const s = sprite.getContext("2d")!;
    const g = s.createRadialGradient(32, 32, 0, 32, 32, 32);
    g.addColorStop(0, "rgba(236,255,190,1)");
    g.addColorStop(0.18, "rgba(190,255,95,0.9)");
    g.addColorStop(0.45, "rgba(150,235,50,0.28)");
    g.addColorStop(1, "rgba(120,220,40,0)");
    s.fillStyle = g;
    s.fillRect(0, 0, 64, 64);

    const spawn = (anywhere: boolean): Mote => {
      const shard = Math.random() < 0.08;
      return {
        x: Math.random() * w,
        y: anywhere ? Math.random() * h : h + 20 + Math.random() * 60,
        r: shard ? 2.6 + Math.random() * 2 : 0.7 + Math.random() * 1.8,
        vy: -(0.12 + Math.random() * 0.4) * (shard ? 0.6 : 1),
        drift: 0.15 + Math.random() * 0.5,
        phase: Math.random() * Math.PI * 2,
        twinkle: 0.4 + Math.random() * 1.4,
        hue: Math.random(),
        shard,
      };
    };

    const resize = () => {
      dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      w = window.innerWidth;
      h = window.innerHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const target = Math.round((w * h) / 21000);
      while (list.length < target) list.push(spawn(true));
      list.length = target;
    };

    const draw = (t: number) => {
      ctx.clearRect(0, 0, w, h);
      ctx.globalCompositeOperation = "lighter";
      for (const m of list) {
        // Brighter near the fog at the bottom, fading as they rise.
        const height = 1 - Math.max(0, Math.min(1, m.y / h));
        const glow = (0.55 + 0.45 * Math.sin(t * 0.001 * m.twinkle + m.phase)) * (0.35 + 0.65 * (1 - height * 0.7));
        const size = m.r * (m.shard ? 9 : 7);
        ctx.globalAlpha = Math.max(0, Math.min(1, glow * (m.shard ? 0.95 : 0.7)));
        ctx.drawImage(sprite, m.x - size / 2, m.y - size / 2, size, size);
      }
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "source-over";
    };

    const step = (t: number) => {
      raf = requestAnimationFrame(step);
      if (t - last < 33) return; // ~30 fps is plenty for drifting dust
      const dt = Math.min(3, (t - last) / 16.7);
      last = t;
      for (let i = 0; i < list.length; i++) {
        const m = list[i];
        m.y += m.vy * dt;
        m.x += Math.sin(t * 0.0004 * m.drift + m.phase) * 0.18 * dt;
        if (m.y < -30) list[i] = spawn(false);
      }
      draw(t);
    };

    const start = () => {
      cancelAnimationFrame(raf);
      if (!motes) {
        ctx.clearRect(0, 0, w, h);
        return;
      }
      if (still || document.hidden) draw(performance.now());
      else raf = requestAnimationFrame(step);
    };

    resize();
    start();
    const onResize = () => {
      resize();
      start();
    };
    const onVisibility = () => (document.hidden ? cancelAnimationFrame(raf) : start());
    window.addEventListener("resize", onResize);
    document.addEventListener("visibilitychange", onVisibility);
    restart = start;

    return () => {
      restart = null;
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  });
</script>

<div class="backdrop" aria-hidden="true">
  <div class="warp-sky"></div>
  <canvas bind:this={canvas}></canvas>
  <div class="fog fog-back"></div>
  <div class="fog fog-front"></div>
  <div class="vignette"></div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
    background: var(--bg);
  }

  /* A warp storm brewing far off in the top right, like the lightning in the art. */
  .warp-sky {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(900px 480px at 96% -6%, rgb(168 242 63 / 0.13), transparent 62%),
      radial-gradient(520px 300px at 80% 8%, rgb(120 200 40 / 0.07), transparent 70%),
      radial-gradient(800px 520px at -8% 108%, rgb(207 159 77 / 0.07), transparent 60%);
    animation: storm 11s ease-in-out infinite alternate;
  }

  @keyframes storm {
    to {
      opacity: 0.7;
    }
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  /* Green-grey fog rolling along the floor of the burrow. */
  .fog {
    position: absolute;
    left: -30%;
    width: 160%;
    bottom: -8vh;
    height: 46vh;
    filter: blur(24px);
  }

  .fog-back {
    background:
      radial-gradient(40% 55% at 22% 80%, rgb(120 160 80 / 0.16), transparent 70%),
      radial-gradient(35% 50% at 58% 85%, rgb(100 140 70 / 0.14), transparent 70%),
      radial-gradient(38% 60% at 88% 78%, rgb(130 170 90 / 0.13), transparent 70%);
    animation: drift 70s ease-in-out infinite alternate;
  }

  .fog-front {
    height: 32vh;
    background:
      radial-gradient(30% 60% at 12% 90%, rgb(168 242 63 / 0.09), transparent 70%),
      radial-gradient(34% 55% at 46% 95%, rgb(140 190 80 / 0.14), transparent 72%),
      radial-gradient(30% 60% at 78% 92%, rgb(168 242 63 / 0.08), transparent 70%);
    animation: drift 46s ease-in-out infinite alternate-reverse;
  }

  @keyframes drift {
    from {
      transform: translateX(-6%);
    }
    to {
      transform: translateX(6%);
    }
  }

  .vignette {
    position: absolute;
    inset: 0;
    background: radial-gradient(120% 100% at 50% 45%, transparent 55%, rgb(0 0 0 / 0.55));
  }
</style>
