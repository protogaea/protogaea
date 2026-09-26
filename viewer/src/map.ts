import { Application, BlurFilter, Container, Graphics, Sprite, Text, Texture } from 'pixi.js';

import type { Effect, MapState, Rift } from './api';
import { drawGlyph, hueColor } from './glyph';
import { glowCanvas, renderTerrain, terrainKey, TERRAIN_PX, type Region } from './terrain';

/** World units per cell at zoom 1. */
const C = 16;
/** Screen pixels per cell from which organisms are drawn one by one, and as glyphs. */
const DOTS_FROM = 14;
const GLYPHS_FROM = 34;
const REDUCED = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
/** How long organisms take to walk to their new cells when an epoch arrives. */
const MOVE_MS = REDUCED ? 0 : 2600;
const MIN_SCALE = 0.3;
/** From this many screen pixels per cell, the visible terrain is redrawn in more detail. */
const DETAIL_FROM = 36;
const MAX_SCALE = 40;

export interface Layers {
  biomes: boolean;
  territories: boolean;
  food: boolean;
  organisms: boolean;
  rifts: boolean;
  events: boolean;
}

export type Pick = { kind: 'organism'; id: number } | { kind: 'cell'; cell: number };
export interface Hover {
  cell: number;
  organism?: number;
}

/** Where each organism stands: a slot of a 2 × 2 grid in its cell, by order of id. */
function slots(state: MapState): Float32Array {
  const o = state.organisms;
  const pos = new Float32Array(o.id.length * 2);
  const filled = new Map<number, number>();
  for (let i = 0; i < o.id.length; i++) {
    const cell = o.cell[i];
    const k = filled.get(cell) ?? 0;
    filled.set(cell, k + 1);
    // A small fixed offset per organism, so they do not stand in rows.
    const h = Math.imul(o.id[i], 2654435761) >>> 0;
    const jx = ((h & 1023) / 1023 - 0.5) * 0.16;
    const jy = (((h >>> 10) & 1023) / 1023 - 0.5) * 0.16;
    pos[i * 2] = ((cell % state.width) + 0.27 + 0.46 * (k % 2) + jx) * C;
    pos[i * 2 + 1] = (Math.floor(cell / state.width) + 0.27 + 0.46 * Math.floor(k / 2) + jy) * C;
  }
  return pos;
}

interface Ghost {
  x: number;
  y: number;
  hue: number;
  traits: number[];
}

interface Camera {
  x: number;
  y: number;
  s: number;
}

interface EventSprite {
  kind: Effect['kind'];
  glow: Sprite;
  core?: Sprite;
  ring?: Graphics;
  r: number;
  phase: number;
  strength: number;
}

interface Herd {
  n: number;
  hunters: number;
  bestN: number;
  hue: number;
  dim: number;
  counts: Map<number, number>;
}

export class WorldMap {
  readonly app = new Application();
  private world = new Container();
  private terrain = new Sprite();
  private detail = new Sprite();
  private detailKey = '';
  private detailTimer = 0;
  private territory = new Sprite();
  private food = new Sprite();
  private range = new Sprite();
  private riftGlow = new Graphics();
  private riftLines = new Graphics();
  private bridges = new Container();
  private events = new Container();
  private orgG = new Graphics();
  private hoverG = new Graphics();
  private markG = new Graphics();
  private glowTex!: Texture;

  private state?: MapState;
  private terrainCanvas?: HTMLCanvasElement;
  private terrainAt = '';
  private pos: Float32Array = new Float32Array(0);
  private from: Float32Array = new Float32Array(0);
  private born: Uint8Array = new Uint8Array(0);
  /** Which way each organism faces: the way it last walked. */
  private face: Int8Array = new Int8Array(0);
  private faceById = new Map<number, number>();
  private ghosts: Ghost[] = [];
  private moveStart = 0;
  private moveMs = MOVE_MS;
  private rifts: Rift[] = [];
  private foodMax: number[] = [];
  private eventSprites: EventSprite[] = [];
  private highlightClade?: number;
  private highlightOrganism?: number;
  private hover?: Hover;
  private cam: Camera = { x: 0, y: 0, s: 1 };
  private goal: Camera = { x: 0, y: 0, s: 1 };
  private dirty = true;
  private minimap?: HTMLCanvasElement;
  private minimapBase?: HTMLCanvasElement;

  onPick: (p: Pick) => void = () => {};
  onHover: (h: Hover | null, x: number, y: number) => void = () => {};

  constructor(private host: HTMLElement) {}

