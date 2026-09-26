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

type View = 'map' | 'muller' | 'tree';

interface Route {
  epoch?: number;
  clade?: number;
  organism?: number;
  view?: View;
}

function readRoute(): Route {
  const p = new URLSearchParams(location.hash.slice(1));
  const num = (k: string) => (p.get(k) ? Number(p.get(k)) : undefined);
  const view = p.get('view');
  return {
    epoch: num('epoch'),
    clade: num('clade'),
    organism: num('organism'),
    view: view === 'muller' || view === 'tree' ? view : undefined,
  };
}

function link(r: Route): string {
  const p = new URLSearchParams();
  if (r.epoch !== undefined) p.set('epoch', String(r.epoch));
  if (r.clade !== undefined) p.set('clade', String(r.clade));
  if (r.organism !== undefined) p.set('organism', String(r.organism));
  const view = 'view' in r ? r.view : route.view;
  if (view && view !== 'map') p.set('view', view);
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
    $('clock').innerHTML = `<span class="countdown">${fmt(t.past, { epoch: state?.epoch ?? route.epoch ?? '' })}</span><button class="btn primary" id="to-live"><i class="ph ph-play"></i>${t.jumpToLatest}</button>`;
    $('to-live').addEventListener('click', () => go({ clade: route.clade, organism: route.organism }));
    return;
  }
  const left = Math.max(0, Math.round((world.next_epoch_ms - Date.now()) / 1000));
  const s = left >= 60 ? `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}` : `0:${String(left).padStart(2, '0')}`;
  $('clock').innerHTML = `<span class="countdown">${left > 0 ? fmt(t.nextEpoch, { s: `<span class="num">${s}</span>` }) : t.epochNow}</span><span class="live-pill">${t.live}</span>`;
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
};
const MAJOR = new Set(['phase', 'bridge_closed', 'dominant_changed', 'revival', 'clade_extinct']);

function eventText(e: WorldEvent): string {
  const d = e.data as Record<string, number | string | boolean>;
  const cladeLink = (id: unknown) => `<a href="${link({ ...route, clade: Number(id), organism: undefined })}">${cladeLabel(Number(id))}</a>`;
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
  const { stories, names: storyNames } = await api.stories(5);
  learnNames(storyNames);
  $('stories').innerHTML =
    `<h2>${t.storiesTitle}</h2>` +
    (stories.length === 0 ? `<p class="empty">${t.storiesEmpty}</p>` : `<div class="stories">${stories.map(storyHtml).join('')}</div>`);
}

async function renderFeed() {
  const { events, names: eventNames } = await api.latestEvents(120);
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
    ${traitsTable(o.genome.traits)}`;
}

async function renderDetails() {
  const el = $('details');
  try {
    if (route.organism !== undefined) {
      const o = await api.organism(route.organism);
      el.innerHTML = renderOrganism(o);
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
  if (p.kind === 'organism') go({ epoch: route.epoch, organism: p.id });
  else go({ epoch: route.epoch });
};

// ---------------------------------------------------------------- loading

async function showEpoch(epoch: number | undefined, animate: boolean) {
  try {
    const next = await api.map(epoch);
    learnNames(next.names);
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
  for (const id of ['layers', 'legend', 'minimap', 'replay-btn']) $(id).style.visibility = view === 'map' ? '' : 'hidden';
  document.querySelector<HTMLElement>('.zoom')!.style.visibility = view === 'map' ? '' : 'hidden';
  if (view === 'map') return;
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
  if (route.epoch !== previous.epoch || !state) await showEpoch(route.epoch, false);
  renderClock();
  renderSeason();
  renderStats(world.header);
  await renderView();
  await renderDetails();
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
