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
  sets: string[];
}

export interface Tier {
  key: string;
  name: string;
  priority: number;
  description: string;
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

export interface Bootstrap {
  taxonomy: Taxonomy;
  mod_count: number;
  data_dir: string;
  kaedrin_dir: string | null;
}

export type ListSource =
  | { kind: "code"; text: string }
  | { kind: "file"; path: string }
  | { kind: "profile"; name: string };

export interface FileFilter {
  name: string;
  extensions: string[];
}