  async init() {
    await this.app.init({
      resizeTo: this.host,
      backgroundAlpha: 0,
      antialias: true,
      autoDensity: true,
      resolution: Math.min(window.devicePixelRatio || 1, 2),
      preserveDrawingBuffer: true,
    });
    this.host.appendChild(this.app.canvas);
    this.glowTex = Texture.from(glowCanvas());
    this.riftGlow.filters = [new BlurFilter({ strength: 6, quality: 3 })];
    this.range.blendMode = 'add';
    this.world.addChild(
      this.terrain,
      this.detail,
      this.territory,
      this.food,
      this.range,
      this.riftGlow,
      this.riftLines,
      this.events,
      this.bridges,
      this.orgG,
      this.hoverG,
      this.markG,
    );
    this.app.stage.addChild(this.world);
    this.app.ticker.add(() => this.frame());
    this.bindInput();
    new ResizeObserver(() => (this.dirty = true)).observe(this.host);
  }

  setFoodMax(foodMax: number[]) {
    this.foodMax = foodMax;
  }

  setRifts(rifts: Rift[]) {
    this.rifts = rifts;
    if (this.state) this.paintRifts();
  }

  setLayers(l: Layers) {
    this.terrain.visible = this.detail.visible = l.biomes;
    this.territory.visible = l.territories;
    this.food.visible = l.food;
    this.riftGlow.visible = this.riftLines.visible = this.bridges.visible = l.rifts;
    this.events.visible = l.events;
    this.orgG.visible = l.organisms;
    this.dirty = true;
  }

  highlight(clade?: number, organism?: number) {
    this.highlightClade = clade;
    this.highlightOrganism = organism;
    if (this.state) this.paintRange();
    this.dirty = true;
  }

  attachMinimap(canvas: HTMLCanvasElement) {
    this.minimap = canvas;
    const jump = (ev: PointerEvent) => {
      if (!this.state) return;
      const r = canvas.getBoundingClientRect();
      const wx = ((ev.clientX - r.left) / r.width) * this.state.width * C;
      const wy = ((ev.clientY - r.top) / r.height) * this.state.height * C;
      this.centerOn(wx, wy, this.goal.s);
    };
    canvas.addEventListener('pointerdown', (ev) => {
      jump(ev);
      canvas.setPointerCapture(ev.pointerId);
    });
    canvas.addEventListener('pointermove', (ev) => {
      if (ev.buttons) jump(ev);
    });
  }

  // ---------------------------------------------------------------- the camera

  private centerOn(wx: number, wy: number, s: number) {
    s = Math.min(MAX_SCALE, Math.max(MIN_SCALE, s));
    this.goal = { s, x: this.host.clientWidth / 2 - wx * s, y: this.host.clientHeight / 2 - wy * s };
    if (REDUCED) this.cam = { ...this.goal };
    this.dirty = true;
  }

  /** Scales and centers the whole map in the view. */
  fit(instant = false) {
    if (!this.state) return;
    const w = this.state.width * C;
    const h = this.state.height * C;
    const s = Math.min(this.host.clientWidth / w, this.host.clientHeight / h) * 0.94;
    this.centerOn(w / 2, h / 2, s);
    if (instant) this.cam = { ...this.goal };
  }

  zoomBy(factor: number) {
    this.zoomAt(this.host.clientWidth / 2, this.host.clientHeight / 2, factor);
  }

  private zoomAt(sx: number, sy: number, factor: number) {
    const g = this.goal;
    const s = Math.min(MAX_SCALE, Math.max(MIN_SCALE, g.s * factor));
    const wx = (sx - g.x) / g.s;
    const wy = (sy - g.y) / g.s;
    this.goal = { s, x: sx - wx * s, y: sy - wy * s };
    if (REDUCED) this.cam = { ...this.goal };
    this.dirty = true;
  }

  /** Flies to a cell, close enough to see individual organisms. */
  focus(cell: number, zoomPx = GLYPHS_FROM * 1.8) {
    if (!this.state) return;
    const x = ((cell % this.state.width) + 0.5) * C;
    const y = (Math.floor(cell / this.state.width) + 0.5) * C;
    this.centerOn(x, y, Math.max(this.goal.s, zoomPx / C));
  }

  /** Flies to where a clade lives, framing its range. */
  focusClade(clade: number) {
    const s = this.state;
    if (!s) return;
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (let i = 0; i < s.organisms.id.length; i++) {
      if (s.organisms.clade[i] !== clade) continue;
      x0 = Math.min(x0, this.pos[i * 2]);
      x1 = Math.max(x1, this.pos[i * 2]);
      y0 = Math.min(y0, this.pos[i * 2 + 1]);
      y1 = Math.max(y1, this.pos[i * 2 + 1]);
    }
    if (x0 === Infinity) return;
    const pad = C * 3;
    const sc = Math.min(this.host.clientWidth / (x1 - x0 + pad * 2), this.host.clientHeight / (y1 - y0 + pad * 2));
    this.centerOn((x0 + x1) / 2, (y0 + y1) / 2, Math.min(sc, (GLYPHS_FROM * 1.2) / C));
  }

