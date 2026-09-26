import '@fontsource-variable/geist';
import '@fontsource-variable/geist-mono';
import '@phosphor-icons/web/regular';
import './style.css';

import { api, ApiError, type StoryRow, type CladeInfo, type Header, type MapState, type OrganismInfo, type Rift, type WorldEvent, type WorldInfo } from './api';
import { hueColor } from './glyph';
import { fmt, lang, t } from './i18n';
import { WorldMap, type Layers } from './map';
import { renderMuller, type Marker, type MullerData, type TreeClade } from './muller';
import { renderTree } from './tree';
import * as predictions from './predictions';
import { track } from './visits';
import * as timeMachine from './timemachine';
import * as naturalist from './naturalist';
import { parseFrames, stateOf } from './replay';

// Permanent links live in the hash: #epoch=N&clade=ID&organism=ID. Without `epoch` the viewer is
// live and follows the world epoch by epoch.

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
const css = (color: number) => `#${color.toString(16).padStart(6, '0')}`;
const nf = new Intl.NumberFormat(lang);

/** The archetype portraits, generated for the viewer; counted as the harness counts them. */
type Kind = 'grazer' | 'armored' | 'hunter';
const kindOf = (traits: number[]): Kind => (traits[3] >= 4 ? 'hunter' : traits[4] >= 6 ? 'armored' : 'grazer');
const portrait = (kind: Kind, cls = 'portrait') => `<img class="${cls}" src="${import.meta.env.BASE_URL}archetypes/${kind}.webp" alt="" width="36" height="36">`;

type View = 'map' | 'muller' | 'tree' | 'museum' | 'chronicle';

interface Route {
  epoch?: number;
  clade?: number;
  organism?: number;
  view?: View;
  /** A moment remembered to compare the one on the map with. */
  cmp?: number;
}

function readRoute(): Route {
  const p = new URLSearchParams(location.hash.slice(1));
  const num = (k: string) => (p.get(k) ? Number(p.get(k)) : undefined);
  const view = p.get('view');
  return {
    epoch: num('epoch'),
    clade: num('clade'),
    organism: num('organism'),
    view: view === 'muller' || view === 'tree' || view === 'museum' || view === 'chronicle' ? view : undefined,
    cmp: num('cmp'),
  };
}

function link(r: Route): string {
  const p = new URLSearchParams();
  if (r.epoch !== undefined) p.set('epoch', String(r.epoch));
  if (r.clade !== undefined) p.set('clade', String(r.clade));
  if (r.organism !== undefined) p.set('organism', String(r.organism));
  const view = 'view' in r ? r.view : route.view;
  if (view && view !== 'map') p.set('view', view);
  const cmp = 'cmp' in r ? r.cmp : route.cmp;
  if (cmp !== undefined) p.set('cmp', String(cmp));
  return `#${p.toString()}`;
}

// ---------------------------------------------------------------- clade names

/** Names of named clades seen so far, from the map, the feed and cards. */
const names = new Map<number, string>();
function learnNames(record: Record<string, string> | undefined) {
  for (const [id, name] of Object.entries(record ?? {})) names.set(Number(id), name);
}
/** A clade as the interface shows it: its Latin name in italics, or "clade N" before it is named. */
function cladeLabel(id: number): string {
  const name = names.get(id);
  return name ? `<em class="latin">${esc(name)}</em>` : fmt(t.clade, { id });
}

const go = (r: Route) => (location.hash = link(r));

// ---------------------------------------------------------------- layer preferences

const LAYERS_KEY = 'protogaea.layers';
function loadLayers(): Layers {
  const fallback: Layers = { biomes: true, territories: false, food: false, organisms: true, rifts: true, events: true };
  try {
    return { ...fallback, ...JSON.parse(localStorage.getItem(LAYERS_KEY) ?? '{}') };
  } catch {
    return fallback;
  }
}
function saveLayers(l: Layers) {
  try {
    localStorage.setItem(LAYERS_KEY, JSON.stringify(l));
  } catch {
    /* storage can be blocked; the layers still work for this visit */
  }
}

// ---------------------------------------------------------------- state

const map = new WorldMap($('map'));
let world: WorldInfo;
let rules: { rifts: Record<string, number>; season_days: number; biomes: { food_max: number }[] };
let state: MapState | undefined;
/** The header of the past epoch on the map, in past mode. */
let pastHeader: Header | undefined;
let rifts: Rift[] = [];
let bridgeCloses: [number, number][] = [];
let route: Route = readRoute();
let layers = loadLayers();

function banner(html: string | null, error = false) {
  const el = $('banner');
  el.hidden = html === null;
  el.className = `banner${error ? ' error' : ''}`;
  if (html) el.innerHTML = html;
}

function duration(epochs: number): string {
  const minutes = (epochs * 24 * 60) / world.epochs_per_day;
  if (minutes >= 48 * 60) return fmt(t.daysShort, { d: (minutes / 1440).toFixed(1) });
  return minutes >= 90 ? fmt(t.hours, { h: Math.round(minutes / 60) }) : fmt(t.minutes, { m: Math.round(minutes) });
}

const dayOf = (epoch: number) => epoch / world.epochs_per_day;

// ---------------------------------------------------------------- the season timeline

function renderSeason() {
  const r = rules.rifts;
  const days = rules.season_days;
  const bounds = [0, r.fault_day, r.shallows_from_day, r.deep_from_day, r.bridges_from_day, r.bridges_to_day, days];
  const now = dayOf(state?.epoch ?? world.header.epoch);
  const phases = bounds
    .slice(0, -1)
    .map((from, i) => {
      const width = ((bounds[i + 1] - from) / days) * 100;
      const current = now >= from && now < bounds[i + 1];
      return `<div class="season-phase${current ? ' now' : ''}" style="width:${width}%" title="${esc(t.phases[i])}">${esc(t.phases[i])}</div>`;
    })
    .join('');
  const ticks = bridgeCloses.map(([, e]) => `<i class="season-bridge" style="left:${(dayOf(e) / days) * 100}%"></i>`).join('');
  const pct = Math.min(100, (now / days) * 100);
  $('season').innerHTML = `
    <div class="season-track">${phases}<div class="season-elapsed" style="width:${pct}%"></div>${ticks}<i class="season-marker" style="left:calc(${pct}% - 1px)"></i></div>
    <div class="season-meta"><span>${fmt(t.ofSeason, { day: `<span class="num">${now.toFixed(2)}</span>`, days })}</span><span>${nextBridge()}</span></div>`;
}

/** The next land bridge to close; subscribers must know a day ahead (spec §27.8). */
function nextBridge(): string {
  const now = state?.epoch ?? world.header.epoch;
  const upcoming = bridgeCloses.filter(([, e]) => e > now);
  if (upcoming.length === 0) return bridgeCloses.length ? t.noBridges : '';
  const [bridge, epoch] = upcoming[0];
  return fmt(t.nextBridge, { bridge, when: duration(epoch - now), day: Math.floor(dayOf(epoch)) });
}

// ---------------------------------------------------------------- the clock

function renderClock() {
  const live = route.epoch === undefined;
  if (!live) {
    const steps: [string, number, string][] = [
      ['ph-caret-double-left', -12, t.hourBack],
      ['ph-caret-left', -1, t.stepBack],
      ['ph-caret-right', 1, t.stepFwd],
      ['ph-caret-double-right', 12, t.hourFwd],
    ];
    $('clock').innerHTML = `<span class="countdown">${fmt(t.past, { epoch: state?.epoch ?? route.epoch ?? '' })}</span><span class="tm-steps">${steps
      .map(([icon, by, label]) => `<button class="icon-btn" data-by="${by}" title="${esc(label)}" aria-label="${esc(label)}"><i class="ph ${icon}"></i></button>`)
      .join('')}</span>${pinBtn()}<button class="btn primary" id="to-live"><i class="ph ph-play"></i>${t.jumpToLatest}</button>`;
    $('to-live').addEventListener('click', () => go({ clade: route.clade, organism: route.organism }));
    bindPin();
    $('clock').querySelectorAll<HTMLButtonElement>('.tm-steps .icon-btn').forEach((b) => b.addEventListener('click', () => stepBy(Number(b.dataset.by))));
    return;
  }
  const left = Math.max(0, Math.round((world.next_epoch_ms - Date.now()) / 1000));
  const s = left >= 60 ? `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}` : `0:${String(left).padStart(2, '0')}`;
  $('clock').innerHTML = `<span class="countdown">${left > 0 ? fmt(t.nextEpoch, { s: `<span class="num">${s}</span>` }) : t.epochNow}</span>${pinBtn()}<span class="live-pill">${t.live}</span>`;
  bindPin();
}

// ---------------------------------------------------------------- comparing two moments (B7)

function pinBtn(): string {
  const here = route.epoch ?? world.header.epoch;
  return `<button class="icon-btn pin-btn" id="pin-btn" aria-pressed="${route.cmp === here}" title="${esc(t.pinMoment)}" aria-label="${esc(t.pinMoment)}"><i class="ph ph-push-pin"></i></button>`;
}
function bindPin() {
  $('pin-btn').addEventListener('click', () => {
    const here = route.epoch ?? world.header.epoch;
    go({ ...route, cmp: route.cmp === here ? undefined : here });
  });
}

