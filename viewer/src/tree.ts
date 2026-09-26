// The clade tree (spec §7): a phylogeny of the named clades on a time axis. Each clade is a line
// from its founding to its extinction (or to now), as thick as its peak population; a child's line
// starts from its parent's at the moment it split off. Founder lineages are the top level.

import { hueColor } from './glyph';
import type { TreeClade } from './muller';

export function renderTree(
  host: HTMLElement,
  clades: Map<number, TreeClade>,
  opts: {
    now: number;
    epochsPerDay: number;
    seasonDays: number;
    selected?: number;
    onPick: (clade: number) => void;
    label: (c: TreeClade) => string;
    dayLabel: (day: number) => string;
  },
) {
  // Only named clades; an unnamed clade's named descendants hang from its nearest named ancestor.
  const named = [...clades.values()].filter((c) => c.name !== null || c.parent === 0);
  const namedIds = new Set(named.map((c) => c.id));
  const parentOf = (c: TreeClade): number => {
    let p = c.parent;
    while (p !== 0 && !namedIds.has(p)) p = clades.get(p)?.parent ?? 0;
    return p;
  };
  const children = new Map<number, TreeClade[]>();
  const roots: TreeClade[] = [];
  for (const c of named) {
    const p = parentOf(c);
    if (p === 0) roots.push(c);
    else (children.get(p) ?? children.set(p, []).get(p)!).push(c);
  }
  const byFounding = (a: TreeClade, b: TreeClade) => a.founded - b.founded || a.id - b.id;
  roots.sort(byFounding);
  for (const list of children.values()) list.sort(byFounding);

  // Rows in depth-first order: a clade, then its descendants.
  const rows: { c: TreeClade; depth: number; parent?: number }[] = [];
  const walk = (c: TreeClade, depth: number, parent?: number) => {
    rows.push({ c, depth, parent });
    for (const k of children.get(c.id) ?? []) walk(k, depth + 1, c.id);
  };
  roots.forEach((r) => walk(r, 0));

  const rowH = 20;
  const W = Math.max(480, host.clientWidth - 8);
  const pad = { l: 16, r: 200, t: 28, b: 16 };
  const H = pad.t + rows.length * rowH + pad.b;
  const span = opts.seasonDays * opts.epochsPerDay;
  const x = (e: number) => pad.l + (Math.min(e, span) / span) * (W - pad.l - pad.r);
  const rowY = new Map<number, number>();
  rows.forEach((r, i) => rowY.set(r.c.id, pad.t + i * rowH + rowH / 2));
  const maxPeak = Math.max(1, ...rows.map((r) => r.c.peak));

  const out: string[] = [];
  for (let day = 0; day <= opts.seasonDays; day += 7) {
    const xx = x(day * opts.epochsPerDay);
    out.push(`<line class="grid" x1="${xx}" x2="${xx}" y1="${pad.t - 8}" y2="${H - pad.b}"/><text class="axis" x="${xx}" y="${pad.t - 12}" text-anchor="middle">${opts.dayLabel(day)}</text>`);
  }
  out.push(`<line class="now" x1="${x(opts.now)}" x2="${x(opts.now)}" y1="${pad.t - 8}" y2="${H - pad.b}"/>`);
  for (const r of rows) {
    const c = r.c;
    const y = rowY.get(c.id)!;
    const x0 = x(c.founded);
    const x1 = x(c.extinct ?? opts.now);
    const color = `#${hueColor(c.hue, 0.55, 0.6).toString(16).padStart(6, '0')}`;
    const width = 2 + 7 * Math.sqrt(c.peak / maxPeak);
    if (r.parent !== undefined) {
      const py = rowY.get(r.parent)!;
      out.push(`<path class="link" d="M${x0},${py} L${x0},${y}"/>`);
    }
    const dim = opts.selected !== undefined && opts.selected !== c.id ? ' dim' : '';
    const sel = opts.selected === c.id ? ' selected' : '';
    out.push(
      `<g class="clade${dim}${sel}" data-clade="${c.id}"><rect class="hit" x="${x0}" y="${y - rowH / 2}" width="${Math.max(6, x1 - x0) + pad.r}" height="${rowH}"/>` +
        `<line x1="${x0}" x2="${Math.max(x0 + 2, x1)}" y1="${y}" y2="${y}" stroke="${color}" stroke-width="${width.toFixed(1)}" stroke-linecap="round"/>` +
        (c.extinct !== null ? `<circle class="end" cx="${x1}" cy="${y}" r="2.5"/>` : '') +
        `<text class="name" x="${Math.max(x0 + 2, x1) + 8}" y="${y + 4}">${opts.label(c)}</text></g>`,
    );
  }
  host.innerHTML = `<svg class="tree-svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img">${out.join('')}</svg>`;
  host.querySelector('svg')!.addEventListener('click', (ev) => {
    const el = (ev.target as Element).closest('[data-clade]');
    if (el) opts.onPick(Number(el.getAttribute('data-clade')));
  });
  if (opts.selected !== undefined) {
    const y = rowY.get(opts.selected);
    if (y !== undefined) host.scrollTo({ top: Math.max(0, y - host.clientHeight / 2), behavior: 'smooth' });
  }
}
