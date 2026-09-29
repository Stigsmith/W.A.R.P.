// The playtest checklist on the start page, remembered in this window's local
// storage. Steps tick themselves when the app sees them happen; the tester can
// also tick them by hand. "Copy report" sends the checklist along.

import { api } from "./api";
import { app } from "./state.svelte";

export type StepKey = "sync" | "profile" | "conflicts" | "play" | "compare";

export const STEPS: { key: StepKey; title: string; how: string }[] = [
  { key: "sync", title: "Read your mods", how: "Press Read mods (or Sync with game)." },
  { key: "profile", title: "Make a profile", how: "Profiles → + New. Pick mods, or add all installed ones." },
  { key: "conflicts", title: "Look at Conflicts", how: "In a profile, open the Conflicts tab. Does it make sense?" },
  { key: "play", title: "Press ▶ Play", how: "Does Warhammer III start, with your mods on?" },
  { key: "compare", title: "Compare with a friend", how: "Multiplayer: swap share codes with whoever sent you W.A.R.P., then Compare." },
];

const STORAGE_KEY = "warp.playtest";

type Stored = { done: Partial<Record<StepKey, boolean>>; hidden: boolean; playWorked: boolean | null; notes: string };

class Playtest {
  done = $state<Partial<Record<StepKey, boolean>>>({});
  hidden = $state(false);
  /** The tester's answer to "did the game start with your mods?". */
  playWorked = $state<boolean | null>(null);
  notes = $state("");
  /** A report the clipboard wouldn't take, shown for copying by hand. */
  unsent = $state<string | null>(null);

  constructor() {
    try {
      const saved: Partial<Stored> = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
      this.done = saved.done ?? {};
      this.hidden = saved.hidden ?? false;
      this.playWorked = saved.playWorked ?? null;
      this.notes = saved.notes ?? "";
    } catch {
      // No storage: the checklist just starts empty each time.
    }
  }

  /** Done by hand, or plainly visible in the app's state. */
  isDone(key: StepKey): boolean {
    if (this.done[key]) return true;
    if (key === "sync") return app.library.some((e) => e.subscribed);
    if (key === "profile") return app.profiles.length > 0;
    return false;
  }

  mark(key: StepKey, on = true) {
    this.done = { ...this.done, [key]: on };
    this.save();
  }

  update(patch: Partial<Stored>) {
    Object.assign(this, patch);
    this.save();
  }

  save() {
    try {
      const data: Stored = { done: this.done, hidden: this.hidden, playWorked: this.playWorked, notes: this.notes };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch {
      // Not remembered, but fine for this session.
    }
  }

  text(): string {
    const lines = STEPS.map((s) => {
      let line = `  [${this.isDone(s.key) ? "x" : " "}] ${s.title}`;
      if (s.key === "play" && this.playWorked !== null) line += ` (game started with mods: ${this.playWorked ? "yes" : "NO"})`;
      return line;
    });
    const notes = this.notes.trim() ? `\nTester's notes:\n${this.notes.trim()}` : "";
    return `Playtest checklist:\n${lines.join("\n")}${notes}`;
  }
}

export const playtest = new Playtest();

/** Builds the report and puts it on the clipboard. True if it got there. */
export async function copyReport(): Promise<boolean> {
  const text = await app.attempt(async () => (await api()).diagnostics(playtest.text()));
  if (!text) return false;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Older webviews: copy through a hidden text box.
    const box = document.createElement("textarea");
    box.value = text;
    document.body.append(box);
    box.select();
    const ok = document.execCommand("copy");
    box.remove();
    if (!ok) {
      playtest.unsent = text;
      return false;
    }
  }
  app.notify("Report copied. Paste it to whoever sent you W.A.R.P. (Discord is fine).");
  return true;
}
