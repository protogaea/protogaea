// The Muller plot (spec §7): clade shares over the whole season, each child clade drawn inside the
// band of its parent from the moment it split off. Clades that never reached the naming threshold
// are folded into their nearest named ancestor, so the plot shows lineages rather than noise.

import { hueColor } from './glyph';

export interface TreeClade {
  id: number;
  parent: number;
  founded: number;
  extinct: number | null;
  peak: number;
  hue: number;
  name: string | null;
}

export interface Marker {
  epoch: number;
  kind: 'phase' | 'bridge' | 'revival';
  label: string;
}

export interface MullerData {
  clades: Map<number, TreeClade>;
  /** `[epoch, clade, living]`, sorted by epoch. */
  rows: [number, number, number][];
  markers: Marker[];
  epochsPerDay: number;
  seasonDays: number;
}

export interface MullerLayout {
  epochs: number[];
  /** Per shown clade: top and bottom edges (0..1) at each sample. */
  bands: Map<number, { top: Float32Array; bottom: Float32Array }>;
  order: number[];
  /** The shown clade each clade folds into. */
  shownOf: Map<number, number>;
}

const MAX_COLUMNS = 480;

/** The nearest ancestor (itself included) that is named; founders always count as named. */
function shownClade(id: number, clades: Map<number, TreeClade>, memo: Map<number, number>): number {
  const known = memo.get(id);
  if (known !== undefined) return known;
  let c = clades.get(id);
  let cur = id;
  const path: number[] = [];
  while (c && c.name === null && c.parent !== 0 && clades.has(c.parent)) {
    path.push(cur);
    cur = c.parent;
    c = clades.get(cur);
    const m = memo.get(cur);
    if (m !== undefined) {
      cur = m;
      break;
    }
  }
  for (const p of path) memo.set(p, cur);
  memo.set(id, cur);
  return cur;
}

export function layoutMuller(d: MullerData): MullerLayout {
  const memo = new Map<number, number>();
  const shownOf = new Map<number, number>();
  for (const id of d.clades.keys()) shownOf.set(id, shownClade(id, d.clades, memo));

  // Samples, thinned to at most MAX_COLUMNS columns.
  const allEpochs = [...new Set(d.rows.map((r) => r[0]))].sort((a, b) => a - b);
  const stride = Math.max(1, Math.ceil(allEpochs.length / MAX_COLUMNS));
  const epochs = allEpochs.filter((_, i) => i % stride === 0 || i === allEpochs.length - 1);
  const col = new Map(epochs.map((e, i) => [e, i]));
  const T = epochs.length;

  const own = new Map<number, Float32Array>();
  for (const [e, clade, living] of d.rows) {
    const t = col.get(e);
    if (t === undefined) continue;
    const s = shownOf.get(clade) ?? clade;
    let a = own.get(s);
    if (!a) own.set(s, (a = new Float32Array(T)));
    a[t] += living;
  }

  // Hourly counts are jagged; a centered moving average over five samples keeps the trend.
  for (const [id, a] of own) {
    const s = new Float32Array(T);
    for (let i = 0; i < T; i++) {
      let sum = 0;
      let n = 0;
      for (let j = Math.max(0, i - 2); j <= Math.min(T - 1, i + 2); j++) {
        sum += a[j];
        n++;
      }
      // A clade that is absent stays absent: no smearing before its founding or after extinction.
      s[i] = a[i] > 0 ? sum / n : 0;
    }
    own.set(id, s);
  }

  // The tree of shown clades.
  const children = new Map<number, number[]>();
  const roots: number[] = [];
  for (const id of own.keys()) {
    const c = d.clades.get(id);
    const parent = c && c.parent !== 0 ? shownOf.get(c.parent) : undefined;
    if (parent !== undefined && parent !== id && own.has(parent)) {
      (children.get(parent) ?? children.set(parent, []).get(parent)!).push(id);
    } else {
      roots.push(id);
    }
  }
  const byFounding = (a: number, b: number) => (d.clades.get(a)?.founded ?? 0) - (d.clades.get(b)?.founded ?? 0) || a - b;
  roots.sort(byFounding);
  for (const list of children.values()) list.sort(byFounding);

  const total = new Map<number, Float32Array>();
  const order: number[] = [];
  const sum = (id: number): Float32Array => {
    order.push(id);
    const t = Float32Array.from(own.get(id) ?? new Float32Array(T));
    for (const k of children.get(id) ?? []) {
      const kt = sum(k);
      for (let i = 0; i < T; i++) t[i] += kt[i];
    }
    total.set(id, t);
    return t;
  };
  const population = new Float32Array(T);
  for (const r of roots) {
    const t = sum(r);
    for (let i = 0; i < T; i++) population[i] += t[i];
  }

  // Nested bands: a parent's own members split around its children, which sit in its middle.
  const bands = new Map<number, { top: Float32Array; bottom: Float32Array }>();
  const place = (id: number, y0: Float32Array) => {
    const t = total.get(id)!;
    const o = own.get(id) ?? new Float32Array(T);
    const top = new Float32Array(T);
    const bottom = new Float32Array(T);
    const inner = new Float32Array(T);
    for (let i = 0; i < T; i++) {
      const p = population[i] || 1;
      top[i] = y0[i];
      bottom[i] = y0[i] + t[i] / p;
      inner[i] = y0[i] + o[i] / 2 / p;
    }
    bands.set(id, { top, bottom });
    const cursor = Float32Array.from(inner);
    for (const k of children.get(id) ?? []) {
      place(k, cursor);
      const kt = total.get(k)!;
      for (let i = 0; i < T; i++) cursor[i] += kt[i] / (population[i] || 1);
    }
  };
  const y = new Float32Array(T);
  for (const r of roots) {
    place(r, y);
    const t = total.get(r)!;
    for (let i = 0; i < T; i++) y[i] += t[i] / (population[i] || 1);
  }
  return { epochs, bands, order, shownOf };
}

