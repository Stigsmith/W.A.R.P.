// What's new and what's next, shown on the "What's new" page. Keep the newest
// release first, and its version equal to the app's (tauri.conf.json).

export type Release = { version: string; date: string; name: string; changes: string[] };

export type RoadmapItem = { title: string; what: string };

export const RELEASES: Release[] = [
  {
    version: "0.2.3",
    date: "2026-09-29",
    name: "Crash suspects",
    changes: [
      "Mods that were kept up to date for 8.x but haven't been updated since game update 9.0 get a \"not updated for 9.0\" flag: on the load order, in the Library, and in a warning on the profile, which can leave them all out in one click. When the game crashes and the load order isn't to blame, look here first. Mods that were already older aren't flagged: they came through 8.0 unchanged.",
      "The Library can show only those mods, so you can take them out of your sets or check their Workshop pages for news.",
    ],
  },
  {
    version: "0.2.2",
    date: "2026-09-29",
    name: "No detours",
    changes: [
      "Play goes straight into the game. After a game patch, Steam used to intercept it, ask about \"custom arguments\" and open CA's launcher with nothing ticked. W.A.R.P. now leaves Valve's steam_appid.txt next to the game, so the game runs on its own. Steam still sees you playing.",
      "W.A.R.P. is now published as Stigsmith. Coming from 0.2.1? Uninstall it first (Windows Settings, Apps), then install this one. Your sets and profiles stay; your font choices, the tour and the playtest checklist start fresh once, so copy your playtest notes first.",
    ],
  },
  {
    version: "0.2.1",
    date: "2026-09-29",
    name: "Rat-hunting season",
    changes: [
      "Rat-hunting (what lesser races call bug-fixing): the mod picker no longer opens somewhere far below the window, the set menu closes when you click elsewhere and never opens off-screen, and \"Show only its mods\" has a way back. More rats to come. There are always more rats.",
      "Your lists are safe: W.A.R.P. backs up your library each time it starts (the last seven are kept), checks the database before using it, and offers to restore a backup if it's damaged or suddenly empty. Settings lists every backup.",
      "Only one W.A.R.P. at a time: starting it again brings the open window forward.",
      "The tour no longer explains an empty page: without a profile it skips that part, and shows it once your first profile has mods.",
      "The start page fits on one screen: the logo makes room.",
      "A hidden playtest checklist can be brought back from the start page.",
      "Snikkit has opinions about campaigns that aren't Skaven.",
    ],
  },
  {
    version: "0.2.0",
    date: "2026-09-29",
    name: "The playtest build",
    changes: [
      "Sets as a spreadsheet: in the Library, tick mods into a column per set, drag down a column to tick many, and edit whole selections at once.",
      "Profiles keep their own copy of each set and ask before taking a set's changes: update, dismiss, later, or stop asking.",
      "Mods the community list doesn't know get a guessed tier, read from what's inside their packs. Hover \"guess\" to see why.",
      "A warning when two installed mods look like two versions of the same mod (SFO and vanilla editions, a compilation and its parts).",
      "A Load order page that explains every tier, a tier picker with hints, and a sortable Library with a From column.",
      "A playtest checklist, Copy report for sending feedback, a log file, and a friendly screen instead of a crash if something breaks at startup.",
      "Pick your own fonts for headings, text and Snikkit's voice. Buttons and checkboxes are now warpstone behind frosted glass.",
      "A proper Windows installer. No admin rights needed.",
    ],
  },
  {
    version: "0.1.0",
    date: "2026-09-28",
    name: "First light",
    changes: [
      "The load-order engine: tiers and roles put specific mods on top and foundations at the bottom; mods that need or patch another sit above it.",
      "Multiplayer: share your list as a short code, paste a friend's, and see exactly what's missing, extra, out of order or outdated.",
      "Reads your installed mods from Steam and looks inside every pack.",
      "A conflict map: which mods ship the same files, who wins, and how risky it is.",
      "Play: starts Warhammer III with your profile's load order.",
      "Imports the old W.A.R.P. v1 spreadsheet.",
      "The start page, Snikkit's tour, and the Skaven look.",
    ],
  },
];

export const ROADMAP: { stage: "now" | "next" | "later"; label: string; items: RoadmapItem[] }[] = [
  {
    stage: "now",
    label: "Now",
    items: [
      {
        title: "Playtesting",
        what: "The first run on other people's PCs: does Play start the game with the mods, and does comparing lists with a friend work? Your Copy report helps.",
      },
    ],
  },
  {
    stage: "next",
    label: "Next",
    items: [
      { title: "Creative Assembly's mod manager", what: "Import and export profiles of the official launcher's mod manager." },
      { title: "Share your tier fixes", what: "Send the tiers you corrected to the community list, so everyone's guesses get better." },
      { title: "Keep A above B", what: "Pin one mod above another by hand, for the rare cases the tiers get wrong." },
    ],
  },
  {
    stage: "later",
    label: "Later",
    items: [
      { title: "Crash helper", what: "Remember the last list that worked, and narrow down which mod makes the game crash." },
      { title: "Community list updates", what: "Get new tiers and relations without installing a new version." },
      { title: "A signed installer", what: "No more \"Windows protected your PC\" warning." },
    ],
  },
];