type TreeRow = [number, number, number, number | null, number, number, string | null];
let treeRows: { at: number; rows: Promise<TreeRow[]> } | undefined;
function namedClades(): Promise<TreeRow[]> {
  if (!treeRows || treeRows.at !== world.header.epoch) treeRows = { at: world.header.epoch, rows: api.tree().then((r) => r.clades) };
  return treeRows.rows;
}

async function renderCompare() {
  const el = $('compare');
  if (route.cmp === undefined) {
    el.hidden = true;
    return;
  }
  const here = route.epoch ?? world.header.epoch;
  const close = `<button class="icon-btn close" aria-label="${t.close}"><i class="ph ph-x"></i></button>`;
  if (route.cmp === here) {
    el.innerHTML = `<div class="card-head"><h2>${fmt(t.compareTitle, { a: nf.format(here), b: '…' })}</h2>${close}</div><p class="empty">${t.compareHint}</p>`;
  } else {
    const [a, b] = route.cmp < here ? [route.cmp, here] : [here, route.cmp];
    const header = (e: number) => (e === world.header.epoch ? Promise.resolve(world.header) : api.epochHeader(e));
    const [ha, hb, rows, told] = await Promise.all([header(a), header(b), namedClades(), api.storiesBetween(a + 1, b, 3)]);
    learnNames(told.names);
    for (const r of rows) if (r[6]) names.set(r[0], r[6]);
    const alive = (r: TreeRow, e: number) => r[2] <= e && (r[3] === null || r[3] > e);
    const appeared = rows.filter((r) => r[6] && r[1] !== 0 && !alive(r, a) && alive(r, b));
    const gone = rows.filter((r) => r[6] && alive(r, a) && !alive(r, b));
    const row = (label: string, x: number, y: number) => {
      const d = y - x;
      const sign = d > 0 ? `<span class="up">+${nf.format(d)}</span>` : d < 0 ? `<span class="down">−${nf.format(-d)}</span>` : '±0';
      return `<tr><td>${label[0].toUpperCase() + label.slice(1)}</td><td class="n">${nf.format(x)} → ${nf.format(y)}</td><td class="n">${sign}</td></tr>`;
    };
    const cladeLinks = (list: TreeRow[]) =>
      list.length === 0
        ? `<span class="empty">${t.compareNone}</span>`
        : list
            .slice(0, 10)
            .map((r) => `<a href="${link({ ...route, clade: r[0], organism: undefined })}">${cladeLabel(r[0])}</a>`)
            .join(', ') + (list.length > 10 ? ` +${list.length - 10}` : '');
    el.innerHTML = `<div class="card-head"><h2>${fmt(t.compareTitle, { a: nf.format(a), b: nf.format(b) })}</h2>${close}</div>
      <div class="subtitle">${fmt(t.compareSpan, { time: duration(b - a), from: dayOf(a).toFixed(2), to: dayOf(b).toFixed(2) })}</div>
      <table class="cmp-table">
        ${row(t.organismsAlive, ha.population, hb.population)}
        ${row(t.grazers, ha.grazers, hb.grazers)}
        ${row(t.armored, ha.armored, hb.armored)}
        ${row(t.hunters, ha.hunters, hb.hunters)}
        ${row(t.livingClades, ha.clades, hb.clades)}
        ${row(`${t.clades} ${t.clades20}`, ha.clades_20, hb.clades_20)}
      </table>
      <div class="fact">${t.compareLeader}<b>${ha.dominant_clade !== hb.dominant_clade ? `${cladeLabel(ha.dominant_clade)} → ` : ''}${cladeLabel(hb.dominant_clade)}</b></div>
      <div class="fact" style="margin-top:10px">${t.compareAppeared} (${appeared.length})</div><div class="cmp-list">${cladeLinks(appeared)}</div>
      <div class="fact">${t.compareGone} (${gone.length})</div><div class="cmp-list">${cladeLinks(gone)}</div>
      ${told.stories.length ? `<div class="fact">${t.compareStories}</div><div class="stories" style="margin-top:8px">${told.stories.map(storyHtml).join('')}</div>` : ''}
      <p style="margin:10px 0 0"><a href="${link({ ...route, epoch: route.cmp === world.header.epoch ? undefined : route.cmp })}">${fmt(t.compareOpen, { epoch: nf.format(route.cmp) })}</a></p>`;
  }
  el.hidden = false;
  el.querySelector('.close')?.addEventListener('click', () => go({ ...route, cmp: undefined }));
}

// ---------------------------------------------------------------- world numbers

function renderStats(h: Header) {
  const deaths = h.deaths.starvation + h.deaths.old_age + h.deaths.predation + h.deaths.plague + h.deaths.drowned;
  $('stats').innerHTML = `
    <div class="big">${nf.format(h.population)}</div>
    <div class="big-label">${t.organismsAlive} · ${t.epoch} <span class="num">${nf.format(h.epoch)}</span></div>
    <div class="split" aria-hidden="true">
      <i style="flex-grow:${h.grazers};background:var(--grazer)"></i>
      <i style="flex-grow:${h.armored};background:var(--armored)"></i>
      <i style="flex-grow:${h.hunters};background:var(--hunter)"></i>
    </div>
    <div class="kinds">
      <div class="kind">${portrait('grazer')}<span>${t.grazers}<b>${nf.format(h.grazers)}</b></span></div>
      <div class="kind">${portrait('armored')}<span>${t.armored}<b>${nf.format(h.armored)}</b></span></div>
      <div class="kind">${portrait('hunter')}<span>${t.hunters}<b>${nf.format(h.hunters)}</b></span></div>
    </div>
    <div class="facts">
      <div class="fact">${t.clades}<b class="num">${h.clades} <span style="color:var(--text-3)">/ ${h.clades_20} ${t.clades20}</span></b></div>
      <div class="fact">${t.dominant}<b><a href="${link({ ...route, clade: h.dominant_clade, organism: undefined })}">${cladeLabel(h.dominant_clade)}</a> <span class="num" style="color:var(--text-3)">${(h.dominant_permille / 10).toFixed(0)}%</span></b></div>
      <div class="fact">${t.bornCount}, ${t.thisEpoch}<b class="num">${nf.format(h.births)}</b></div>
      <div class="fact">${t.diedCount}, ${t.thisEpoch}<b class="num">${nf.format(deaths)}</b></div>
    </div>`;
}

// ---------------------------------------------------------------- layers and legend

function renderLayers() {
  const items: [keyof Layers, string, string][] = [
    ['biomes', t.layerBiomes, 'ph-mountains'],
    ['territories', t.layerTerritories, 'ph-polygon'],
    ['food', t.layerFood, 'ph-plant'],
    ['organisms', t.layerOrganisms, 'ph-bug-beetle'],
    ['rifts', t.layerRifts, 'ph-waves'],
    ['events', t.layerEvents, 'ph-fire'],
  ];
  $('layers').innerHTML = items
    .map(([k, name, icon]) => `<button class="layer" data-layer="${k}" aria-pressed="${layers[k]}" title="${esc(name)}"><i class="ph ${icon}"></i><span>${esc(name)}</span></button>`)
    .join('');
  $('layers').querySelectorAll<HTMLButtonElement>('.layer').forEach((b) =>
    b.addEventListener('click', () => {
      const k = b.dataset.layer as keyof Layers;
      layers = { ...layers, [k]: !layers[k] };
      b.setAttribute('aria-pressed', String(layers[k]));
      saveLayers(layers);
      map.setLayers(layers);
    }),
  );
  $('legend').innerHTML = `
    <span><i class="dot" style="background:#7fb3d9"></i>${t.legendGrazer}</span>
    <span><i class="dot" style="background:#7fb3d9;box-shadow:0 0 0 1.5px rgb(11 16 22 / .6)"></i>${t.legendArmored}</span>
    <span><i class="dot" style="background:#7fb3d9;box-shadow:0 0 0 2px #d9412c"></i>${t.legendHunter}</span>
    <span><i class="seam"></i>${t.legendSeam}</span>
    <span><i class="fault"></i>${t.legendFault}</span>
    <span><i class="dot" style="background:#f2c14e;box-shadow:0 0 0 1.5px #fff1c9"></i>${t.legendBridge}</span>`;
}

// ---------------------------------------------------------------- the feed

const EVENT_ICONS: Record<string, string> = {
  phase: 'ph-flag-banner',
  clade_founded: 'ph-git-branch',
  clade_named: 'ph-tag',
  clade_extinct: 'ph-skull',
  dominant_changed: 'ph-crown-simple',
  bridge_closed: 'ph-waves',
  wildfire: 'ph-fire',
  drought: 'ph-sun',
  flood: 'ph-drop',
  plague: 'ph-virus',
  revival: 'ph-plant',
  miracle: 'ph-sparkle',
};
const MAJOR = new Set(['phase', 'bridge_closed', 'dominant_changed', 'revival', 'clade_extinct', 'miracle']);

