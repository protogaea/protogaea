import { Application, Container, Graphics, Sprite, Texture } from 'pixi.js';

import type { MapState, Rift } from './api';
import { drawGlyph, hueColor } from './glyph';

/** World units per cell at zoom 1. */
const C = 16;
/** From this many screen pixels per cell, organisms are drawn as glyphs. */
const GLYPH_FROM = 34;
/** How long organisms take to move to their new places when an epoch arrives; none with reduced motion. */
const MOVE_MS = window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 2600;

const BIOME_COLORS = [0x1d3b5c, 0x3f7aa0, 0x2f5d33, 0x9aa05a, 0xd6c28a, 0x7c746a, 0x4e6b4b];

export interface Layers {
  biomes: boolean;
  food: boolean;
  organisms: boolean;
  rifts: boolean;
  events: boolean;
}

export type Pick = { kind: 'organism'; id: number } | { kind: 'cell'; cell: number };

/** Where each organism is drawn: a slot of a 2 × 2 grid in its cell, by order of id. */
function slots(state: MapState): Float32Array {
  const o = state.organisms;
  const pos = new Float32Array(o.id.length * 2);
  const filled = new Map<number, number>();
  for (let i = 0; i < o.id.length; i++) {
    const cell = o.cell[i];
    const k = filled.get(cell) ?? 0;
    filled.set(cell, k + 1);
    const cx = cell % state.width;
    const cy = Math.floor(cell / state.width);
    pos[i * 2] = (cx + 0.27 + 0.46 * (k % 2)) * C;
    pos[i * 2 + 1] = (cy + 0.27 + 0.46 * Math.floor(k / 2)) * C;
  }
  return pos;
}

interface Ghost {
  x: number;
  y: number;
  hue: number;
  kind: number;
  traits: number[];
}

export class WorldMap {
  readonly app = new Application();
  private world = new Container();
  private biomeSprite = new Sprite();
  private foodSprite = new Sprite();
  private riftG = new Graphics();
  private effectG = new Graphics();
  private orgG = new Graphics();
  private markG = new Graphics();

  private state?: MapState;
  private pos: Float32Array = new Float32Array(0);
  private from: Float32Array = new Float32Array(0);
  /** Organisms of the previous state that are gone: drawn fading out. */
  private ghosts: Ghost[] = [];
  private born: Uint8Array = new Uint8Array(0);
  private moveStart = 0;
  private rifts: Rift[] = [];
  private foodMax: number[] = [];
  private highlightClade?: number;
  private highlightOrganism?: number;
  private dirty = true;