  cellOfOrganism(id: number): number | undefined {
    const o = this.state?.organisms;
    if (!o) return undefined;
    const i = o.id.indexOf(id);
    return i >= 0 ? o.cell[i] : undefined;
  }

  get epoch() {
    return this.state?.epoch;
  }

  // ---------------------------------------------------------------- states

  /** Shows a state; with `animate`, organisms walk from where they were in the previous one. */
  setState(state: MapState, animate: boolean, moveMs = MOVE_MS) {
    this.moveMs = REDUCED ? 0 : moveMs;
    const prev = this.state;
    const prevPos = this.pos;
    this.state = state;
    this.pos = slots(state);
    this.from = this.pos.slice();
    this.born = new Uint8Array(state.organisms.id.length);
    this.face = new Int8Array(state.organisms.id.length);
    this.ghosts = [];
    if (animate && prev && this.moveMs > 0) {
      const index = new Map<number, number>();
      prev.organisms.id.forEach((id, i) => index.set(id, i));
      const still = new Set<number>();
      state.organisms.id.forEach((id, i) => {
        const j = index.get(id);
        if (j === undefined) this.born[i] = 1;
        else {
          still.add(id);
          this.from[i * 2] = prevPos[j * 2];
          this.from[i * 2 + 1] = prevPos[j * 2 + 1];
          const dx = this.pos[i * 2] - this.from[i * 2];
          if (Math.abs(dx) > C * 0.3) this.faceById.set(id, dx > 0 ? 1 : -1);
        }
      });
      prev.organisms.id.forEach((id, j) => {
        if (!still.has(id)) {
          this.ghosts.push({ x: prevPos[j * 2], y: prevPos[j * 2 + 1], hue: prev.organisms.hue[j], traits: prev.organisms.traits.slice(j * 6, j * 6 + 6) });
        }
      });
      this.moveStart = performance.now();
    } else {
      this.moveStart = 0;
    }
    const alive = new Map<number, number>();
    state.organisms.id.forEach((id, i) => {
      const f = this.faceById.get(id) ?? ((Math.imul(id, 2246822519) >>> 31) === 1 ? 1 : -1);
      this.face[i] = f;
      alive.set(id, f);
    });
    this.faceById = alive;
    const key = terrainKey(state);
    if (key !== this.terrainAt) {
      this.terrainAt = key;
      this.terrainCanvas = renderTerrain(state);
      this.setTexture(this.terrain, this.terrainCanvas, C / TERRAIN_PX, 'linear');
      this.paintMinimapBase();
    }
    this.paintFood();
    this.paintTerritory();
    this.paintRange();
    this.paintRifts();
    this.paintEvents();
    if (!prev) this.fit(true);
    this.dirty = true;
  }

  /** Once the camera settles close in, redraws the visible cells at a resolution to match. */
  private scheduleDetail() {
    const s = this.state;
    if (!s) return;
    const px = C * this.cam.s;
    if (px < DETAIL_FROM) {
      this.detail.visible = false;
      this.detailKey = '';
      return;
    }
    const v = this.visible();
    const region: Region = {
      x0: Math.max(0, Math.floor(v.x0 / C)),
      y0: Math.max(0, Math.floor(v.y0 / C)),
      w: 0,
      h: 0,
    };
    region.w = Math.min(s.width, Math.ceil(v.x1 / C)) - region.x0;
    region.h = Math.min(s.height, Math.ceil(v.y1 / C)) - region.y0;
    if (region.w <= 0 || region.h <= 0) return;
    // About one terrain pixel per screen pixel, capped so a redraw stays near 200 ms.
    let P = Math.min(64, Math.ceil(px / 8) * 8);
    while (P > 16 && region.w * region.h * P * P > 700_000) P -= 8;
    const key = `${this.terrainAt}:${region.x0},${region.y0},${region.w},${region.h}@${P}`;
    if (key === this.detailKey) return;
    clearTimeout(this.detailTimer);
    this.detailTimer = window.setTimeout(() => {
      this.detailKey = key;
      const canvas = renderTerrain(s, P, region);
      this.setTexture(this.detail, canvas, C / P, 'linear');
      this.detail.position.set(region.x0 * C, region.y0 * C);
      this.detail.visible = this.terrain.visible;
    }, 180);
  }

  private setTexture(sprite: Sprite, canvas: HTMLCanvasElement, scale: number, mode: 'linear' | 'nearest') {
    const old = sprite.texture;
    const tex = Texture.from(canvas);
    tex.source.scaleMode = mode;
    sprite.texture = tex;
    sprite.scale.set(scale);
    if (old !== Texture.EMPTY) old.destroy(true);
  }