function eventText(e: WorldEvent): string {
  const d = e.data as Record<string, number | string | boolean>;
  const cladeLink = (id: unknown) => `<a href="${link({ ...route, clade: Number(id), organism: undefined })}">${cladeLabel(Number(id))}</a>`;
  if (e.kind === 'miracle') {
    const at = (e.data.at as number[] | undefined) ?? [];
    const what = d.action === 'weather' ? (d.kind === 'rain' ? t.miracleRain : t.miracleDrought) : d.action === 'migrate' ? t.miracleMigrate : d.source === 'museum' ? t.miracleReviveMuseum : t.miracleReviveSpores;
    const text = fmt(what, { clade: e.clade_id !== null ? cladeLink(e.clade_id) : '', x: String(at[0] ?? ''), y: String(at[1] ?? '') });
    return d.applied ? fmt(t.miracleApplied, { what: text }) : fmt(t.miracleRefused, { what: text, reason: esc(String(d.reason ?? '')) });
  }
  return fmt(t.events[e.kind] ?? e.kind, {
    clade: e.clade_id !== null ? cladeLink(e.clade_id) : '',
    parent: cladeLink(d.parent_id),
    peak: String(d.peak_living ?? ''),
    share: d.permille !== undefined ? (Number(d.permille) / 10).toFixed(0) : '',
    bridge: String(d.bridge ?? ''),
    name: esc(t.phases[Number(d.phase) - 1] ?? String(d.name ?? '')),
    deaths: String(d.deaths ?? ''),
    n: String(d.organisms ?? ''),
  });
}

// ---------------------------------------------------------------- stories

const STORY_ICONS: Record<string, string> = {
  comeback: 'ph-arrow-counter-clockwise',
  crossing: 'ph-boat',
  invasion: 'ph-flag-pennant',
  arms_race: 'ph-sword',
  last_of_its_kind: 'ph-hourglass-low',
  changing_of_the_guard: 'ph-crown',
  split_by_sea: 'ph-waves',
  fall: 'ph-skull',
};

function storyHtml(s: StoryRow): string {
  const [title, template] = t.story[s.kind] ?? [s.kind, ''];
  const d = s.data as Record<string, unknown>;
  const clade = (id: unknown) => (id === null || id === undefined ? '' : `<a href="${link({ ...route, clade: Number(id), organism: undefined })}">${cladeLabel(Number(id))}</a>`);
  const continent = (plate: unknown) => (plate === null || plate === undefined ? '' : fmt(t.continent, { n: Number(plate) + 1 }));
  const tenth = (v: unknown) => (Number(v) / 10).toFixed(1);
  const hunting = (d.hunting_x10 as number[] | undefined) ?? [];
  const defense = (d.defense_x10 as number[] | undefined) ?? [];
  const text = fmt(template, {
    clade: clade(s.clade_id),
    other: clade(s.other_id),
    low: String(d.low ?? ''),
    living: String(d.living ?? ''),
    peak: String(d.peak ?? ''),
    to: continent(d.to),
    plate: continent(s.plate),
    organism: d.organism ? `<a href="${link({ ...route, organism: Number(d.organism), clade: undefined })}">#${d.organism}</a>` : '',
    h0: tenth(hunting[0]),
    h1: tenth(hunting[1]),
    d0: tenth(defense[0]),
    d1: tenth(defense[1]),
  });
  return `<article class="story"><span class="icon"><i class="ph ${STORY_ICONS[s.kind] ?? 'ph-star'}"></i></span><div><h4><span>${title}</span><span class="when">${t.day} ${dayOf(s.epoch).toFixed(2)}</span></h4><p>${text}</p></div></article>`;
}

async function renderStories() {
  const { stories, names: storyNames } = await api.stories(5, route.epoch);
  learnNames(storyNames);
  $('stories').innerHTML =
    `<h2>${t.storiesTitle}</h2>` +
    (stories.length === 0 ? `<p class="empty">${t.storiesEmpty}</p>` : `<div class="stories">${stories.map(storyHtml).join('')}</div>`);
}

async function renderFeed() {
  const { events, names: eventNames } = await api.latestEvents(120, route.epoch);
  learnNames(eventNames);
  // New clades split off all the time; the feed keeps the bigger news and a sample of foundings.
  const shown = events
    .filter((e) => (e.kind === 'clade_founded' ? e.id % 8 === 0 : e.kind !== 'clade_extinct' || e.data.named === true))
    .slice(0, 40);
  $('feed').innerHTML =
    `<h2>${t.allEvents}</h2>` +
    (shown.length === 0
      ? `<p class="empty">${t.noEvents}</p>`
      : `<ul class="feed">${shown
          .map(
            (e) =>
              `<li class="${MAJOR.has(e.kind) ? 'major' : ''}${typeof e.data.cell === 'number' ? ' place' : ''}"${typeof e.data.cell === 'number' ? ` data-cell="${e.data.cell}" title="${t.showOnMap}"` : ''}><i class="ph ${EVENT_ICONS[e.kind] ?? 'ph-dot-outline'}"></i><span>${eventText(e)}</span><span class="when">${dayOf(e.epoch).toFixed(2)}</span></li>`,
          )
          .join('')}</ul>`);
  $('feed').querySelectorAll<HTMLElement>('li.place').forEach((li) =>
    li.addEventListener('click', (ev) => {
      if ((ev.target as HTMLElement).closest('a')) return;
      map.focus(Number(li.dataset.cell), 18);
    }),
  );
}

// ---------------------------------------------------------------- cards

function sparkline(history: [number, number][]): string {
  if (history.length < 2) return '';
  const e0 = history[0][0];
  const span = Math.max(1, history[history.length - 1][0] - e0);
  const maxN = Math.max(...history.map((h) => h[1]), 1);
  const pts = history.map(([e, n]) => `${(((e - e0) / span) * 300).toFixed(1)},${(62 - (n / maxN) * 56).toFixed(1)}`);
  return `<svg class="spark" viewBox="0 0 300 64" preserveAspectRatio="none" role="img" aria-label="population"><polygon points="0,64 ${pts.join(' ')} 300,64"/><polyline points="${pts.join(' ')}"/></svg>`;
}

function traitsTable(traits: number[]): string {
  return `<div class="traits">${traits
    .map((v, i) => `<span>${t.traitNames[i]}</span><span class="t"><i style="width:${Math.max(2, (v / 8) * 100)}%"></i></span><span class="v">${v}</span>`)
    .join('')}</div>`;
}

const closeBtn = `<button class="icon-btn close" aria-label="${t.close}"><i class="ph ph-x"></i></button>`;

function renderClade(c: CladeInfo) {
  const status = c.extinct_epoch !== null ? fmt(t.extinctShort, { epoch: c.extinct_epoch }) : `${nf.format(c.living)}`;
  const kids = (c.children ?? []).slice(0, 20).map((id) => `<a href="${link({ ...route, clade: id, organism: undefined })}">${id}</a>`).join('');
  if (c.name) names.set(c.id, c.name);
  if (c.parent_name) names.set(c.parent_id, c.parent_name);
  const kind = kindOf(c.reference.traits);
  const kindName = { grazer: t.legendGrazer, armored: t.legendArmored, hunter: t.legendHunter }[kind];
  return `<div class="card-head"><div class="who">${portrait(kind, 'avatar')}<div><h3><span class="swatch" style="background:${css(hueColor(c.reference.hue))}"></span>${c.name ? `<em class="latin">${esc(c.name)}</em>` : fmt(t.clade, { id: c.id })}</h3>
    <div class="subtitle">${c.name ? `${fmt(t.cladeNumber, { id: c.id })}, ` : ''}${kindName}, ${t.habitat}: ${t.habitats[c.reference.habitat] ?? '?'}</div></div></div>${closeBtn}</div>
    ${sparkline(c.history ?? [])}
    <div class="facts">
      <div class="fact">${c.extinct_epoch !== null ? t.statusLabel : t.living_}<b class="num">${status}</b></div>
      <div class="fact">${t.peakLabel}<b class="num">${nf.format(c.peak_living)}</b></div>
      <div class="fact">${t.foundedLabel}<b>${t.day} <span class="num">${dayOf(c.founded_epoch).toFixed(2)}</span></b></div>
      <div class="fact">${t.parent}<b>${c.parent_id ? `<a href="${link({ ...route, clade: c.parent_id, organism: undefined })}">${cladeLabel(c.parent_id)}</a>` : '-'}</b></div>
    </div>
    ${kids ? `<div class="fact" style="margin-top:14px">${t.children}<div class="links" style="margin-top:4px">${kids}</div></div>` : ''}
    ${c.extinct_epoch === null && route.epoch === undefined ? predictHtml(c) : ''}
    ${c.extinct_epoch === null && route.epoch === undefined ? `<button class="btn" data-migrate="${c.id}" style="margin-top:12px"><i class="ph ph-sparkle"></i>${t.migrateButton}</button><div class="st" id="migrate-note"></div>` : ''}
    ${traitsTable(c.reference.traits)}`;
}

// ---------------------------------------------------------------- predictions for tomorrow

