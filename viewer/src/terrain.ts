// An illustrated terrain for the map, drawn from the cells: soft coastlines with beaches and
// shallows, water shaded by depth, hill shading, and a texture for each biome. It is a picture of
// the state, not part of it; organic detail comes from deterministic noise, so the same world always
// looks the same.

import type { MapState } from './api';

/** Pixels of terrain per cell. */
export const TERRAIN_PX = 12;

// Biome indices: 0 deep water, 1 shallows, 2 forest, 3 steppe, 4 desert, 5 mountains, 6 swamp.
const COLORS: [number, number, number][] = [
  [22, 50, 77],
  [47, 111, 143],
  [47, 90, 50],
  [143, 154, 82],
  [205, 181, 127],
  [123, 116, 104],
  [71, 95, 69],
];
const HEIGHT = [-1, -0.35, 0.3, 0.18, 0.22, 1, 0.04];
const DEEP: [number, number, number] = [16, 38, 62];
const SHALLOW: [number, number, number] = [58, 128, 158];
const SAND: [number, number, number] = [214, 196, 150];
const SNOW: [number, number, number] = [232, 234, 236];
const MURK: [number, number, number] = [52, 78, 70];

function hash(x: number, y: number): number {
  let h = (x * 374761393 + y * 668265263) | 0;
  h = (h ^ (h >>> 13)) * 1274126177;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

function noise(x: number, y: number): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const xf = x - xi;
  const yf = y - yi;
  const u = xf * xf * (3 - 2 * xf);
  const v = yf * yf * (3 - 2 * yf);
  const a = hash(xi, yi);
  const b = hash(xi + 1, yi);
  const c = hash(xi, yi + 1);
  const d = hash(xi + 1, yi + 1);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

function fbm(x: number, y: number, octaves = 4): number {
  let sum = 0;
  let amp = 0.5;
  let freq = 1;
  for (let i = 0; i < octaves; i++) {
    sum += amp * noise(x * freq, y * freq);
    freq *= 2.03;
    amp *= 0.5;
  }
  return sum / (1 - Math.pow(0.5, octaves));
}

const mix = (a: number, b: number, t: number) => a + (b - a) * t;
const clamp = (v: number, lo = 0, hi = 1) => Math.max(lo, Math.min(hi, v));

/** A key that changes only when the drawn terrain would change (biomes and rift phases). */
export function terrainKey(s: MapState): string {
  let h = 2166136261;
  for (let i = 0; i < s.biome.length; i++) {
    h = Math.imul(h ^ (s.biome[i] * 8 + s.rift[i]), 16777619);
  }
  return `${s.width}x${s.height}:${h >>> 0}`;
}

export interface Region {
  x0: number;
  y0: number;
  w: number;
  h: number;
}

/**
 * Draws the terrain at `P` pixels per cell, for the whole map or for a region of cells. The noise is
 * in cell units, so a region drawn at a higher resolution matches the whole map and adds finer
 * octaves on top: that is the detail the map shows when zoomed in.
 */
export function renderTerrain(s: MapState, P = TERRAIN_PX, region: Region = { x0: 0, y0: 0, w: s.width, h: s.height }): HTMLCanvasElement {
  const started = performance.now();
  const W = region.w * P;
  const H = region.h * P;
  const extra = Math.max(0, Math.round(Math.log2(P / 8)));
  const canvas = document.createElement('canvas');
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d')!;
  const img = ctx.createImageData(W, H);
  const cell = (cx: number, cy: number) => clamp(cy, 0, s.height - 1) * s.width + clamp(cx, 0, s.width - 1);
  const landOf = (b: number) => (b >= 2 ? 1 : b === 1 ? 0.36 : 0);

  const height = new Float32Array(W * H);
  const colors = new Float32Array(W * H * 3);
  const water = new Uint8Array(W * H);

  const k8 = 8 / P;
  for (let py = 0; py < H; py++) {
    for (let px = 0; px < W; px++) {
      const gx = region.x0 * P + px;
      const gy = region.y0 * P + py;
      const qx = gx * k8;
      const qy = gy * k8;
      // Domain warp: region borders wander instead of following the grid.
      const wx = (fbm(qx / 23, qy / 23, 3) - 0.5) * 2.2 + (fbm(qx / 6, qy / 6, 2 + extra) - 0.5) * 0.7;
      const wy = (fbm(qx / 23 + 40, qy / 23 + 17, 3) - 0.5) * 2.2 + (fbm(qx / 6 + 9, qy / 6 + 31, 2 + extra) - 0.5) * 0.7;
      const u = (gx + 0.5) / P - 0.5 + wx;
      const v = (gy + 0.5) / P - 0.5 + wy;
      const x0 = Math.floor(u);
      const y0 = Math.floor(v);
      const fx = u - x0;
      const fy = v - y0;
      const idx = [cell(x0, y0), cell(x0 + 1, y0), cell(x0, y0 + 1), cell(x0 + 1, y0 + 1)];
      const bw = [(1 - fx) * (1 - fy), fx * (1 - fy), (1 - fx) * fy, fx * fy];

      let land = 0;
      let h = 0;
      for (let k = 0; k < 4; k++) {
        land += bw[k] * landOf(s.biome[idx[k]]);
        h += bw[k] * HEIGHT[s.biome[idx[k]]];
      }
      const coast = land + (fbm(qx / 9, qy / 9, 3 + extra) - 0.5) * 0.22;
      const i = py * W + px;
      let r: number;
      let g: number;
      let b: number;
      if (coast < 0.5) {
        // Water: darker with depth, lighter and greener toward the coast.
        const t = Math.pow(clamp(coast / 0.5), 1.6);
        r = mix(DEEP[0], SHALLOW[0], t);
        g = mix(DEEP[1], SHALLOW[1], t);
        b = mix(DEEP[2], SHALLOW[2], t);
        const ripple = (fbm(qx / 5, qy / 14, 2) - 0.5) * 10;
        r += ripple;
        g += ripple;
        b += ripple;
        water[i] = 1;
        h = -1 + t * 0.6;
      } else {
        // Land: biome colors blended with sharpened weights, so regions stay distinct but soft.
        let wsum = 0;
        let best = 0;
        let bestW = -1;
        r = g = b = 0;
        for (let k = 0; k < 4; k++) {
          const biome = s.biome[idx[k]];
          if (biome < 2) continue;
          const w = Math.pow(bw[k], 3.2) + 1e-6;
          wsum += w;
          r += COLORS[biome][0] * w;
          g += COLORS[biome][1] * w;
          b += COLORS[biome][2] * w;
          if (w > bestW) {
            bestW = w;
            best = biome;
          }
        }
        if (wsum > 0) {
          r /= wsum;
          g /= wsum;
          b /= wsum;
        } else {
          [r, g, b] = COLORS[3];
          best = 3;
        }
        // Texture by biome.
        const n = fbm(qx / 6, qy / 6, 3 + extra);
        if (best === 2) {
          const canopy = fbm(qx / 3.2, qy / 3.2, 3 + extra);
          const f = canopy > 0.55 ? 0.72 : 1.04;
          r *= f;
          g *= f;
          b *= f;
        } else if (best === 3) {
          const streak = (noise(qx / 1.6, qy / 5) - 0.5) * 0.16;
          r *= 1 + streak;
          g *= 1 + streak;
          b *= 1 + streak;
        } else if (best === 4) {
          const dune = Math.sin(qx / 3.1 + qy / 7 + n * 7) * 0.06;
          r *= 1 + dune;
          g *= 1 + dune;
          b *= 1 + dune;
        } else if (best === 5) {
          const ridge = 1 - Math.abs(2 * fbm(qx / 7, qy / 7, 4 + extra) - 1);
          const f = 0.82 + ridge * 0.36;
          r *= f;
          g *= f;
          b *= f;
          h += ridge * 0.9;
          if (ridge > 0.86) {
            const snow = clamp((ridge - 0.86) / 0.1);
            r = mix(r, SNOW[0], snow);
            g = mix(g, SNOW[1], snow);
            b = mix(b, SNOW[2], snow);
          }
        } else if (best === 6) {
          const pool = fbm(qx / 4, qy / 4, 3 + extra);
          if (pool > 0.58) {
            const m = clamp((pool - 0.58) / 0.08);
            r = mix(r, MURK[0], m);
            g = mix(g, MURK[1], m);
            b = mix(b, MURK[2], m);
          }
        }
        // A beach where land meets the sea, except at mountains and swamps.
        const beach = clamp(1 - (coast - 0.5) / 0.07);
        if (beach > 0 && best !== 5 && best !== 6) {
          r = mix(r, SAND[0], beach * 0.85);
          g = mix(g, SAND[1], beach * 0.85);
          b = mix(b, SAND[2], beach * 0.85);
        }
        // A faulted cell darkens, as the ground starts to give way.
        const phase = s.rift[cell(Math.round(u), Math.round(v))];
        if (phase === 2) {
          r *= 0.78;
          g *= 0.74;
          b *= 0.72;
        }
        h += (n - 0.5) * 0.25;
      }
      height[i] = h;
      colors[i * 3] = r;
      colors[i * 3 + 1] = g;
      colors[i * 3 + 2] = b;
    }
  }

  // Hill shading, lit from the north-west.
  for (let py = 0; py < H; py++) {
    for (let px = 0; px < W; px++) {
      const i = py * W + px;
      let shade = 1;
      if (!water[i]) {
        const hx = height[py * W + Math.min(W - 1, px + 1)] - height[py * W + Math.max(0, px - 1)];
        const hy = height[Math.min(H - 1, py + 1) * W + px] - height[Math.max(0, py - 1) * W + px];
        shade = clamp(1 - (hx + hy) * 2.4 * (P / 8), 0.62, 1.3);
      }
      img.data[i * 4] = clamp(colors[i * 3] * shade, 0, 255);
      img.data[i * 4 + 1] = clamp(colors[i * 3 + 1] * shade, 0, 255);
      img.data[i * 4 + 2] = clamp(colors[i * 3 + 2] * shade, 0, 255);
      img.data[i * 4 + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  if (import.meta.env.DEV) console.info(`terrain ${W}x${H} in ${Math.round(performance.now() - started)} ms`);
  (window as unknown as { __terrainMs?: number }).__terrainMs = performance.now() - started;
  return canvas;
}

/** A soft round glow, white, for events and highlights; tinted where it is used. */
export function glowCanvas(size = 128): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d')!;
  const grad = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  grad.addColorStop(0, 'rgba(255,255,255,1)');
  grad.addColorStop(0.35, 'rgba(255,255,255,0.55)');
  grad.addColorStop(1, 'rgba(255,255,255,0)');
  g.fillStyle = grad;
  g.fillRect(0, 0, size, size);
  return c;
}
