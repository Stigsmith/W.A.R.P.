import type { WorkshopId } from "./types";

const dateFmt = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });

export function date(unix: number): string {
  return unix > 0 ? dateFmt.format(new Date(unix * 1000)) : "—";
}

export function size(bytes: number): string {
  if (bytes <= 0) return "—";
  const mb = bytes / 1_048_576;
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Steam page inside the Steam client (where Subscribe works). */
export function steamClientUrl(id: WorkshopId): string {
  return `steam://url/CommunityFilePage/${id}`;
}

export function workshopUrl(id: WorkshopId): string {
  return `https://steamcommunity.com/sharedfiles/filedetails/?id=${id}`;
}

/**
 * Colour for a tier band: bright warp-green at the top of the load order,
 * fading to bronze at the foundation.
 */
export function tierColor(priority: number, maxPriority: number): string {
  const t = maxPriority > 0 ? priority / maxPriority : 0;
  const hue = 28 + t * 80;
  const sat = 45 + t * 25;
  const light = 48 + t * 8;
  return `hsl(${hue.toFixed(0)} ${sat.toFixed(0)}% ${light.toFixed(0)}%)`;
}