/** Draws the plot as SVG; returns the layout for hit testing. */
export function renderMuller(
  host: HTMLElement,
  d: MullerData,
  opts: { selected?: number; now: number; onPick: (clade: number) => void; onHover: (clade: number | null, share: number, x: number, y: number) => void; dayLabel: (day: number) => string },
): MullerLayout {
  const L = layoutMuller(d);
  const W = Math.max(320, host.clientWidth);
  const H = Math.max(240, host.clientHeight);
  const pad = { l: 16, r: 16, t: 30, b: 34 };
  const span = d.seasonDays * d.epochsPerDay;
  const x = (e: number) => pad.l + (e / span) * (W - pad.l - pad.r);
  const y = (v: number) => pad.t + v * (H - pad.t - pad.b);
  const selectedShown = opts.selected !== undefined ? L.shownOf.get(opts.selected) ?? opts.selected : undefined;

  const paths: string[] = [];
  for (const id of L.order) {
    const b = L.bands.get(id)!;
    const up: string[] = [];
    const down: string[] = [];
    for (let i = 0; i < L.epochs.length; i++) {
      if (b.bottom[i] - b.top[i] <= 0 && (i === 0 || b.bottom[i - 1] - b.top[i - 1] <= 0)) continue;
      up.push(`${x(L.epochs[i]).toFixed(1)},${y(b.top[i]).toFixed(1)}`);
      down.push(`${x(L.epochs[i]).toFixed(1)},${y(b.bottom[i]).toFixed(1)}`);
    }
    if (up.length < 2) continue;
    const c = d.clades.get(id);
    const fill = hueColor(c?.hue ?? 0, 0.55, 0.55);
    const dim = selectedShown !== undefined && selectedShown !== id ? ' dim' : '';
    paths.push(
      `<polygon class="band${dim}" data-clade="${id}" fill="#${fill.toString(16).padStart(6, '0')}" points="${up.join(' ')} ${down.reverse().join(' ')}"/>`,
    );
  }

  const ticks: string[] = [];
  const step = d.seasonDays > 21 ? 7 : 3;
  for (let day = 0; day <= d.seasonDays; day += step) {
    const xx = x(day * d.epochsPerDay);
    ticks.push(`<line class="tick" x1="${xx}" x2="${xx}" y1="${H - pad.b}" y2="${H - pad.b + 5}"/><text class="axis" x="${xx}" y="${H - pad.b + 18}" text-anchor="middle">${opts.dayLabel(day)}</text>`);
  }
  const marks = d.markers
    .map((m) => {
      const xx = x(m.epoch);
      if (m.kind === 'phase') {
        return `<line class="phase" x1="${xx}" x2="${xx}" y1="${pad.t}" y2="${H - pad.b}"/><text class="phase-label" x="${xx + 4}" y="${pad.t - 10}">${m.label}</text>`;
      }
      const icon = m.kind === 'bridge' ? 'bridge' : 'revival';
      return `<g class="${icon}" transform="translate(${xx},${H - pad.b})"><title>${m.label}</title><rect x="-4" y="-4" width="8" height="8" transform="rotate(45)"/></g>`;
    })
    .join('');
  const nowX = x(opts.now);
  host.innerHTML = `<svg class="muller-svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img">
    <rect class="muller-bg" x="${pad.l}" y="${pad.t}" width="${nowX - pad.l}" height="${H - pad.t - pad.b}"/>
    ${paths.join('')}
    <rect class="future" x="${nowX}" y="${pad.t}" width="${W - pad.r - nowX}" height="${H - pad.t - pad.b}"/>
    <line class="now" x1="${nowX}" x2="${nowX}" y1="${pad.t - 4}" y2="${H - pad.b}"/>
    ${marks}${ticks.join('')}
  </svg>`;

  const svg = host.querySelector('svg')!;
  svg.addEventListener('click', (ev) => {
    const el = (ev.target as Element).closest('[data-clade]');
    if (el) opts.onPick(Number(el.getAttribute('data-clade')));
  });
  svg.addEventListener('mousemove', (ev) => {
    const el = (ev.target as Element).closest('[data-clade]');
    if (!el) return opts.onHover(null, 0, 0, 0);
    const id = Number(el.getAttribute('data-clade'));
    const r = svg.getBoundingClientRect();
    const e = ((ev.clientX - r.left - pad.l) / (W - pad.l - pad.r)) * span;
    let i = 0;
    while (i < L.epochs.length - 1 && L.epochs[i + 1] <= e) i++;
    const b = L.bands.get(id)!;
    opts.onHover(id, b.bottom[i] - b.top[i], ev.clientX, ev.clientY);
  });
  svg.addEventListener('mouseleave', () => opts.onHover(null, 0, 0, 0));
  return L;
}