  /** One pixel per cell, scaled up with smoothing: soft patches rather than squares. */
  private cellCanvas(paint: (i: number) => [number, number, number, number] | null): HTMLCanvasElement {
    const s = this.state!;
    const c = document.createElement('canvas');
    c.width = s.width;
    c.height = s.height;
    const ctx = c.getContext('2d')!;
    const img = ctx.createImageData(s.width, s.height);
    for (let i = 0; i < s.width * s.height; i++) {
      const p = paint(i);
      if (p) img.data.set(p, i * 4);
    }
    ctx.putImageData(img, 0, 0);
    return c;
  }

  /** Upscales a one-pixel-per-cell canvas with a blur, so overlays read as soft washes. */
  private soften(src: HTMLCanvasElement, blur: number): HTMLCanvasElement {
    const k = 6;
    const c = document.createElement('canvas');
    c.width = src.width * k;
    c.height = src.height * k;
    const ctx = c.getContext('2d')!;
    ctx.imageSmoothingEnabled = true;
    ctx.filter = `blur(${blur}px)`;
    ctx.drawImage(src, 0, 0, c.width, c.height);
    return c;
  }

  private paintFood() {
    const s = this.state!;
    const c = this.cellCanvas((i) => {
      const max = this.foodMax[s.biome[i]] ?? 0;
      if (max <= 0) return null;
      return [120, 235, 90, Math.min(1, s.food[i] / (max * 10)) * 150];
    });
    this.setTexture(this.food, this.soften(c, 4), C / 6, 'linear');
  }

  /** Each cell tinted by the clade most of its organisms belong to. */
  private paintTerritory() {
    const s = this.state!;
    const o = s.organisms;
    const byCell = new Map<number, Map<number, number>>();
    const hueOf = new Map<number, number>();
    for (let i = 0; i < o.id.length; i++) {
      let m = byCell.get(o.cell[i]);
      if (!m) byCell.set(o.cell[i], (m = new Map()));
      m.set(o.clade[i], (m.get(o.clade[i]) ?? 0) + 1);
      hueOf.set(o.clade[i], o.hue[i]);
    }
    const c = this.cellCanvas((i) => {
      const m = byCell.get(i);
      if (!m) return null;
      let best = 0;
      let n = 0;
      for (const [clade, count] of m) {
        if (count > n) {
          best = clade;
          n = count;
        }
      }
      const col = hueColor(hueOf.get(best) ?? 0, 0.55, 0.7);
      return [(col >> 16) & 255, (col >> 8) & 255, col & 255, 44 + n * 16];
    });
    this.setTexture(this.territory, this.soften(c, 5), C / 6, 'linear');
  }

  /** A glow over the cells where the selected clade lives. */
  private paintRange() {
    const s = this.state;
    if (!s || this.highlightClade === undefined) {
      this.range.visible = false;
      return;
    }
    const count = new Map<number, number>();
    let hue = 0;
    s.organisms.clade.forEach((c, i) => {
      if (c !== this.highlightClade) return;
      count.set(s.organisms.cell[i], (count.get(s.organisms.cell[i]) ?? 0) + 1);
      hue = s.organisms.hue[i];
    });
    const col = hueColor(hue, 0.6, 0.75);
    const c = this.cellCanvas((i) => {
      const n = count.get(i);
      return n ? [(col >> 16) & 255, (col >> 8) & 255, col & 255, 90 + n * 40] : null;
    });
    this.setTexture(this.range, this.soften(c, 5), C / 6, 'linear');
    this.range.visible = true;
  }

