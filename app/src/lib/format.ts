import type { LibraryEntry, WorkshopId } from "./types";

const dateFmt = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });

export function date(unix: number): string {
  return unix > 0 ? dateFmt.format(new Date(unix * 1000)) : "—";
}

const dayFmtUtc = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });

/** "not updated for 9.0", for a mod kept up to date until the latest game update but not since. */
export function notUpdatedLabel(e: LibraryEntry): string {
  return e.not_updated ? `not updated for ${e.not_updated.patch}` : "";
}

/** Why a mod is flagged as not updated, for its hover text. */
export function notUpdatedHint(e: LibraryEntry): string {
  const n = e.not_updated;
  if (!n) return "";
  const updated = Math.max(e.info.time_updated, e.installed_version ?? 0);
  return (
    `Kept up to date during ${n.previous.split(".")[0]}.x (last update ${date(updated)}), but not updated since game ` +
    `update ${n.patch} came out on ${dayFmtUtc.format(new Date(n.released * 1000))}. Until its author updates it, ` +
    "it's a likely suspect if the game crashes."
  );
}

export function size(bytes: number): string {
  if (bytes <= 0) return "—";
  const mb = bytes / 1_048_576;
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** A long mod title cut down to fit on a button. */
export function short(title: string, max = 30): string {
  return title.length <= max ? title : `${title.slice(0, max - 1).trimEnd()}…`;
}

/** Two titles cut down, but no further than they can still be told apart. */
export function shortPair(a: string, b: string, max = 30): [string, string] {
  while (max < 80 && short(a, max) === short(b, max)) max += 10;
  return [short(a, max), short(b, max)];
}

/** Steam page inside the Steam client (where Subscribe works). */
export function steamClientUrl(id: WorkshopId): string {
  return `steam://url/CommunityFilePage/${id}`;
}

export function workshopUrl(id: WorkshopId): string {
  return `https://steamcommunity.com/sharedfiles/filedetails/?id=${id}`;
}

/**
 * Colour for a tier band: warpstone green at the top of the load order, through
 * brass, down to rust at the foundation.
 */
export function tierColor(priority: number, maxPriority: number): string {
  const t = maxPriority > 0 ? Math.min(1, Math.max(0, priority / maxPriority)) : 0;
  // [position, hue, saturation, lightness]
  const stops: [number, number, number, number][] = [
    [0, 14, 58, 47], // rust
    [0.45, 38, 52, 52], // brass
    [1, 86, 84, 58], // warpstone
  ];
  const i = t <= stops[1][0] ? 0 : 1;
  const [p0, h0, s0, l0] = stops[i];
  const [p1, h1, s1, l1] = stops[i + 1];
  const f = (t - p0) / (p1 - p0);
  const mix = (a: number, b: number) => (a + (b - a) * f).toFixed(0);
  return `hsl(${mix(h0, h1)} ${mix(s0, s1)}% ${mix(l0, l1)}%)`;
}