function predictHtml(c: CladeInfo): string {
  const list = predictions.load();
  const row = (question: predictions.Question, text: string) => {
    const p = predictions.pending(list, c.id, question);
    const right = p
      ? `<span class="done">${fmt(t.predictPending, { answer: p.answer ? t.yes : t.no, day: dayOf(p.due).toFixed(2) })}</span>`
      : `<span class="btns"><button class="btn" data-q="${question}" data-a="1">${t.yes}</button><button class="btn" data-q="${question}" data-a="0">${t.no}</button></span>`;
    return `<div class="q"><span>${text}</span>${right}</div>`;
  };
  return `<div class="predict" data-clade="${c.id}" data-living="${c.living}"><h4>${t.predictTitle}</h4>${row('survive', t.predictSurvive)}${row('grow', fmt(t.predictGrow, { n: c.living }))}</div>`;
}

function bindPredict(el: HTMLElement) {
  el.querySelectorAll<HTMLButtonElement>('.predict .btn').forEach((b) =>
    b.addEventListener('click', async () => {
      const box = b.closest<HTMLElement>('.predict')!;
      track('prediction', b.dataset.q);
      predictions.make(
        Number(box.dataset.clade),
        b.dataset.q as predictions.Question,
        b.dataset.a === '1',
        world.header.epoch,
        world.epochs_per_day,
        Number(box.dataset.living),
      );
      await renderDetails();
      renderPredictions();
    }),
  );
}

function renderPredictions() {
  const list = predictions.load();
  const el = $('predictions');
  if (list.length === 0) {
    el.hidden = true;
    return;
  }
  const settled = list.filter((p) => p.correct !== undefined);
  const right = settled.filter((p) => p.correct).length;
  const recent = list.slice(-6).reverse();
  el.hidden = false;
  el.innerHTML = `<h2>${t.predictionsTitle}</h2>
    <div class="subtitle">${fmt(t.predictionsScore, { right, settled: settled.length, pending: list.length - settled.length })}</div>
    <ul class="plist">${recent
      .map((p) => {
        const r = p.correct === undefined ? `<span class="r wait">${t.predictionWaiting}</span>` : p.correct ? `<span class="r ok">${t.predictionRight}</span>` : `<span class="r bad">${t.predictionWrong}</span>`;
        return `<li><span><a href="${link({ ...route, clade: p.clade, organism: undefined })}">${cladeLabel(p.clade)}</a>: ${p.question === 'survive' ? t.qSurvive : t.qGrow}, ${p.answer ? t.yes : t.no}</span>${r}</li>`;
      })
      .join('')}</ul>`;
}