  /** Rift lines: the published line a dark dashed seam, an active fault glowing amber. */
  private paintRifts() {
    const s = this.state!;
    const glow = this.riftGlow.clear();
    const lines = this.riftLines.clear();
    this.bridges.removeChildren().forEach((c) => c.destroy());
    if (this.rifts.length === 0) return;
    const at = new Set(this.rifts.map((r) => r.cell));
    const center = (cell: number): [number, number] => [((cell % s.width) + 0.5) * C, (Math.floor(cell / s.width) + 0.5) * C];
    for (const r of this.rifts) {
      const [x, y] = center(r.cell);
      const cx = r.cell % s.width;
      const cy = Math.floor(r.cell / s.width);
      const has = (dx: number, dy: number) => {
        const nx = cx + dx;
        const ny = cy + dy;
        return nx >= 0 && ny >= 0 && nx < s.width && ny < s.height && at.has(ny * s.width + nx);
      };
      for (const [dx, dy] of [[1, 0], [0, 1], [1, 1], [1, -1]]) {
        if (!has(dx, dy)) continue;
        // A diagonal only where the line has no orthogonal step to take.
        if (dx !== 0 && dy !== 0 && (has(dx, 0) || has(0, dy))) continue;
        const n = (cy + dy) * s.width + cx + dx;
        const [x2, y2] = center(n);
        const phase = Math.max(s.rift[r.cell], s.rift[n]);
        if (phase >= 2) glow.moveTo(x, y).lineTo(x2, y2);
        if (phase <= 1) {
          for (let k = 0; k < 4; k++) lines.circle(x + ((x2 - x) * k) / 4, y + ((y2 - y) * k) / 4, C * 0.045);
        }
      }
    }
    glow.stroke({ width: C * 0.55, color: 0xf0a23a, alpha: 0.75 });
    lines.fill({ color: 0x1c140c, alpha: 0.55 });
    for (const r of this.rifts) {
      if (s.rift[r.cell] !== 2) continue;
      const [x, y] = center(r.cell);
      lines.circle(x, y, C * 0.12);
    }
    lines.fill({ color: 0xffd08a, alpha: 0.9 });

    // Land bridges: a gold marker with the bridge's number while it still stands.
    const byBridge = new Map<number, number[]>();
    for (const r of this.rifts) {
      if (r.bridge === 0 || s.rift[r.cell] >= 4) continue;
      const list = byBridge.get(r.bridge) ?? [];
      list.push(r.cell);
      byBridge.set(r.bridge, list);
    }
    for (const [bridge, cells] of byBridge) {
      const cx = cells.reduce((a, c) => a + center(c)[0], 0) / cells.length;
      const cy = cells.reduce((a, c) => a + center(c)[1], 0) / cells.length;
      const g = new Graphics()
        .circle(0, 0, C * 0.62)
        .fill({ color: 0x14100a, alpha: 0.55 })
        .circle(0, 0, C * 0.48)
        .fill({ color: 0xf2c14e })
        .stroke({ width: C * 0.06, color: 0xfff1c9 });
      g.position.set(cx, cy);
      const label = new Text({
        text: String(bridge),
        style: { fontFamily: 'Geist Variable, sans-serif', fontSize: 11, fontWeight: '700', fill: 0x2a1a05 },
        resolution: 4,
      });
      label.anchor.set(0.5);
      label.scale.set(C / 16);
      label.position.set(cx, cy);
      this.bridges.addChild(g, label);
    }
  }

  private paintEvents() {
    const s = this.state!;
    this.events.removeChildren().forEach((c) => c.destroy());
    this.eventSprites = [];
    s.effects.forEach((e, k) => {
      const x = ((e.center % s.width) + 0.5) * C;
      const y = (Math.floor(e.center / s.width) + 0.5) * C;
      const r = (e.radius + 1) * C;
      const glow = new Sprite(this.glowTex);
      glow.anchor.set(0.5);
      glow.position.set(x, y);
      glow.width = glow.height = r * 2.4;
      const ev: EventSprite = { kind: e.kind, glow, r, phase: k * 1.7, strength: Math.min(1, e.remaining_ticks / 48 + 0.35) };
      if (e.kind === 'Ash') {
        glow.tint = 0xff5a1f;
        const core = new Sprite(this.glowTex);
        core.anchor.set(0.5);
        core.position.set(x, y);
        core.width = core.height = r;
        core.tint = 0xffd27a;
        core.blendMode = 'add';
        ev.core = core;
        this.events.addChild(glow, core);
      } else if (e.kind === 'Flood') {
        glow.tint = 0x5fb4ff;
        const ring = new Graphics();
        ring.position.set(x, y);
        ev.ring = ring;
        this.events.addChild(glow, ring);
      } else if (e.kind === 'Rain') {
        glow.tint = 0x4fd1c5;
        const ring = new Graphics();
        ring.position.set(x, y);
        ev.ring = ring;
        this.events.addChild(glow, ring);
      } else if (e.kind === 'Dry') {
        glow.tint = 0xe0a040;
        this.events.addChild(glow);
      } else {
        glow.tint = 0xffb347;
        this.events.addChild(glow);
      }
      this.eventSprites.push(ev);
    });
  }

  // ---------------------------------------------------------------- the minimap

  private paintMinimapBase() {
    if (!this.minimap || !this.terrainCanvas) return;
    const base = document.createElement('canvas');
    base.width = this.minimap.width;
    base.height = this.minimap.height;
    base.getContext('2d')!.drawImage(this.terrainCanvas, 0, 0, base.width, base.height);
    this.minimapBase = base;
  }

