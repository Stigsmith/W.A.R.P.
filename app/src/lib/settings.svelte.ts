// Personal display preferences, remembered in this window's local storage.
// Nothing here matters to the backend, so it never leaves the UI.

export type HeadingFont = { key: string; name: string; family: string; weight: number; note: string };

export const HEADING_FONTS: HeadingFont[] = [
  { key: "grenze", name: "Grenze Gotisch", family: '"Grenze Gotisch"', weight: 700, note: "Gothic ironwork. The default." },
  { key: "pirata", name: "Pirata One", family: '"Pirata One"', weight: 400, note: "Spiky blackletter, like a Skaven banner." },
  { key: "rocker", name: "New Rocker", family: '"New Rocker"', weight: 400, note: "Heavy-metal gothic." },
  { key: "metamorphous", name: "Metamorphous", family: "Metamorphous", weight: 400, note: "Old World script. Easiest to read." },
];

const STORAGE_KEY = "warp.settings";

type Stored = { headingFont: string; motes: boolean; tourDone: boolean };

class Settings {
  headingFont = $state(HEADING_FONTS[0].key);
  /** Floating warpstone motes and fog. */
  motes = $state(true);
  tourDone = $state(false);

  constructor() {
    try {
      const saved: Partial<Stored> = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
      if (HEADING_FONTS.some((f) => f.key === saved.headingFont)) this.headingFont = saved.headingFont!;
      if (typeof saved.motes === "boolean") this.motes = saved.motes;
      if (typeof saved.tourDone === "boolean") this.tourDone = saved.tourDone;
    } catch {
      // No storage (private window, blocked): defaults it is.
    }
    this.apply();
  }

  font(): HeadingFont {
    return HEADING_FONTS.find((f) => f.key === this.headingFont) ?? HEADING_FONTS[0];
  }

  set<K extends keyof Stored>(key: K, value: Stored[K]) {
    (this as unknown as Stored)[key] = value;
    this.apply();
    try {
      const data: Stored = { headingFont: this.headingFont, motes: this.motes, tourDone: this.tourDone };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch {
      // Not remembered, but still applied for this session.
    }
  }

  apply() {
    const f = this.font();
    const root = document.documentElement.style;
    root.setProperty("--font-display", `${f.family}, "Segoe UI", serif`);
    root.setProperty("--font-display-weight", String(f.weight));
  }
}

export const settings = new Settings();