  onPick: (p: Pick) => void = () => {};
  onHover: (cell: number | null, x: number, y: number) => void = () => {};

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
    this.world.addChild(this.biomeSprite, this.foodSprite, this.riftG, this.effectG, this.orgG, this.markG);
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
    this.dirty = true;
  }

  setLayers(layers: Layers) {
    this.biomeSprite.visible = layers.biomes;
    this.foodSprite.visible = layers.food;
    this.riftG.visible = layers.rifts;
    this.effectG.visible = layers.events;
    this.orgG.visible = layers.organisms;
    this.dirty = true;
  }

  highlight(clade?: number, organism?: number) {
    this.highlightClade = clade;
    this.highlightOrganism = organism;
    this.dirty = true;
  }

  get epoch() {
    return this.state?.epoch;
  }

  /** Shows a state; with `animate`, organisms move from where they were in the previous one. */
  setState(state: MapState, animate: boolean) {
    const prev = this.state;
    const prevPos = this.pos;
    const firstTime = !prev;
    this.state = state;
    this.pos = slots(state);
    this.from = this.pos.slice();
    this.born = new Uint8Array(state.organisms.id.length);
    this.ghosts = [];
    if (animate && prev) {
      const index = new Map<number, number>();
      prev.organisms.id.forEach((id, i) => index.set(id, i));
      const still = new Set<number>();
      state.organisms.id.forEach((id, i) => {
        const j = index.get(id);
        if (j === undefined) {
          this.born[i] = 1;
        } else {
          still.add(id);
          this.from[i * 2] = prevPos[j * 2];
          this.from[i * 2 + 1] = prevPos[j * 2 + 1];
        }
      });
      prev.organisms.id.forEach((id, j) => {
        if (!still.has(id)) {
          this.ghosts.push({
            x: prevPos[j * 2],
            y: prevPos[j * 2 + 1],
            hue: prev.organisms.hue[j],
            kind: prev.organisms.kind[j],
            traits: prev.organisms.traits.slice(j * 6, j * 6 + 6),
          });
        }
      });
      this.moveStart = MOVE_MS > 0 ? performance.now() : 0;
    } else {
      this.moveStart = 0;
    }
    this.paintCells();
    if (firstTime) this.fit();
    this.dirty = true;
  }

  /** Scales and centers the whole map in the view. */
  fit() {
    if (!this.state) return;
    const w = this.state.width * C;
    const h = this.state.height * C;
    const s = Math.min(this.host.clientWidth / w, this.host.clientHeight / h) * 0.96;
    this.world.scale.set(s);
    this.world.position.set((this.host.clientWidth - w * s) / 2, (this.host.clientHeight - h * s) / 2);
    this.dirty = true;
  }

  /** Zooms around the center of the view. */
  zoomBy(factor: number) {
    const cx = this.host.clientWidth / 2;
    const cy = this.host.clientHeight / 2;
    const s0 = this.world.scale.x;
    const s = Math.min(40, Math.max(0.3, s0 * factor));
    const wx = (cx - this.world.position.x) / s0;
    const wy = (cy - this.world.position.y) / s0;
    this.world.scale.set(s);
    this.world.position.set(cx - wx * s, cy - wy * s);
    this.dirty = true;
  }

  /** Centers the view on a cell and zooms in to glyphs. */
  focus(cell: number) {
    if (!this.state) return;
    const s = Math.max(this.world.scale.x, (GLYPH_FROM * 2.4) / C);
    const x = ((cell % this.state.width) + 0.5) * C;
    const y = (Math.floor(cell / this.state.width) + 0.5) * C;
    this.world.scale.set(s);
    this.world.position.set(this.host.clientWidth / 2 - x * s, this.host.clientHeight / 2 - y * s);
    this.dirty = true;
  }

  cellOfOrganism(id: number): number | undefined {
    const o = this.state?.organisms;
    if (!o) return undefined;
    const i = o.id.indexOf(id);
    return i >= 0 ? o.cell[i] : undefined;
  }

  private paintCells() {
    const s = this.state!;
    const n = s.width * s.height;
    const biome = document.createElement('canvas');
    biome.width = s.width;
    biome.height = s.height;
    const bctx = biome.getContext('2d')!;
    const bimg = bctx.createImageData(s.width, s.height);
    const food = document.createElement('canvas');
    food.width = s.width;
    food.height = s.height;
    const fctx = food.getContext('2d')!;
    const fimg = fctx.createImageData(s.width, s.height);
    for (let i = 0; i < n; i++) {
      const c = BIOME_COLORS[s.biome[i]] ?? 0;
      // Moisture tints the land slightly darker where it is wet.
      const wet = s.biome[i] >= 2 ? 1 - (s.moisture[i] / 100) * 0.12 : 1;
      bimg.data[i * 4] = ((c >> 16) & 255) * wet;
      bimg.data[i * 4 + 1] = ((c >> 8) & 255) * wet;
      bimg.data[i * 4 + 2] = (c & 255) * wet;
      bimg.data[i * 4 + 3] = 255;
      const max = this.foodMax[s.biome[i]] ?? 0;
      const share = max > 0 ? Math.min(1, s.food[i] / (max * 10)) : 0;
      fimg.data[i * 4] = 40;
      fimg.data[i * 4 + 1] = 230;
      fimg.data[i * 4 + 2] = 80;
      fimg.data[i * 4 + 3] = share * 190;
    }
    bctx.putImageData(bimg, 0, 0);
    fctx.putImageData(fimg, 0, 0);
    for (const [sprite, canvas] of [
      [this.biomeSprite, biome],
      [this.foodSprite, food],
    ] as const) {
      const old = sprite.texture;
      const tex = Texture.from(canvas);
      tex.source.scaleMode = 'nearest';
      sprite.texture = tex;
      sprite.scale.set(C);
      if (old !== Texture.EMPTY) old.destroy(true);
    }
    this.paintRifts();
    this.paintEffects();
  }

  /** Current rift phases, and the schedule: cells that will sink, brighter as their day nears. */
  private paintRifts() {
    const s = this.state!;
    const g = this.riftG.clear();
    for (let i = 0; i < s.rift.length; i++) {
      const phase = s.rift[i];
      if (phase === 0) continue;
      const x = (i % s.width) * C;
      const y = Math.floor(i / s.width) * C;
      if (phase === 1) {
        g.moveTo(x + C * 0.2, y + C * 0.2).lineTo(x + C * 0.8, y + C * 0.8);
      } else if (phase === 2) {
        g.rect(x, y, C, C);
      }
    }
    g.stroke({ width: 0.7, color: 0x2a1a12, alpha: 0.7 });
    g.fill({ color: 0x3b2418, alpha: 0.45 });
    for (const r of this.rifts) {
      if (r.deep_epoch <= s.epoch) continue;
      const x = (r.cell % s.width) * C;
      const y = Math.floor(r.cell / s.width) * C;
      const soon = Math.max(0.15, 1 - (r.deep_epoch - s.epoch) / (r.deep_epoch + 1));
      g.rect(x + C * 0.38, y + C * 0.38, C * 0.24, C * 0.24).fill({
        color: r.bridge > 0 ? 0xf2c14e : 0xe0503c,
        alpha: 0.25 + 0.55 * soon,
      });
    }
  }

  private paintEffects() {
    const s = this.state!;
    const g = this.effectG.clear();
    for (const e of s.effects) {
      const x = ((e.center % s.width) + 0.5) * C;
      const y = (Math.floor(e.center / s.width) + 0.5) * C;
      const r = (e.radius + 0.5) * C;
      if (e.kind === 'Drought') g.rect(x - r, y - r, r * 2, r * 2).fill({ color: 0xff9a2e, alpha: 0.22 });
      else if (e.kind === 'Flood') g.circle(x, y, r).fill({ color: 0x4aa3ff, alpha: 0.28 });
      else g.circle(x, y, r).fill({ color: 0x8a8a8a, alpha: 0.35 });
    }
  }

  private frame() {
    const s = this.state;
    if (!s) return;
    const moving = this.moveStart > 0 && performance.now() - this.moveStart < MOVE_MS;
    if (!this.dirty && !moving) return;
    this.dirty = moving;
    const p = this.moveStart > 0 ? Math.min(1, (performance.now() - this.moveStart) / MOVE_MS) : 1;
    const e = p < 0.5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
    const scale = this.world.scale.x;
    const glyphs = C * scale >= GLYPH_FROM;
    // Only what is on screen, in world units.
    const vx0 = -this.world.position.x / scale - C;
    const vy0 = -this.world.position.y / scale - C;
    const vx1 = vx0 + this.host.clientWidth / scale + 2 * C;
    const vy1 = vy0 + this.host.clientHeight / scale + 2 * C;
    const g = this.orgG.clear();
    const o = s.organisms;
    const size = C * 0.44;
    for (let i = 0; i < o.id.length; i++) {
      const x = this.from[i * 2] + (this.pos[i * 2] - this.from[i * 2]) * e;
      const y = this.from[i * 2 + 1] + (this.pos[i * 2 + 1] - this.from[i * 2 + 1]) * e;
      if (x < vx0 || x > vx1 || y < vy0 || y > vy1) continue;
      let alpha = this.born[i] ? e : 1;
      if (this.highlightClade !== undefined && o.clade[i] !== this.highlightClade) alpha *= 0.18;
      if (glyphs) {
        drawGlyph(g, x, y, size, o.traits.slice(i * 6, i * 6 + 6), o.hue[i], alpha);
      } else {
        const color = o.kind[i] === 2 ? hueColor(o.hue[i], 0.3, 0.7) : hueColor(o.hue[i]);
        g.rect(x - size * 0.4, y - size * 0.4, size * 0.8, size * 0.8).fill({ color, alpha });
        if (o.kind[i] === 1) g.stroke({ width: 1, color: 0xd9dce3, alpha: alpha * 0.8 });
      }
    }
    for (const ghost of this.ghosts) {
      if (p >= 1) break;
      if (ghost.x < vx0 || ghost.x > vx1 || ghost.y < vy0 || ghost.y > vy1) continue;
      if (glyphs) drawGlyph(g, ghost.x, ghost.y, size, ghost.traits, ghost.hue, 1 - e);
      else g.rect(ghost.x - size * 0.4, ghost.y - size * 0.4, size * 0.8, size * 0.8).fill({ color: hueColor(ghost.hue), alpha: 1 - e });
    }
    const m = this.markG.clear();
    if (this.highlightOrganism !== undefined) {
      const i = o.id.indexOf(this.highlightOrganism);
      if (i >= 0) {
        const x = this.from[i * 2] + (this.pos[i * 2] - this.from[i * 2]) * e;
        const y = this.from[i * 2 + 1] + (this.pos[i * 2 + 1] - this.from[i * 2 + 1]) * e;
        m.circle(x, y, C * 0.34).stroke({ width: Math.max(1.5, 2 / scale), color: 0xffffff });
      }
    }
  }

  private toWorld(clientX: number, clientY: number) {
    const r = this.app.canvas.getBoundingClientRect();
    const s = this.world.scale.x;
    return { x: (clientX - r.left - this.world.position.x) / s, y: (clientY - r.top - this.world.position.y) / s };
  }

  private cellAt(x: number, y: number): number | null {
    const s = this.state;
    if (!s) return null;
    const cx = Math.floor(x / C);
    const cy = Math.floor(y / C);
    if (cx < 0 || cy < 0 || cx >= s.width || cy >= s.height) return null;
    return cy * s.width + cx;
  }

  private bindInput() {
    const el = this.app.canvas;
    el.addEventListener(
      'wheel',
      (ev) => {
        ev.preventDefault();
        const before = this.toWorld(ev.clientX, ev.clientY);
        const s = Math.min(40, Math.max(0.3, this.world.scale.x * Math.pow(1.0015, -ev.deltaY)));
        this.world.scale.set(s);
        const r = el.getBoundingClientRect();
        this.world.position.set(ev.clientX - r.left - before.x * s, ev.clientY - r.top - before.y * s);
        this.dirty = true;
      },
      { passive: false },
    );
    let drag: { x: number; y: number; px: number; py: number; moved: boolean } | null = null;
    el.addEventListener('pointerdown', (ev) => {
      drag = { x: ev.clientX, y: ev.clientY, px: this.world.position.x, py: this.world.position.y, moved: false };
      el.setPointerCapture(ev.pointerId);
    });
    el.addEventListener('pointermove', (ev) => {
      if (drag) {
        const dx = ev.clientX - drag.x;
        const dy = ev.clientY - drag.y;
        if (Math.abs(dx) + Math.abs(dy) > 4) drag.moved = true;
        if (drag.moved) {
          this.world.position.set(drag.px + dx, drag.py + dy);
          this.dirty = true;
        }
      }
      const w = this.toWorld(ev.clientX, ev.clientY);
      this.onHover(this.cellAt(w.x, w.y), ev.clientX, ev.clientY);
    });
    el.addEventListener('pointerleave', () => this.onHover(null, 0, 0));
    el.addEventListener('pointerup', (ev) => {
      const wasDrag = drag?.moved;
      drag = null;
      if (wasDrag || !this.state) return;
      const w = this.toWorld(ev.clientX, ev.clientY);
      const cell = this.cellAt(w.x, w.y);
      if (cell === null) return;
      // The nearest organism in the cell, if the click was close to one.
      const o = this.state.organisms;
      let best = -1;
      let bestD = (C * 0.4) ** 2;
      for (let i = 0; i < o.id.length; i++) {
        if (o.cell[i] !== cell) continue;
        const d = (this.pos[i * 2] - w.x) ** 2 + (this.pos[i * 2 + 1] - w.y) ** 2;
        if (d < bestD) {
          bestD = d;
          best = i;
        }
      }
      this.onPick(best >= 0 ? { kind: 'organism', id: o.id[best] } : { kind: 'cell', cell });
    });
  }
}