  private paintMinimap() {
    const m = this.minimap;
    const s = this.state;
    if (!m || !s || !this.minimapBase) return;
    const ctx = m.getContext('2d')!;
    ctx.drawImage(this.minimapBase, 0, 0);
    const k = m.width / (s.width * C);
    const x = (-this.cam.x / this.cam.s) * k;
    const y = (-this.cam.y / this.cam.s) * k;
    const w = (this.host.clientWidth / this.cam.s) * k;
    const h = (this.host.clientHeight / this.cam.s) * k;
    ctx.fillStyle = 'rgba(8,12,18,0.45)';
    ctx.beginPath();
    ctx.rect(0, 0, m.width, m.height);
    ctx.rect(x, y, w, h);
    ctx.fill('evenodd');
    ctx.strokeStyle = '#e8a33d';
    ctx.lineWidth = 1.5;
    ctx.strokeRect(x, y, w, h);
  }

  // ---------------------------------------------------------------- drawing

  private frame() {
    const s = this.state;
    if (!s) return;
    const now = performance.now();

    // Ease the camera toward its goal; the scale moves in log space.
    const c = this.cam;
    const g = this.goal;
    if (Math.abs(c.s - g.s) > 1e-4 || Math.abs(c.x - g.x) > 0.3 || Math.abs(c.y - g.y) > 0.3) {
      const k = 0.2;
      c.s = Math.exp(Math.log(c.s) + (Math.log(g.s) - Math.log(c.s)) * k);
      c.x += (g.x - c.x) * k;
      c.y += (g.y - c.y) * k;
      this.dirty = true;
    } else if (c.s !== g.s || c.x !== g.x || c.y !== g.y) {
      this.cam = { ...g };
      this.dirty = true;
    }
    this.world.scale.set(this.cam.s);
    this.world.position.set(this.cam.x, this.cam.y);

    // Natural events breathe.
    if (this.events.visible && !REDUCED) {
      const t = now / 1000;
      for (const e of this.eventSprites) {
        const pulse = Math.sin(t * (e.kind === 'Ash' ? 3.1 : 1.3) + e.phase);
        e.glow.alpha = e.strength * (e.kind === 'Ash' ? 0.55 + 0.15 * pulse : 0.3 + 0.08 * pulse);
        if (e.core) {
          e.core.alpha = e.strength * (0.55 + 0.35 * Math.sin(t * 7.3 + e.phase));
          e.core.scale.set((e.r / 128) * (1 + 0.08 * pulse));
        }
        if (e.ring) {
          const p = (((t * 0.45 + e.phase) % 1) + 1) % 1;
          e.ring
            .clear()
            .circle(0, 0, e.r * (0.3 + p))
            .stroke({ width: C * 0.12, color: 0xbfe3ff, alpha: (1 - p) * 0.8 * e.strength });
        }
      }
    }

    const now2 = now;
    const walking = this.moveStart > 0 && now2 - this.moveStart < this.moveMs;
    const pulsing = this.highlightOrganism !== undefined && !REDUCED;
    if (!this.dirty && !walking && !pulsing) return;
    const wasDirty = this.dirty || walking;
    this.dirty = false;
    const p = this.moveStart > 0 ? Math.min(1, (now2 - this.moveStart) / this.moveMs) : 1;
    const e = p < 0.5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
    if (wasDirty) {
      this.drawOrganisms(e, p);
      this.paintMinimap();
      if (this.cam.s === this.goal.s && this.cam.x === this.goal.x && this.cam.y === this.goal.y) this.scheduleDetail();
      else if (C * this.cam.s < DETAIL_FROM) this.detail.visible = false;
    }
    this.drawMarks(e, now2);
  }

  private visible() {
    const sc = this.cam.s;
    const x0 = -this.cam.x / sc - C;
    const y0 = -this.cam.y / sc - C;
    return { x0, y0, x1: x0 + this.host.clientWidth / sc + 2 * C, y1: y0 + this.host.clientHeight / sc + 2 * C };
  }