function renderOrganism(o: OrganismInfo) {
  const status = o.living
    ? fmt(t.alive, { age: o.living.age, energy: (o.living.energy / 100).toFixed(0) })
    : o.died_epoch !== null
      ? fmt(t.died, { epoch: o.died_epoch, cause: t.causes[o.cause ?? ''] ?? o.cause ?? '?' })
      : '';
  const kids = o.offspring.slice(0, 16).map((id) => `<a href="${link({ ...route, organism: id, clade: undefined })}">${id}</a>`).join('');
  return `<div class="card-head"><div class="who">${portrait(kindOf(o.genome.traits), 'avatar')}<div><h3><span class="swatch" style="background:${css(hueColor(o.genome.hue))}"></span>${fmt(t.organism, { id: o.id })}</h3>
    <div class="subtitle">${status}</div></div></div>${closeBtn}</div>
    <div class="facts">
      <div class="fact">${t.clade.replace(' {id}', '')}<b><a href="${link({ ...route, clade: o.clade_id, organism: undefined })}">${cladeLabel(o.clade_id)}</a></b></div>
      <div class="fact">${t.bornLabel}<b>${t.epoch} <span class="num">${o.born_epoch}</span></b></div>
      <div class="fact">${t.parentLabel}<b>${o.parent_id ? `<a href="${link({ ...route, organism: o.parent_id, clade: undefined })}">#${o.parent_id}</a>` : '-'}</b></div>
      <div class="fact">${t.habitat}<b>${t.habitats[o.genome.habitat] ?? '?'}</b></div>
    </div>
    ${kids ? `<div class="fact" style="margin-top:14px">${t.offspring}<div class="links" style="margin-top:4px">${kids}</div></div>` : ''}
    <div class="proof"><button class="btn" id="proof-btn"><i class="ph ph-seal-check"></i>${t.proofCheck}</button><div id="proof-out"></div></div>
    ${traitsTable(o.genome.traits)}`;
}

/** Checks in the browser that an organism is part of the state on the map (spec §15). */
async function checkProof(id: number) {
  const out = $('proof-out');
  const epoch = route.epoch ?? world.header.epoch;
  if (epoch !== world.header.epoch && epoch % world.archive_every !== 0) {
    out.innerHTML = fmt(t.proofNotHere, { every: world.archive_every });
    return;
  }
  out.textContent = t.proofChecking;
  try {
    const proof = await api.proof(epoch, id);
    const header = await api.epochHeader(proof.epoch);
    const ok = await timeMachine.verifyProof(proof, header.state_root);
    out.innerHTML = ok
      ? `<i class="ph ph-seal-check ok"></i>${fmt(t.proofOk, { id, epoch: nf.format(proof.epoch), n: proof.path.length, size: nf.format(proof.size) })}`
      : `<i class="ph ph-warning bad"></i>${t.proofBad}`;
    track('view', 'proof');
  } catch (e) {
    out.innerHTML =
      e instanceof ApiError && e.status === 404 && e.code === 'E_NOT_FOUND'
        ? fmt(t.proofNotAlive, { id, epoch: nf.format(epoch) })
        : esc(fmt(t.error, { message: e instanceof Error ? e.message : String(e) }));
  }
}

async function renderDetails() {
  const el = $('details');
  try {
    if (route.organism !== undefined) {
      const o = await api.organism(route.organism);
      el.innerHTML = renderOrganism(o);
      $('proof-btn').addEventListener('click', () => checkProof(o.id));
      map.highlight(undefined, o.id);
      const cell = map.cellOfOrganism(o.id);
      if (cell !== undefined) map.focus(cell);
    } else if (route.clade !== undefined) {
      el.innerHTML = renderClade(await api.clade(route.clade));
      map.highlight(route.clade, undefined);
      if ((route.view ?? 'map') === 'map') map.focusClade(route.clade);
    } else {
      el.hidden = true;
      map.highlight();
      return;
    }
    el.hidden = false;
    el.querySelector('.close')?.addEventListener('click', () => go({ epoch: route.epoch }));
    bindPredict(el);
    el.querySelector<HTMLButtonElement>('[data-migrate]')?.addEventListener('click', (ev) => {
      const clade = Number((ev.currentTarget as HTMLElement).dataset.migrate);
      const d = densest(clade);
      if (!d || d.n < 10) {
        $('migrate-note').textContent = fmt(t.migrateTooFew, { n: d?.n ?? 0 });
        return;
      }
      plan({ kind: 'migrate', clade, from: d.at });
    });
  } catch (e) {
    el.hidden = false;
    el.innerHTML = `<div class="card-head"><span class="subtitle">${esc(e instanceof Error ? e.message : String(e))}</span>${closeBtn}</div>`;
    el.querySelector('.close')?.addEventListener('click', () => go({ epoch: route.epoch }));
  }
}

// ---------------------------------------------------------------- the hover tip

map.onHover = (hover, x, y) => {
  const tip = $('tip');
  if (hover === null || !state) {
    tip.hidden = true;
    return;
  }
  const cell = hover.cell;
  const o = state.organisms;
  const i = hover.organism !== undefined ? o.id.indexOf(hover.organism) : -1;
  const who =
    i >= 0
      ? `<div class="tip-org"><span class="swatch" style="background:${css(hueColor(o.hue[i]))}"></span><b>${fmt(t.organism, { id: o.id[i] })}</b><span>${[t.legendGrazer, t.legendArmored, t.legendHunter][o.kind[i]]}, ${names.has(o.clade[i]) ? `<em class="latin">${esc(names.get(o.clade[i])!)}</em>` : fmt(t.cladeOf, { id: o.clade[i] })}</span></div>`
      : '';
  const count = state.organisms.cell.filter((c) => c === cell).length;
  const rift = state.rift[cell] ? `<div class="row"><span>${t.layerRifts.split(' ')[0]}</span><span>${t.riftPhases[state.rift[cell]]}</span></div>` : '';
  tip.innerHTML = `${who}<strong>${t.biomes[state.biome[cell]]}</strong>
    <div class="row"><span>${fmt(t.cell, { x: cell % state.width, y: Math.floor(cell / state.width) })}</span><span></span></div>
    <div class="row"><span>${t.food}</span><span>${(state.food[cell] / 10).toFixed(0)}</span></div>
    <div class="row"><span>${t.moisture}</span><span>${state.moisture[cell]}</span></div>
    <div class="row"><span>${t.organismsHere}</span><span>${count}</span></div>${rift}`;
  tip.hidden = false;
  const w = tip.offsetWidth;
  tip.style.left = `${x + 16 + w > window.innerWidth ? x - w - 12 : x + 16}px`;
  tip.style.top = `${y + 16}px`;
};

map.onPick = (p) => {
  if (nat.picking && nat.plan) {
    const cell = p.kind === 'cell' ? p.cell : map.cellOfOrganism(p.id);
    if (cell !== undefined) {
      nat.at = [cell % world.width, Math.floor(cell / world.width)];
      nat.picking = false;
      banner(null);
      renderNaturalist();
    }
    return;
  }
  if (p.kind === 'organism') go({ epoch: route.epoch, organism: p.id });
  else go({ epoch: route.epoch });
};

// ---------------------------------------------------------------- the naturalist (stage C)

/** What the naturalist is about to wish for, before the place is picked on the map. */
type Plan = { kind: 'weather'; rain: boolean } | { kind: 'migrate'; clade: number; from: [number, number] } | { kind: 'revive'; entry: number };

const nat: {
  open: boolean;
  plan?: Plan;
  /** Waiting for a click on the map. */
  picking: boolean;
  at?: [number, number];
  progress?: naturalist.Progress;
  error?: string;
  wishes: naturalist.WishRow[];
  price: bigint;
  busy: boolean;
} = { open: false, picking: false, wishes: [], price: 0n, busy: false };

/** Starts a plan: opens the panel and asks for a click on the map. */
function plan(p: Plan) {
  nat.plan = p;
  nat.at = undefined;
  nat.picking = true;
  nat.open = true;
  nat.error = undefined;
  const ask =
    p.kind === 'weather'
      ? fmt(t.wishPick, { what: (p.rain ? t.wishRain : t.wishDrought).toLowerCase() })
      : p.kind === 'migrate'
        ? fmt(t.migratePick, { clade: cladeLabel(p.clade) })
        : fmt(t.revivePick, { clade: cladeLabel(p.entry) });
  banner(ask);
  if ((route.view ?? 'map') !== 'map' || route.epoch !== undefined) go({ ...route, epoch: undefined, view: 'map' });
  refreshWishes();
}

/** The densest 5 × 5 area of a clade on the map: its center and how many of the clade are in it. */
function densest(clade: number): { at: [number, number]; n: number } | undefined {
  if (!state) return undefined;
  const o = state.organisms;
  const cells: [number, number][] = [];
  for (let i = 0; i < o.id.length; i++) if (o.clade[i] === clade) cells.push([o.cell[i] % state.width, Math.floor(o.cell[i] / state.width)]);
  let best: { at: [number, number]; n: number } | undefined;
  for (const c of cells) {
    const n = cells.filter((d) => Math.abs(d[0] - c[0]) <= 2 && Math.abs(d[1] - c[1]) <= 2).length;
    if (!best || n > best.n) best = { at: c, n };
  }
  return best;
}

/** The wish of the plan and the picked place, as the core takes it. */
function actionOf(p: Plan, at: [number, number]): naturalist.Action {
  if (p.kind === 'weather') return { weather: { x: at[0], y: at[1], rain: p.rain } };
  if (p.kind === 'migrate') return { migrate: { clade_id: p.clade, from: p.from, to: at } };
  return { revive: { museum: true, entry_id: p.entry, steps: [], at } };
}

const big = (n: bigint) => nf.format(Number(n));

async function refreshWishes() {
  try {
    const [rows, l] = await Promise.all([naturalist.mine(), naturalist.ledger()]);
    nat.wishes = rows;
    nat.price = l.price;
  } catch (e) {
    nat.error = e instanceof Error ? e.message : String(e);
  }
  renderNaturalist();
}

async function renderNaturalist() {
  const el = $('naturalist');
  $('wish-btn').setAttribute('aria-pressed', String(nat.open));
  if (!nat.open) {
    el.hidden = true;
    return;
  }
  const key = await naturalist.me();
  const parts: string[] = [
    `<div class="card-head"><h2>${t.naturalistTitle}</h2><button class="icon-btn close" aria-label="${t.close}"><i class="ph ph-x"></i></button></div>`,
    `<div class="subtitle">${fmt(t.naturalistKey, { key: `<code>${key.slice(0, 8)}…${key.slice(-4)}</code>` })}</div>`,
    `<div class="wish-kinds"><button class="btn" data-rain="1" aria-pressed="${nat.plan?.kind === 'weather' && nat.plan.rain}"><i class="ph ph-cloud-rain"></i>${t.wishRain}</button><button class="btn" data-rain="0" aria-pressed="${nat.plan?.kind === 'weather' && !nat.plan.rain}"><i class="ph ph-sun"></i>${t.wishDrought}</button></div>`,
    `<p class="st" style="margin:0;font-size:12px;color:var(--text-3)">${t.wishOthers}</p>`,
  ];
  if (nat.plan && nat.at) {
    const [x, y] = nat.at;
    const p = nat.plan;
    const mult = p.kind === 'weather' ? 100n : p.kind === 'migrate' ? 120n : 200n;
    const price = big((nat.price * mult) / 100n);
    const text =
      p.kind === 'weather'
        ? fmt(t.wishPreview, { what: p.rain ? t.wishRain : t.wishDrought, x, y, price, threads: naturalist.threads() })
        : p.kind === 'migrate'
          ? fmt(t.migratePreview, { clade: cladeLabel(p.clade), fx: p.from[0], fy: p.from[1], x, y, price, threads: naturalist.threads() })
          : fmt(t.revivePreview, { clade: cladeLabel(p.entry), x, y, price, threads: naturalist.threads() });
    parts.push(`<div class="wish-box">${text}
      <div class="actions"><button class="btn primary" id="wish-make"${nat.busy ? ' disabled' : ''}>${t.wishMake}</button><button class="btn" id="wish-cancel">${t.cancel}</button></div></div>`);
  }
  const p = nat.progress;
  if (naturalist.mining() && p) {
    parts.push(`<div class="wish-box">${fmt(t.kindling, { wish: `<code>${p.proposal.slice(0, 8)}</code>`, n: nf.format(p.accepted), work: big(p.work), rate: p.rate.toFixed(0) })}
      <div class="actions"><button class="btn" id="wish-stop">${t.kindlingStop}</button></div></div>`);
  }
  if (nat.error) parts.push(`<div class="wish-box error">${esc(nat.error)}</div>`);
  parts.push(`<h3 style="margin:14px 0 6px;font-size:13px">${t.myWishes}</h3>`);
  if (nat.wishes.length === 0) parts.push(`<p class="empty">${t.noWishes}</p>`);
  else
    parts.push(
      `<ul class="wish-list">${nat.wishes
        .slice(0, 12)
        .map((w) => {
          const price = (nat.price * BigInt(w.price_mult)) / 100n;
          const work = BigInt(w.work);
          const pct = price > 0n ? Math.min(100, Number((work * 100n) / price)) : 0;
          const live = w.status === 'open' || w.status === 'ready';
          return `<li><div class="top"><span>${t.actionName[w.action] ?? w.action} <code>${w.id.slice(0, 8)}</code></span><span class="st ${w.status}">${t.status[w.status] ?? w.status}${w.reason ? `: ${esc(w.reason)}` : ''}</span></div>
            ${live ? `<div class="bar"><i style="width:${pct}%"></i></div><div class="st">${fmt(t.wishWork, { work: big(work), price: big(price) })}</div>` : ''}
            ${live && naturalist.mining() !== w.id ? `<button class="btn" data-support="${w.id}">${t.support}</button>` : ''}</li>`;
        })
        .join('')}</ul>`,
    );
  el.innerHTML = parts.join('');
  el.hidden = false;
  el.querySelector('.close')?.addEventListener('click', () => {
    nat.open = false;
    nat.picking = false;
    nat.plan = undefined;
    banner(null);
    renderNaturalist();
  });
  el.querySelectorAll<HTMLButtonElement>('.wish-kinds .btn').forEach((b) =>
    b.addEventListener('click', () => plan({ kind: 'weather', rain: b.dataset.rain === '1' })),
  );
  el.querySelector('#wish-cancel')?.addEventListener('click', () => {
    nat.plan = undefined;
    nat.at = undefined;
    renderNaturalist();
  });
  el.querySelector('#wish-make')?.addEventListener('click', async () => {
    const action = actionOf(nat.plan!, nat.at!);
    nat.busy = true;
    nat.error = undefined;
    renderNaturalist();
    try {
      await naturalist.wish(action);
      nat.plan = undefined;
      nat.at = undefined;
      track('prediction', 'wish');
    } catch (e) {
      nat.error = e instanceof Error ? e.message : String(e);
    }
    nat.busy = false;
    await refreshWishes();
  });
  el.querySelector('#wish-stop')?.addEventListener('click', () => {
    naturalist.stop();
    nat.progress = undefined;
    renderNaturalist();
  });
  el.querySelectorAll<HTMLButtonElement>('[data-support]').forEach((b) =>
    b.addEventListener('click', async () => {
      nat.error = undefined;
      try {
        await naturalist.support(b.dataset.support!);
      } catch (e) {
        nat.error = e instanceof Error ? e.message : String(e);
      }
      renderNaturalist();
    }),
  );
}

naturalist.onProgress((p) => {
  if (p.forged) {
    nat.error = t.forged;
    nat.progress = undefined;
  } else {
    nat.progress = p;
    if (p.error) nat.error = p.error;
  }
  renderNaturalist();
});

// ---------------------------------------------------------------- loading

async function showEpoch(epoch: number | undefined, animate: boolean) {
  if (epoch === undefined) pastHeader = undefined;
  else if (epoch !== world.header.epoch && epoch % world.archive_every !== 0) return travel(epoch, animate);
  try {
    const next = await api.map(epoch);
    learnNames(next.names);
    if (epoch !== undefined) pastHeader = await api.epochHeader(epoch).catch(() => undefined);
    state = next;
    map.setState(next, animate);
    banner(null);
  } catch (e) {
    if (e instanceof ApiError && e.code === 'E_NO_SNAPSHOT' && epoch !== undefined) {
      const { every, epochs } = await api.snapshots();
      const near = epochs.reduce((a, b) => (Math.abs(b - epoch) < Math.abs(a - epoch) ? b : a), epochs[0] ?? 0);
      banner(fmt(t.noSnapshot, { epoch, every, near: `<a href="${link({ ...route, epoch: near })}">${near}</a>` }));
      return;
    }
    banner(esc(fmt(t.error, { message: e instanceof Error ? e.message : String(e) })), true);
  }
}

// ---------------------------------------------------------------- the time machine (B7)

let treeNames: Promise<Record<string, string>> | undefined;
/** The names of all named clades of the season, for recomputed states. */
function allNames(): Promise<Record<string, string>> {
  treeNames ??= api.tree().then(({ clades }) => Object.fromEntries(clades.filter((c) => c[6]).map((c) => [String(c[0]), c[6] as string])));
  return treeNames;
}

/** The miracles given to the world in epochs `from` to `to`, as the core's JSON per epoch. */
async function miraclesBetween(from: number, to: number): Promise<Record<number, string>> {
  if (to < from) return {};
  const { miracles } = await api.miracles(from, to);
  const by: Record<number, unknown[]> = {};
  for (const m of miracles) (by[m.epoch] ??= []).push(m.miracle);
  return Object.fromEntries(Object.entries(by).map(([e, list]) => [Number(e), JSON.stringify(list)]));
}

/** Recomputes a past epoch in the browser from the nearest earlier snapshot and checks it. */
async function travel(epoch: number, animate: boolean) {
  const base = Math.floor(epoch / world.archive_every) * world.archive_every;
  const cont = state !== undefined && state.epoch < epoch && Math.floor(state.epoch / world.archive_every) * world.archive_every === base;
  banner(fmt(t.tmComputing, { epoch, base, pct: 0 }));
  try {
    const c = await timeMachine.compute(epoch, base, api.snapshotBytes, miraclesBetween, (done, total) =>
      banner(fmt(t.tmComputing, { epoch, base, pct: total ? Math.round((done / total) * 100) : 100 })),
    );
    if (route.epoch !== epoch) return;
    const [named, header] = await Promise.all([allNames(), api.epochHeader(epoch).catch(() => undefined)]);
    const present = new Set(c.state.organisms.clade);
    c.state.names = Object.fromEntries(Object.entries(named).filter(([id]) => present.has(Number(id))));
    learnNames(c.state.names);
    pastHeader = header;
    state = c.state;
    map.setState(c.state, animate);
    if (!header) banner(null);
    else if (header.state_root === c.state.state_root) {
      const text = cont ? fmt(t.tmContinued, { epoch }) : fmt(t.tmVerified, { epoch, base, n: c.stepped, s: (c.ms / 1000).toFixed(1) });
      banner(`<i class="ph ph-seal-check ok"></i>${text}`);
    } else banner(fmt(t.tmMismatch, { epoch, mine: c.state.state_root.slice(0, 16), theirs: header.state_root.slice(0, 16) }), true);
    track('view', 'time-machine');
  } catch (e) {
    banner(esc(fmt(t.tmFailed, { epoch, message: e instanceof Error ? e.message : String(e) })), true);
  }
}

/** Moves the map `by` epochs; past the latest epoch it goes live. */
function stepBy(by: number) {
  const latest = world.header.epoch;
  const target = Math.max(0, (route.epoch ?? latest) + by);
  go({ ...route, epoch: target >= latest ? undefined : target });
}

// ---------------------------------------------------------------- the museum and the chronicle (B7)

let museumOrder: 'great' | 'recent' = 'great';

async function renderMuseum() {
  const host = $('museum');
  const { museum } = await api.museum();
  for (const c of museum) if (c.name) names.set(c.id, c.name);
  const rows = museum.slice().sort((a, b) => (museumOrder === 'great' ? b.peak_living - a.peak_living : (b.extinct_epoch ?? 0) - (a.extinct_epoch ?? 0)));
  const order = (o: typeof museumOrder, label: string) => `<button class="btn" data-order="${o}" aria-pressed="${museumOrder === o}">${label}</button>`;
  host.innerHTML = `<p class="hint">${t.museumHint}</p>
    <div class="museum-bar"><span>${fmt(t.museumCount, { n: nf.format(rows.length) })}</span>${order('great', t.museumGreat)}${order('recent', t.museumRecent)}</div>
    ${
      rows.length === 0
        ? `<p class="empty" style="margin:0 4px">${t.museumEmpty}</p>`
        : `<div class="museum-grid">${rows
            .slice(0, 300)
            .map((c) => {
              const died = c.extinct_epoch ?? 0;
              const last = Math.max(0, died - 1);
              return `<div class="exhibit">${portrait(kindOf(c.reference.traits), 'avatar')}<div>
                <h4><span class="swatch" style="background:${css(hueColor(c.reference.hue))}"></span><a href="${link({ ...route, clade: c.id, organism: undefined })}">${cladeLabel(c.id)}</a></h4>
                <div class="meta">${fmt(t.museumLived, { from: dayOf(c.founded_epoch).toFixed(1), to: dayOf(died).toFixed(1), time: duration(died - c.founded_epoch) })}<br>
                <b>${fmt(t.museumPeak, { n: nf.format(c.peak_living) })}</b> · ${t.habitats[c.reference.habitat] ?? '?'}</div>
                <a class="last" href="${link({ epoch: last >= world.header.epoch ? undefined : last, clade: c.id, view: 'map' })}">${t.museumLast}</a>
                ${
                  world.header.epoch >= died + 36
                    ? `<button class="btn revive" data-revive="${c.id}"><i class="ph ph-sparkle"></i>${t.reviveButton}</button>`
                    : `<div class="meta">${fmt(t.reviveLater, { epoch: nf.format(died + 36) })}</div>`
                }
              </div></div>`;
            })
            .join('')}</div>`
    }`;
  host.querySelectorAll<HTMLButtonElement>('[data-revive]').forEach((b) =>
    b.addEventListener('click', () => plan({ kind: 'revive', entry: Number(b.dataset.revive) })),
  );
  host.querySelectorAll<HTMLButtonElement>('.museum-bar .btn').forEach((b) =>
    b.addEventListener('click', () => {
      museumOrder = b.dataset.order as typeof museumOrder;
      renderMuseum();
    }),
  );
}

let chronicleDays = 7;

async function renderChronicle() {
  const host = $('chronicle');
  const per = world.epochs_per_day;
  const now = route.epoch ?? world.header.epoch;
  const today = Math.floor(now / per);
  const first = Math.max(0, today - chronicleDays + 1);
  const days: number[] = [];
  for (let d = today; d >= first; d--) days.push(d);
  const digests = await Promise.all(days.map((d) => api.digestRange(d * per, Math.min((d + 1) * per, now)).catch(() => null)));
  const pct = (h: Header) => (h.dominant_permille / 10).toFixed(0);
  const articles = days.map((d, i) => {
    const g = digests[i];
    if (!g || !g.then) return '';
    learnNames(g.names);
    const a = g.then;
    const b = g.header;
    const partial = g.now < (d + 1) * per;
    const change = b.population - a.population;
    const lines: string[] = [];
    const big = Math.max(20, a.population / 20);
    lines.push(
      fmt(change > big ? t.chronicleGrew : change < -big ? t.chronicleShrank : t.chronicleSteady, { a: nf.format(a.population), b: nf.format(b.population) }),
    );
    lines.push(
      a.dominant_clade !== b.dominant_clade
        ? fmt(t.chronicleLeaderNew, { clade: cladeLabel(b.dominant_clade), other: cladeLabel(a.dominant_clade), pct: pct(b) })
        : fmt(t.chronicleLeaderKept, { clade: cladeLabel(b.dominant_clade), pct: pct(b) }),
    );
    if (g.counts.clade_named) lines.push(fmt(t.chronicleNamed, { n: g.counts.clade_named }));
    if (g.bridges_closed.length) lines.push(fmt(t.chronicleBridges, { list: g.bridges_closed.map((e) => e.data.bridge).join(', ') }));
    const nature = (
      [
        ['wildfire', t.natureWildfire],
        ['drought', t.natureDrought],
        ['flood', t.natureFlood],
        ['plague', t.naturePlague],
      ] as [string, string][]
    )
      .filter(([k]) => g.counts[k])
      .map(([k, label]) => fmt(label, { n: g.counts[k] }));
    if (nature.length) lines.push(fmt(t.chronicleNature, { list: nature.join(', ') }));
    const phase = t.phases[b.phase - 1] ?? b.phase_name;
    return `<article class="${partial ? 'today' : ''}">
      <h3>${fmt(partial ? t.chronicleToday : t.chronicleDay, { day: d })}<span class="phase">${esc(phase)}</span></h3>
      <p>${lines.join(' ')}</p>
      ${g.stories.length ? `<div class="stories">${g.stories.slice(0, 4).map(storyHtml).join('')}</div>` : `<p>${t.chronicleQuiet}</p>`}
      ${partial ? '' : `<a class="open" href="${link({ ...route, epoch: g.now >= world.header.epoch ? undefined : g.now, view: 'map' })}">${t.chronicleOpen}</a>`}
    </article>`;
  });
  host.innerHTML = `<p class="hint">${t.chronicleHint}</p><div class="chronicle">${articles.join('')}</div>
    ${first > 0 ? `<p style="margin:14px 4px"><button class="btn" id="chronicle-more">${t.chronicleMore}</button></p>` : ''}`;
  $('chronicle').querySelector('#chronicle-more')?.addEventListener('click', () => {
    chronicleDays += 7;
    renderChronicle();
  });
}

// ---------------------------------------------------------------- views: the map, the Muller plot, the clade tree

let treeData: Map<number, TreeClade> | undefined;
let mullerData: MullerData | undefined;
let chartsAt = -1;

async function loadCharts() {
  const epoch = world.header.epoch;
  if (chartsAt === epoch && treeData && mullerData) return;
  const [tree, muller, phases, bridges, revivals] = await Promise.all([
    api.tree(),
    api.muller(),
    api.eventsOfKind('phase'),
    api.eventsOfKind('bridge_closed'),
    api.eventsOfKind('revival'),
  ]);
  treeData = new Map(
    tree.clades.map(([id, parent, founded, extinct, peak, hue, name]) => {
      if (name) names.set(id, name);
      return [id, { id, parent, founded, extinct, peak, hue, name }];
    }),
  );
  const markers: Marker[] = [
    ...phases.events.map((e) => ({ epoch: e.epoch, kind: 'phase' as const, label: esc(t.phases[Number(e.data.phase) - 1] ?? '') })),
    ...bridges.events.map((e) => ({ epoch: e.epoch, kind: 'bridge' as const, label: esc(fmt(t.bridgeMark, { bridge: String(e.data.bridge) })) })),
    ...revivals.events.map((e) => ({ epoch: e.epoch, kind: 'revival' as const, label: esc(t.revivalMark) })),
  ];
  mullerData = { clades: treeData, rows: muller.rows, markers, epochsPerDay: world.epochs_per_day, seasonDays: world.season_days };
  chartsAt = epoch;
}

function renderTabs() {
  const view = route.view ?? 'map';
  const tabs: [View, string, string][] = [
    ['map', t.tabMap, 'ph-map-trifold'],
    ['muller', t.tabMuller, 'ph-chart-line'],
    ['tree', t.tabTree, 'ph-tree-structure'],
    ['chronicle', t.tabChronicle, 'ph-scroll'],
    ['museum', t.tabMuseum, 'ph-bank'],
  ];
  $('tabs').innerHTML = tabs
    .map(([v, name, icon]) => `<a class="tab" role="tab" aria-selected="${v === view}" href="${link({ ...route, view: v })}"><i class="ph ${icon}"></i><span>${name}</span></a>`)
    .join('');
}

async function renderView() {
  const view = route.view ?? 'map';
  renderTabs();
  $('muller').hidden = view !== 'muller';
  $('tree').hidden = view !== 'tree';
  $('museum').hidden = view !== 'museum';
  $('chronicle').hidden = view !== 'chronicle';
  for (const id of ['layers', 'legend', 'minimap', 'replay-btn']) $(id).style.visibility = view === 'map' ? '' : 'hidden';
  document.querySelector<HTMLElement>('.zoom')!.style.visibility = view === 'map' ? '' : 'hidden';
  if (view === 'map') return;
  if (view === 'museum') return renderMuseum();
  if (view === 'chronicle') return renderChronicle();
  await loadCharts();
  const pick = (clade: number) => go({ ...route, clade, organism: undefined });
  const dayLabel = (day: number) => `${t.day} ${day}`;
  if (view === 'muller') {
    const host = $('muller');
    host.innerHTML = `<p class="hint">${t.mullerHint}</p><div class="chart" id="muller-chart"></div>`;
    renderMuller($('muller-chart'), mullerData!, {
      selected: route.clade,
      now: world.header.epoch,
      onPick: pick,
      dayLabel,
      onHover: (clade, share, x, y) => {
        const tip = $('tip');
        if (clade === null) {
          tip.hidden = true;
          return;
        }
        tip.innerHTML = `<strong>${cladeLabel(clade)}</strong><div class="row"><span>${fmt(t.shareNow, { share: (share * 100).toFixed(1) })}</span></div>`;
        tip.hidden = false;
        tip.style.left = `${x + 14}px`;
        tip.style.top = `${y + 14}px`;
      },
    });
  } else {
    const host = $('tree');
    host.innerHTML = `<p class="hint">${t.treeHint}</p><div id="tree-chart"></div>`;
    renderTree($('tree-chart'), treeData!, {
      now: world.header.epoch,
      epochsPerDay: world.epochs_per_day,
      seasonDays: world.season_days,
      selected: route.clade,
      onPick: pick,
      dayLabel,
      label: (c) => (c.name ? esc(c.name) : fmt(t.clade, { id: c.id })),
    });
  }
}

async function onRoute() {
  const previous = route;
  route = readRoute();
  if (route.clade !== undefined && route.clade !== previous.clade) track('card', `clade`);
  if (route.organism !== undefined && route.organism !== previous.organism) track('card', `organism`);
  if (route.view && route.view !== 'map' && route.view !== previous.view) track('view', route.view);
  if (route.epoch !== previous.epoch || !state) {
    const by = (route.epoch ?? world.header.epoch) - (previous.epoch ?? world.header.epoch);
    await showEpoch(route.epoch, by > 0 && by <= 12);
  }
  renderClock();
  renderSeason();
  renderStats(route.epoch !== undefined && pastHeader ? pastHeader : world.header);
  const moved = route.epoch !== previous.epoch;
  await Promise.all([renderView(), renderDetails(), renderCompare(), moved ? renderStories() : null, moved ? renderFeed() : null]);
}

/** Live mode: when the world moves on, the map follows and the organisms walk to their new cells. */
async function poll() {
  try {
    const next = await api.world();
    const moved = next.header.epoch !== world.header.epoch;
    world = next;
    rememberSeen();
    if (moved && (await predictions.resolve(world.header.epoch))) renderPredictions();
    if (moved && route.epoch === undefined && !replaying) {
      await showEpoch(undefined, true);
      renderStats(world.header);
      renderSeason();
      await Promise.all([renderStories(), renderFeed()]);
      if (route.view && route.view !== 'map' && world.header.epoch % 12 === 0) await renderView();
      if (route.clade !== undefined || route.organism !== undefined) await renderDetails();
      if (route.cmp !== undefined) await renderCompare();
    }
  } catch (e) {
    banner(esc(fmt(t.error, { message: e instanceof Error ? e.message : String(e) })), true);
  }
}

// ---------------------------------------------------------------- a welcome for first-time viewers

const WELCOMED_KEY = 'protogaea.welcomed';
const BOT_URL = 'https://t.me/newsbuild_bot';

function showWelcome() {
  const el = $('digest');
  const bot = `<a href="${BOT_URL}" target="_blank" rel="noopener">@${BOT_URL.split('/').pop()}</a>`;
  el.innerHTML = `<div class="digest" role="dialog" aria-modal="true">
    <h2>${t.welcomeTitle}</h2>
    <p class="lead">${t.welcomeLead}</p>
    <ul class="welcome-list">${t.welcomeItems
      .map(([icon, text]) => `<li><i class="ph ${icon}"></i><span>${text.replace('{bot}', bot)}</span></li>`)
      .join('')}</ul>
    <div class="actions"><button class="btn primary" id="digest-close">${t.welcomeGo}</button></div>
  </div>`;
  el.hidden = false;
  const close = () => {
    el.hidden = true;
    try {
      localStorage.setItem(WELCOMED_KEY, '1');
    } catch {
      /* shown again next time */
    }
  };
  $('digest-close').addEventListener('click', close);
  el.addEventListener('click', (ev) => {
    if (ev.target === el) close();
  });
}

function welcomed(): boolean {
  try {
    return localStorage.getItem(WELCOMED_KEY) === '1';
  } catch {
    return true;
  }
}

// ---------------------------------------------------------------- "While you were away"

const SEEN_KEY = 'protogaea.lastSeen';
function rememberSeen() {
  try {
    localStorage.setItem(SEEN_KEY, JSON.stringify({ epoch: world.header.epoch, world: world.world_id }));
  } catch {
    /* no storage, no digest */
  }
}
function lastSeen(): number | undefined {
  try {
    const v = JSON.parse(localStorage.getItem(SEEN_KEY) ?? 'null');
    return v && v.world === world.world_id ? Number(v.epoch) : undefined;
  } catch {
    return undefined;
  }
}

async function showDigest(since: number) {
  const d = await api.digest(since);
  track('digest');
  learnNames(d.names);
  const el = $('digest');
  const minutes = ((d.now - d.since) * 24 * 60) / world.epochs_per_day;
  const time = minutes >= 90 ? fmt(t.hours, { h: Math.round(minutes / 60) }) : fmt(t.minutes, { m: Math.round(minutes) });
  const popThen = d.then?.population;
  const leaderThen = d.then?.dominant_clade;
  const leaderNow = d.header.dominant_clade;
  el.innerHTML = `<div class="digest" role="dialog" aria-modal="true">
    <h2>${t.awayTitle}</h2>
    <p class="lead">${fmt(t.awayFor, { time, from: dayOf(d.since).toFixed(1), to: dayOf(d.now).toFixed(1) })}</p>
    <div class="numbers">
      <div>${t.awayPopulation}<b>${popThen !== undefined ? `${nf.format(popThen)} → ` : ''}${nf.format(d.header.population)}</b></div>
      <div>${t.awayNamed}<b>${d.counts.clade_named ?? 0}</b></div>
      <div>${t.awayExtinct}<b>${d.counts.clade_extinct ?? 0}</b></div>
    </div>
    <div class="fact">${t.awayLeader}<b>${leaderThen !== undefined && leaderThen !== leaderNow ? `${cladeLabel(leaderThen)} → ` : ''}${cladeLabel(leaderNow)}</b></div>
    ${d.bridges_closed.length ? `<div class="fact" style="margin-top:10px">${t.awayBridges}<b>${d.bridges_closed.map((e) => e.data.bridge).join(', ')}</b></div>` : ''}
    <h3>${t.awayStories}</h3>
    ${d.stories.length ? `<div class="stories">${d.stories.map(storyHtml).join('')}</div>` : `<p class="empty">${t.awayNoStories}</p>`}
    <div class="actions"><button class="btn primary" id="digest-close">${t.awayClose}</button></div>
  </div>`;
  el.hidden = false;
  const close = () => {
    el.hidden = true;
  };
  $('digest-close').addEventListener('click', close);
  el.addEventListener('click', (ev) => {
    if (ev.target === el) close();
  });
  el.querySelectorAll('a').forEach((a) => a.addEventListener('click', close));
}

// ---------------------------------------------------------------- the last day in thirty seconds

let replaying = false;
let stopReplay = () => {};

async function playReplay() {
  if (replaying) return stopReplay();
  track('replay');
  const bar = $('replay-bar');
  const caption = $('replay-caption');
  bar.hidden = false;
  bar.innerHTML = `<span class="clock">${t.replayLoading}</span>`;
  replaying = true;
  let stopped = false;
  stopReplay = () => {
    stopped = true;
  };
  $('replay-btn').querySelector('span')!.textContent = t.replayStop;
  try {
    const [buf] = await Promise.all([api.replay(2)]);
    const frames = parseFrames(buf);
    if (frames.length < 2 || !state) {
      bar.innerHTML = `<span class="clock">${t.replayEmpty}</span>`;
      await new Promise((r) => setTimeout(r, 2500));
      return;
    }
    const base = state;
    const { stories, names: storyNames } = await api.storiesSince(frames[0].epoch);
    learnNames(storyNames);
    const told = stories.slice().sort((a, b) => a.epoch - b.epoch);
    const perFrame = Math.max(90, 30000 / frames.length);
    for (let i = 0; i < frames.length && !stopped; i++) {
      const f = frames[i];
      map.setState(stateOf(f, base), i > 0, perFrame * 0.95);
      const pct = ((i + 1) / frames.length) * 100;
      bar.innerHTML = `<span class="clock">${t.day} ${dayOf(f.epoch).toFixed(2)}</span><span class="track"><i style="width:${pct}%"></i></span><button class="btn" id="replay-stop">${t.replayStop}</button>`;
      $('replay-stop').addEventListener('click', () => stopReplay());
      const now = told.filter((s) => s.epoch <= f.epoch && s.epoch > (frames[i - 1]?.epoch ?? -1));
      if (now.length) {
        const s = now.sort((a, b) => b.score - a.score)[0];
        const [title] = t.story[s.kind] ?? [s.kind];
        const html = storyHtml(s).replace(/^.*?<p>/s, '').replace(/<\/p>.*$/s, '');
        caption.innerHTML = `<b>${title}</b>${html}`;
        caption.hidden = false;
        setTimeout(() => (caption.hidden = true), 3200);
      }
      await new Promise((r) => setTimeout(r, perFrame));
    }
  } catch (e) {
    banner(esc(fmt(t.error, { message: e instanceof Error ? e.message : String(e) })), true);
  } finally {
    replaying = false;
    bar.hidden = true;
    caption.hidden = true;
    $('replay-btn').querySelector('span')!.textContent = t.replayButton;
    await showEpoch(route.epoch, false);
  }
}

async function boot() {
  document.documentElement.lang = lang;
  await map.init();
  map.attachMinimap($<HTMLCanvasElement>('minimap'));
  map.setLayers(layers);
  window.addEventListener('keydown', (ev) => {
    if ((ev.target as HTMLElement).closest('input, textarea')) return;
    if (ev.key === '+' || ev.key === '=') map.zoomBy(1.5);
    else if (ev.key === '-') map.zoomBy(1 / 1.5);
    else if (ev.key === '0') map.fit();
  });
  world = await api.world();
  $('world-id').textContent = fmt(t.worldId, { seed: world.seed });
  rules = await (await fetch('/v0/ruleset')).json();
  map.setFoodMax(rules.biomes.map((b) => b.food_max));
  rifts = (await api.rifts()).rifts;
  const closes = new Map<number, number>();
  for (const r of rifts) if (r.bridge > 0) closes.set(r.bridge, Math.max(closes.get(r.bridge) ?? 0, r.deep_epoch));
  bridgeCloses = [...closes].sort((a, b) => a[1] - b[1]);
  map.setRifts(rifts);
  renderLayers();
  $('zoom-in').addEventListener('click', () => map.zoomBy(1.5));
  $('zoom-out').addEventListener('click', () => map.zoomBy(1 / 1.5));
  $('zoom-fit').addEventListener('click', () => map.fit());
  $('zoom-in').title = t.zoomIn;
  $('zoom-out').title = t.zoomOut;
  $('zoom-fit').title = t.fit;
  window.addEventListener('hashchange', onRoute);
  $('replay-btn').querySelector('span')!.textContent = t.replayButton;
  $('replay-btn').addEventListener('click', () => playReplay());
  $('wish-btn').querySelector('span')!.textContent = t.wishButton;
  $('wish-btn').addEventListener('click', () => {
    nat.open = !nat.open;
    if (nat.open) refreshWishes();
    else renderNaturalist();
  });
  window.setInterval(() => nat.open && refreshWishes(), 10000);
  $('season').addEventListener('click', (ev) => {
    const track = (ev.target as HTMLElement).closest<HTMLElement>('.season-track');
    if (!track) return;
    const box = track.getBoundingClientRect();
    const day = ((ev.clientX - box.left) / box.width) * rules.season_days;
    const epoch = Math.round(day * world.epochs_per_day);
    go({ ...route, epoch: epoch >= world.header.epoch ? undefined : Math.max(0, epoch) });
  });
  $('season').addEventListener('mousemove', (ev) => {
    const track = (ev.target as HTMLElement).closest<HTMLElement>('.season-track');
    if (!track) return;
    const box = track.getBoundingClientRect();
    track.title = fmt(t.seasonJump, { day: (((ev.clientX - box.left) / box.width) * rules.season_days).toFixed(2) });
  });
  document.addEventListener('keydown', (ev) => {
    if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLTextAreaElement || ev.altKey || ev.ctrlKey || ev.metaKey) return;
    if (ev.key !== 'ArrowLeft' && ev.key !== 'ArrowRight') return;
    ev.preventDefault();
    stepBy((ev.key === 'ArrowLeft' ? -1 : 1) * (ev.shiftKey ? 12 : 1));
  });
  track('visit');
  document.addEventListener('click', (ev) => {
    if ((ev.target as HTMLElement).closest?.('.story a, .replay-caption a')) track('story');
  });
  const seen = lastSeen();
  await onRoute();
  const help = $('help-btn');
  help.title = t.help;
  help.setAttribute('aria-label', t.help);
  help.addEventListener('click', () => showWelcome());
  if (!welcomed()) showWelcome();
  else if (seen !== undefined && world.header.epoch - seen >= 12) showDigest(seen).catch(() => {});
  rememberSeen();
  document.addEventListener('visibilitychange', () => document.visibilityState === 'hidden' && rememberSeen());
  predictions.resolve(world.header.epoch).then(() => renderPredictions());
  renderPredictions();
  await Promise.all([renderStories(), renderFeed()]);
  setInterval(renderClock, 1000);
  setInterval(poll, 5000);
}

boot().catch((e) => banner(esc(fmt(t.error, { message: e instanceof Error ? e.message : String(e) })), true));
