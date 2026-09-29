// Mirrors of the warp-core types, as serde serializes them.

export type WorkshopId = string;

export interface ModInfo {
  id: WorkshopId;
  title: string;
  description: string;
  steam_tags: string[];
  file_size: number;
  time_created: number;
  time_updated: number;
  preview_url: string;
  subscriptions: number;
  favorited: number;
  views: number;
  available: boolean;
}

export type Source = "user" | "community" | "heuristic" | "default";

export interface ModKnowledge {
  title?: string;
  tier?: string | null;
  role?: string | null;
  tags?: string[];
  factions?: string[];
  units?: string[];
  requires?: WorkshopId[];
  patches?: WorkshopId[];
  incompatible_with?: WorkshopId[];
}

export interface Resolved {
  tier: string;
  tier_source: Source;
  role: string;
  role_source: Source;
  tags: string[];
  factions: string[];
  units: string[];
  requires: WorkshopId[];
  patches: WorkshopId[];
  incompatible_with: WorkshopId[];
}

export interface LibraryEntry {
  info: ModInfo;
  packs: string[];
  subscribed: boolean;
  knowledge: Resolved;
  user: ModKnowledge | null;
  community: ModKnowledge | null;
  /** What WARP guessed from the packs' contents, headers and Steam tags. */
  guessed: ModKnowledge;
  /** Why WARP guessed that tier ("82% of its files are animations"), when it guessed one. */
  guess_why: string | null;
  sets: string[];
  /** Version on disk (Steam's time_updated when downloaded); null if not installed. */
  installed_version: number | null;
  /** Files across the mod's packs, once indexed. */
  files: number;
  /** Kept up to date for the previous game update, but not updated since the latest. */
  not_updated: NotUpdated | null;
}

/** A mod kept up to date for `previous` that hasn't been updated since `patch` came out. */
export interface NotUpdated {
  /** The latest game update, "9.0". */
  patch: string;
  /** Unix seconds: the day it went live. */
  released: number;
  /** The update before it, "8.0". */
  previous: string;
}

export interface Tier {
  key: string;
  name: string;
  priority: number;
  description: string;
  /** A few words, for tooltips. */
  hint: string;
}
export type Role = Tier;

export interface Taxonomy {
  tier: Tier[];
  role: Role[];
}

export interface ModSet {
  name: string;
  members: WorkshopId[];
}

export interface Pin {
  above: string;
  below: string;
}

export interface ProfileDef {
  name: string;
  sets: string[];
  include: WorkshopId[];
  exclude: WorkshopId[];
  pins: Pin[];
  /** The profile's own copy of each of its sets; filled in by the backend on save. */
  set_members?: Record<string, WorkshopId[]>;
  set_changes?: SetChanges;
  /** Dismissed set changes: set name to the set's fingerprint at the time. */
  dismissed?: Record<string, string>;
}

/** What a profile does when one of its sets changes. */
export type SetChanges = "ask" | "follow" | "ignore";

/** A set that changed since a profile took it. */
export interface SetUpdate {
  set: string;
  added: WorkshopId[];
  removed: WorkshopId[];
  /** The set no longer exists; taking the update drops it from the profile. */
  deleted: boolean;
}

export type RuleKind = "requires" | "patches" | "pin";

export type Reason =
  | { kind: "default"; tier: string; role: string }
  | { kind: "above"; other: string; rule: RuleKind }
  | { kind: "below"; other: string; rule: RuleKind }
  | { kind: "raised"; other: string; rule: RuleKind }
  | { kind: "in_cycle" };

export interface Placement {
  pack: string;
  workshop_id: WorkshopId | null;
  tier: string;
  role: string;
  reasons: Reason[];
}

export interface Rule {
  above: string;
  below: string;
  kind: RuleKind;
}

export interface OrderResult {
  placements: Placement[];
  cycles: Rule[][];
  unused_pins: Pin[];
}

export interface ResolvedProfile {
  profile: ProfileDef;
  order: OrderResult;
  unknown_mods: WorkshopId[];
  mods_without_packs: WorkshopId[];
  unsubscribed: WorkshopId[];
  missing_requirements: [WorkshopId, WorkshopId][];
  incompatibilities: [WorkshopId, WorkshopId][];
  /** Pairs in the profile that look like two versions of the same mod. */
  either_or: DbOverlap[];
  /** Mods kept up to date for the previous game update but not since the latest. */
  not_updated: WorkshopId[];
}

/** Two mods shipping DB files under identical paths. */
export interface DbOverlap {
  a: WorkshopId;
  b: WorkshopId;
  shared: number;
  a_files: number;
  b_files: number;
  /** One shared file, e.g. db/land_units_tables/my_mod. */
  example: string;
}

export interface ShareEntry {
  pack: string;
  workshop_id: WorkshopId | null;
  time_updated: number;
  version?: number;
}

export interface ShareList {
  name: string;
  entries: ShareEntry[];
}

export interface DiffEntry {
  pack: string;
  workshop_id: WorkshopId | null;
  position: number;
}

export interface Move {
  pack: string;
  workshop_id: WorkshopId | null;
  below: string | null;
  below_workshop_id: WorkshopId | null;
}

export interface VersionMismatch {
  pack: string;
  workshop_id: WorkshopId | null;
  a_time_updated: number;
  b_time_updated: number;
  stale: "a" | "b" | "both" | null;
}

export interface ListDiff {
  identical: boolean;
  common: number;
  only_in_a: DiffEntry[];
  only_in_b: DiffEntry[];
  moves: Move[];
  version_mismatches: VersionMismatch[];
}

export interface CompareResult {
  diff: ListDiff;
  a: ShareList;
  b: ShareList;
  /** Steam titles for mods that aren't in the library. */
  titles: Record<WorkshopId, string>;
}

export interface ImportSummary {
  mods: number;
  sets: string[];
  profile: string | null;
  overrides: number;
  unresolved_dependencies: [string, string][];
}

export interface Install {
  game_dir: string;
  workshop_dir: string;
  manifest: string;
}

/** One of W.A.R.P.'s automatic database backups. */
export interface BackupInfo {
  path: string;
  /** Unix seconds. */
  at: number;
  mods: number;
  sets: number;
  profiles: number;
}

export interface Bootstrap {
  version: string;
  /** A backup worth offering because the library is empty and it isn't. */
  restorable: BackupInfo | null;
  taxonomy: Taxonomy;
  mod_count: number;
  data_dir: string;
  kaedrin_dir: string | null;
  /** Where the game is, or why it wasn't found. */
  install: { Ok: Install } | { Err: string };
}

export interface SyncSummary {
  installed: number;
  new_mods: WorkshopId[];
  unsubscribed: WorkshopId[];
  resubscribed: WorkshopId[];
  packs_indexed: number;
  packs_cached: number;
  pack_errors: [string, string][];
  steam_refreshed: number;
}

export type Severity = "low" | "medium" | "high";
export type ContentKind = "db_table" | "startpos" | "script" | "text" | "map" | "ui" | "art" | "other";

export interface PairConflict {
  winner: string;
  loser: string;
  severity: Severity;
  intended: boolean;
  total: number;
  by_kind: Partial<Record<ContentKind, number>>;
  files: string[];
}

export interface Shadowed {
  pack: string;
  files: number;
  overridden: number;
  by: string[];
}

export interface ConflictReport {
  pairs: PairConflict[];
  shadowed: Shadowed[];
  not_indexed: string[];
}

export type ListSource =
  | { kind: "code"; text: string }
  | { kind: "file"; path: string }
  | { kind: "profile"; name: string };

export interface FileFilter {
  name: string;
  extensions: string[];
}