  private drawOrganisms(e: number, p: number) {
    const s = this.state!;
    const o = s.organisms;
    const g = this.orgG.clear();
    const px = C * this.cam.s;
    const v = this.visible();
    const dim = (i: number) => (this.highlightClade !== undefined && o.clade[i] !== this.highlightClade ? 0.14 : 1);

    if (px < DOTS_FROM) {
      // Herds: one mark per cell, sized by how many live there, colored by the main clade.
      const cells = new Map<number, Herd>();
      for (let i = 0; i < o.id.length; i++) {
        let c = cells.get(o.cell[i]);
        if (!c) cells.set(o.cell[i], (c = { n: 0, hunters: 0, bestN: 0, hue: 0, dim: 0, counts: new Map() }));
        c.n++;
        if (o.kind[i] === 2) c.hunters++;
        const k = (c.counts.get(o.clade[i]) ?? 0) + 1;
        c.counts.set(o.clade[i], k);
        if (k > c.bestN) {
          c.bestN = k;
          c.hue = o.hue[i];
        }
        c.dim = Math.max(c.dim, dim(i));
      }
      for (const [cell, c] of cells) {
        // A small fixed offset per cell, so herds do not stand in a grid.
        const jx = ((Math.imul(cell, 2654435761) >>> 0) % 1000) / 1000 - 0.5;
        const jy = ((Math.imul(cell ^ 0x5bd1e995, 2246822519) >>> 0) % 1000) / 1000 - 0.5;
        const x = ((cell % s.width) + 0.5 + jx * 0.4) * C;
        const y = (Math.floor(cell / s.width) + 0.5 + jy * 0.4) * C;
        const r = C * (0.1 + 0.05 * c.n);
        g.circle(x, y, r).fill({ color: hueColor(c.hue, 0.6, 0.6), alpha: 0.9 * c.dim });
        if (c.hunters > 0) g.circle(x, y, C * 0.12).fill({ color: 0xe0462f, alpha: c.dim });
      }
      return;
    }

    const glyphs = px >= GLYPHS_FROM;
    const size = C * 0.44;
    for (let i = 0; i < o.id.length; i++) {
      const x = this.from[i * 2] + (this.pos[i * 2] - this.from[i * 2]) * e;
      const y = this.from[i * 2 + 1] + (this.pos[i * 2 + 1] - this.from[i * 2 + 1]) * e;
      if (x < v.x0 || x > v.x1 || y < v.y0 || y > v.y1) continue;
      const alpha = (this.born[i] ? e : 1) * dim(i);
      if (glyphs) {
        g.ellipse(x, y + size * 0.36, size * 0.42, size * 0.12).fill({ color: 0x05080c, alpha: 0.3 * alpha });
        drawGlyph(g, x, y, size, o.traits.slice(i * 6, i * 6 + 6), o.hue[i], alpha, this.face[i]);
      } else {
        const r = C * (o.kind[i] === 2 ? 0.2 : 0.16);
        g.circle(x, y + C * 0.04, r).fill({ color: 0x05080c, alpha: 0.35 * alpha });
        g.circle(x, y, r).fill({ color: hueColor(o.hue[i], 0.56, 0.62), alpha });
        if (o.kind[i] === 2) g.circle(x, y, r).stroke({ width: C * 0.06, color: 0xd9412c, alpha });
        else if (o.kind[i] === 1) g.circle(x, y, r).stroke({ width: C * 0.035, color: 0x0b1016, alpha: alpha * 0.5 });
      }
    }
    if (p < 1) {
      for (const ghost of this.ghosts) {
        if (ghost.x < v.x0 || ghost.x > v.x1 || ghost.y < v.y0 || ghost.y > v.y1) continue;
        if (glyphs) drawGlyph(g, ghost.x, ghost.y, size, ghost.traits, ghost.hue, 1 - e);
        else g.circle(ghost.x, ghost.y, C * 0.16).fill({ color: hueColor(ghost.hue), alpha: 1 - e });
      }
    }
  }

  private drawMarks(e: number, now: number) {
    const s = this.state!;
    const o = s.organisms;
    const lw = Math.max(0.6, 1.4 / this.cam.s);
    const h = this.hoverG.clear();
    if (this.hover) {
      const x = (this.hover.cell % s.width) * C;
      const y = Math.floor(this.hover.cell / s.width) * C;
      h.rect(x, y, C, C).stroke({ width: lw, color: 0xffffff, alpha: 0.7 });
      if (this.hover.organism !== undefined) {
        const i = o.id.indexOf(this.hover.organism);
        if (i >= 0) h.circle(this.pos[i * 2], this.pos[i * 2 + 1], C * 0.3).stroke({ width: lw, color: 0xffffff, alpha: 0.9 });
      }
    }
    const m = this.markG.clear();
    if (this.highlightOrganism !== undefined) {
      const i = o.id.indexOf(this.highlightOrganism);
      if (i >= 0) {
        const x = this.from[i * 2] + (this.pos[i * 2] - this.from[i * 2]) * e;
        const y = this.from[i * 2 + 1] + (this.pos[i * 2 + 1] - this.from[i * 2 + 1]) * e;
        const pulse = REDUCED ? 0 : (Math.sin(now / 260) + 1) / 2;
        m.circle(x, y, C * (0.34 + 0.1 * pulse)).stroke({ width: Math.max(1.2, 2.2 / this.cam.s), color: 0xe8a33d, alpha: 1 - pulse * 0.5 });
      }
    }
  }

  // ---------------------------------------------------------------- input

  private toWorld(clientX: number, clientY: number) {
    const r = this.app.canvas.getBoundingClientRect();
    return { x: (clientX - r.left - this.cam.x) / this.cam.s, y: (clientY - r.top - this.cam.y) / this.cam.s };
  }

