// Personal display preferences, remembered in this window's local storage.
// Nothing here matters to the backend, so it never leaves the UI.

export type HeadingFont = { key: string; name: string; family: string; weight: number; note: string };

export const HEADING_FONTS: HeadingFont[] = [
  { key: "grenze", name: "Grenze Gotisch", family: '"Grenze Gotisch"', weight: 700, note: "Gothic ironwork. The default." },
  { key: "pirata", name: "Pirata One", family: '"Pirata One"', weight: 400, note: "Spiky blackletter, like a Skaven banner." },
  { key: "rocker", name: "New Rocker", family: '"New Rocker"', weight: 400, note: "Heavy-metal gothic." },
  { key: "metamorphous", name: "Metamorphous", family: "Metamorphous", weight: 400, note: "Old World script. Easiest to read." },
];

/** Fonts for everything else: lists, tables, buttons. All calm enough to read all day. */
export type TextFont = { key: string; name: string; family: string; note: string };

export const TEXT_FONTS: TextFont[] = [
  { key: "segoe", name: "Segoe UI", family: '"Segoe UI Variable Text", "Segoe UI"', note: "Clean and plain. The default." },
  { key: "alegreya", name: "Alegreya Sans", family: '"Alegreya Sans"', note: "Old-world lettering, still easy on the eyes." },
  { key: "lora", name: "Lora", family: "Lora", note: "A book serif, like a well-kept tome." },
  { key: "barlow", name: "Barlow", family: "Barlow", note: "Industrial, like Skryre engineering plates." },
];

/** Fonts for the Warlock-Engineer's speech. Some are wider, so each has its own size. */
export type SpeechFont = { key: string; name: string; family: string; style: "italic" | "normal"; size: number; note: string };

export const SPEECH_FONTS: SpeechFont[] = [
  { key: "fell", name: "IM Fell English", family: '"IM Fell English"', style: "italic", size: 17, note: "An old printed pamphlet. The default." },
  { key: "almendra", name: "Almendra", family: "Almendra", style: "italic", size: 17, note: "A scribe's calligraphy." },
  { key: "uncial", name: "Uncial Antiqua", family: '"Uncial Antiqua"', style: "normal", size: 15, note: "Ancient rune-scrawl. Heavy, still readable." },
  { key: "sharp", name: "MedievalSharp", family: "MedievalSharp", style: "normal", size: 16, note: "Rough notes scratched by claw." },
];

const STORAGE_KEY = "warp.settings";

export type LibraryMode = "details" | "sets";

type Stored = { headingFont: string; textFont: string; speechFont: string; seenVersion: string; motes: boolean; tourDone: boolean; libraryMode: LibraryMode };

class Settings {
  headingFont = $state(HEADING_FONTS[0].key);
  speechFont = $state(SPEECH_FONTS[0].key);
  textFont = $state(TEXT_FONTS[0].key);
  /** The version whose "What's new" the user has opened. */
  seenVersion = $state("");
  /** Floating warpstone motes and fog. */
  motes = $state(true);
  tourDone = $state(false);
  /** The Library's last view: plain details, or a column per set. */
  libraryMode = $state<LibraryMode>("details");

  constructor() {
    try {
      const saved: Partial<Stored> = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
      if (HEADING_FONTS.some((f) => f.key === saved.headingFont)) this.headingFont = saved.headingFont!;
      if (SPEECH_FONTS.some((f) => f.key === saved.speechFont)) this.speechFont = saved.speechFont!;
      if (TEXT_FONTS.some((f) => f.key === saved.textFont)) this.textFont = saved.textFont!;
      if (typeof saved.seenVersion === "string") this.seenVersion = saved.seenVersion;
      if (typeof saved.motes === "boolean") this.motes = saved.motes;
      if (typeof saved.tourDone === "boolean") this.tourDone = saved.tourDone;
      if (saved.libraryMode === "sets" || saved.libraryMode === "details") this.libraryMode = saved.libraryMode;
    } catch {
      // No storage (private window, blocked): defaults it is.
    }
    this.apply();
  }

  font(): HeadingFont {
    return HEADING_FONTS.find((f) => f.key === this.headingFont) ?? HEADING_FONTS[0];
  }

  text(): TextFont {
    return TEXT_FONTS.find((f) => f.key === this.textFont) ?? TEXT_FONTS[0];
  }

  speech(): SpeechFont {
    return SPEECH_FONTS.find((f) => f.key === this.speechFont) ?? SPEECH_FONTS[0];
  }

  set<K extends keyof Stored>(key: K, value: Stored[K]) {
    (this as unknown as Stored)[key] = value;
    this.apply();
    try {
      const data: Stored = {
        headingFont: this.headingFont,
        speechFont: this.speechFont,
        textFont: this.textFont,
        seenVersion: this.seenVersion,
        motes: this.motes,
        tourDone: this.tourDone,
        libraryMode: this.libraryMode,
      };
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
    root.setProperty("--font", `${this.text().family}, system-ui, sans-serif`);
    const s = this.speech();
    root.setProperty("--font-skaven", `${s.family}, Georgia, serif`);
    root.setProperty("--font-skaven-style", s.style);
    root.setProperty("--font-skaven-size", `${s.size}px`);
  }
}

export const settings = new Settings();
