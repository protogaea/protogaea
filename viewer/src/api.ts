// The read API v0 of the world server (spec §23), as the viewer uses it.

export interface Deaths {
  starvation: number;
  old_age: number;
  predation: number;
  plague: number;
  drowned: number;
}

export interface Header {
  epoch: number;
  day: number;
  phase: number;
  phase_name: string;
  state_root: string;
  population: number;
  grazers: number;
  armored: number;
  hunters: number;
  clades: number;
  clades_20: number;
  dominant_clade: number;
  dominant_permille: number;
  births: number;
  deaths: Deaths;
  wildfires: number;
  floods: number;
  droughts: number;
  plagues: number;
}

export interface WorldInfo {
  world_id: string;
  seed: number;
  width: number;
  height: number;
  epochs_per_day: number;
  season_days: number;
  epoch_seconds: number;
  next_epoch_ms: number;
  archive_every: number;
  finished: boolean;
  header: Header;
}

export interface Effect {
  kind: 'Ash' | 'Drought' | 'Flood';
  center: number;
  radius: number;
  remaining_ticks: number;
}

export interface MapState {
  epoch: number;
  width: number;
  height: number;
  biome: number[];
  food: number[];
  moisture: number[];
  rift: number[];
  effects: Effect[];
  /** Names of the living named clades, by id. */
  names: Record<string, string>;
  organisms: {
    id: number[];
    cell: number[];
    clade: number[];
    hue: number[];
    kind: number[];
    energy: number[];
    age: number[];
    traits: number[];
  };
}

export interface Rift {
  cell: number;
  bridge: number;
  fault_epoch: number;
  shallows_epoch: number;
  deep_epoch: number;
}

export interface Genome {
  traits: number[];
  habitat: number;
  dispersal: number;
  boldness: number;
  hue: number;
}

export interface WorldEvent {
  id: number;
  epoch: number;
  kind: string;
  clade_id: number | null;
  organism_id: number | null;
  data: Record<string, unknown>;
}

export interface CladeInfo {
  id: number;
  parent_id: number;
  founded_epoch: number;
  extinct_epoch: number | null;
  living: number;
  peak_living: number;
  reference: Genome;
  name: string | null;
  parent_name?: string | null;
  children?: number[];
  history?: [number, number][];
}

export interface OrganismInfo {
  id: number;
  parent_id: number;
  clade_id: number;
  lineage_id: number;
  born_epoch: number;
  died_epoch: number | null;
  cause: string | null;
  genome: Genome;
  at_death: { age: number; energy: number; cell: number } | null;
  living?: { epoch: number; cell: number; age: number; energy: number };
  offspring: number[];
}

export interface StoryRow {
  id: number;
  epoch: number;
  kind: string;
  clade_id: number | null;
  other_id: number | null;
  plate: number | null;
  score: number;
  data: Record<string, unknown>;
}

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

async function get<T>(path: string): Promise<T> {
  const res = await fetch(path);
  if (!res.ok) {
    let code = 'E_HTTP';
    let message = `${res.status}`;
    try {
      const body = await res.json();
      code = body.error ?? code;
      message = body.message ?? message;
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, code, message);
  }
  return res.json() as Promise<T>;
}

export const api = {
  world: () => get<WorldInfo>('/v0/world'),
  map: (epoch?: number) => get<MapState>(epoch === undefined ? '/v0/map' : `/v0/map?epoch=${epoch}`),
  rifts: () => get<{ rifts: Rift[] }>('/v0/rifts'),
  snapshots: () => get<{ every: number; epochs: number[] }>('/v0/snapshots'),
  latestEvents: (limit = 60, until?: number) =>
    get<{ events: WorldEvent[]; names: Record<string, string> }>(`/v0/events?before=0&limit=${limit}${until === undefined ? '' : `&until=${until}`}`),
  eventsOfKind: (kind: string) => get<{ events: WorldEvent[]; names: Record<string, string> }>(`/v0/events?kind=${kind}&limit=500`),
  tree: () =>
    get<{ clades: [number, number, number, number | null, number, number, string | null][]; name_threshold: number }>('/v0/tree'),
  stories: (limit = 6, until?: number) =>
    get<{ since: number; stories: StoryRow[]; names: Record<string, string> }>(`/v0/stories?limit=${limit}${until === undefined ? '' : `&until=${until}`}`),
  storiesBetween: (since: number, until: number, limit = 3) =>
    get<{ since: number; stories: StoryRow[]; names: Record<string, string> }>(`/v0/stories?since=${since}&until=${until}&limit=${limit}`),
  proof: (epoch: number, id: number) => get<{ epoch: number; state_root: string; path: string[]; size: number }>(`/v0/proofs/${epoch}/organism/${id}`),
  digest: (since: number) =>
    get<{
      since: number;
      now: number;
      then: Header | null;
      header: Header;
      counts: Record<string, number>;
      bridges_closed: WorldEvent[];
      stories: StoryRow[];
      names: Record<string, string>;
    }>(`/v0/digest?since=${since}`),
  replay: async (step = 2): Promise<ArrayBuffer> => {
    const res = await fetch(`/v0/replay?step=${step}`);
    if (!res.ok) throw new ApiError(res.status, 'E_HTTP', `${res.status}`);
    return res.arrayBuffer();
  },
  storiesSince: (since: number) => get<{ stories: StoryRow[]; names: Record<string, string> }>(`/v0/stories?since=${since}&limit=60`),
  epochHeader: (n: number) => get<Header>(`/v0/epochs/${n}`),
  snapshotBytes: async (n: number): Promise<ArrayBuffer> => {
    const res = await fetch(`/v0/snapshots/${n}`);
    if (!res.ok) throw new ApiError(res.status, 'E_NO_SNAPSHOT', `no snapshot at epoch ${n}`);
    return res.arrayBuffer();
  },
  muller: () => get<{ every: number; rows: [number, number, number][] }>('/v0/muller'),
  clade: (id: number) => get<CladeInfo>(`/v0/clades/${id}`),
  organism: (id: number) => get<OrganismInfo>(`/v0/organisms/${id}`),
};