  private cellAt(x: number, y: number): number | null {
    const s = this.state;
    if (!s) return null;
    const cx = Math.floor(x / C);
    const cy = Math.floor(y / C);
    if (cx < 0 || cy < 0 || cx >= s.width || cy >= s.height) return null;
    return cy * s.width + cx;
  }

  /** The organism nearest a world point within its cell, when organisms are drawn one by one. */
  private organismAt(x: number, y: number, cell: number): number | undefined {
    const o = this.state!.organisms;
    if (C * this.cam.s < DOTS_FROM) return undefined;
    let best = -1;
    let bestD = (C * 0.42) ** 2;
    for (let i = 0; i < o.id.length; i++) {
      if (o.cell[i] !== cell) continue;
      const d = (this.pos[i * 2] - x) ** 2 + (this.pos[i * 2 + 1] - y) ** 2;
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    }
    return best >= 0 ? o.id[best] : undefined;
  }

  private bindInput() {
    const el = this.app.canvas;
    el.addEventListener(
      'wheel',
      (ev) => {
        ev.preventDefault();
        const r = el.getBoundingClientRect();
        this.zoomAt(ev.clientX - r.left, ev.clientY - r.top, Math.pow(1.0018, -ev.deltaY));
      },
      { passive: false },
    );
    el.addEventListener('dblclick', (ev) => {
      const r = el.getBoundingClientRect();
      this.zoomAt(ev.clientX - r.left, ev.clientY - r.top, 2.2);
    });

    const pointers = new Map<number, { x: number; y: number }>();
    let drag: { x: number; y: number; cx: number; cy: number; moved: boolean } | null = null;
    let pinch: { d: number; s: number; mx: number; my: number; cx: number; cy: number } | null = null;
    el.addEventListener('pointerdown', (ev) => {
      el.setPointerCapture(ev.pointerId);
      pointers.set(ev.pointerId, { x: ev.clientX, y: ev.clientY });
      if (pointers.size === 1) {
        drag = { x: ev.clientX, y: ev.clientY, cx: this.goal.x, cy: this.goal.y, moved: false };
      } else if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const r = el.getBoundingClientRect();
        pinch = {
          d: Math.hypot(a.x - b.x, a.y - b.y),
          s: this.goal.s,
          mx: (a.x + b.x) / 2 - r.left,
          my: (a.y + b.y) / 2 - r.top,
          cx: this.goal.x,
          cy: this.goal.y,
        };
        drag = null;
      }
    });
    el.addEventListener('pointermove', (ev) => {
      if (pointers.has(ev.pointerId)) pointers.set(ev.pointerId, { x: ev.clientX, y: ev.clientY });
      if (pinch && pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const s = Math.min(MAX_SCALE, Math.max(MIN_SCALE, pinch.s * (Math.hypot(a.x - b.x, a.y - b.y) / pinch.d)));
        const wx = (pinch.mx - pinch.cx) / pinch.s;
        const wy = (pinch.my - pinch.cy) / pinch.s;
        this.goal = { s, x: pinch.mx - wx * s, y: pinch.my - wy * s };
        this.cam = { ...this.goal };
        this.dirty = true;
        return;
      }
      if (drag) {
        const dx = ev.clientX - drag.x;
        const dy = ev.clientY - drag.y;
        if (Math.abs(dx) + Math.abs(dy) > 4) drag.moved = true;
        if (drag.moved) {
          this.goal = { ...this.goal, x: drag.cx + dx, y: drag.cy + dy };
          this.cam = { ...this.goal };
          this.dirty = true;
        }
      }
      const w = this.toWorld(ev.clientX, ev.clientY);
      const cell = this.cellAt(w.x, w.y);
      const hover = cell === null ? undefined : { cell, organism: this.organismAt(w.x, w.y, cell) };
      if (hover?.cell !== this.hover?.cell || hover?.organism !== this.hover?.organism) {
        this.hover = hover;
        this.dirty = true;
      }
      this.onHover(hover ?? null, ev.clientX, ev.clientY);
    });
    const end = (ev: PointerEvent) => {
      pointers.delete(ev.pointerId);
      if (pointers.size < 2) pinch = null;
      const clicked = drag !== null && !drag.moved && ev.type === 'pointerup';
      drag = null;
      if (!clicked || !this.state) return;
      const w = this.toWorld(ev.clientX, ev.clientY);
      const cell = this.cellAt(w.x, w.y);
      if (cell === null) return;
      const id = this.organismAt(w.x, w.y, cell);
      this.onPick(id !== undefined ? { kind: 'organism', id } : { kind: 'cell', cell });
    };
    el.addEventListener('pointerup', end);
    el.addEventListener('pointercancel', end);
    el.addEventListener('pointerleave', () => {
      this.hover = undefined;
      this.dirty = true;
      this.onHover(null, 0, 0);
    });
  }
}
