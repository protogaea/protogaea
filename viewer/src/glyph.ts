import type { Graphics } from 'pixi.js';

// An organism drawn from its genome when the map is zoomed in (spec §7): legs for movement,
// eyes for perception, jaws for hunting, a shell for defense, a belly for plant eating and a
// brood for fertility. Traits are 0–8, in the order M P G H D F.

const M = 0, P = 1, G = 2, H = 3, D = 4, F = 5;

/** The neutral `hue` gene as a color: related organisms share it. */
export function hueColor(hue: number, lightness = 0.52, saturation = 0.58): number {
  const h = (((hue % 360) + 360) % 360) / 60;
  const c = (1 - Math.abs(2 * lightness - 1)) * saturation;
  const x = c * (1 - Math.abs((h % 2) - 1));
  const [r, g, b] =
    h < 1 ? [c, x, 0] : h < 2 ? [x, c, 0] : h < 3 ? [0, c, x] : h < 4 ? [0, x, c] : h < 5 ? [x, 0, c] : [c, 0, x];
  const m = lightness - c / 2;
  const to = (v: number) => Math.round((v + m) * 255);
  return (to(r) << 16) | (to(g) << 8) | to(b);
}

/**
 * Draws one organism centered at (x, y), `size` across, facing right (`dir` 1) or left (-1).
 * `alpha` fades newborns in and the dead out.
 */
export function drawGlyph(
  g: Graphics,
  x: number,
  y: number,
  size: number,
  traits: ArrayLike<number>,
  hue: number,
  alpha: number,
  dir = 1,
) {
  // Every horizontal offset from the center is mirrored for an organism facing left.
  const X = (dx: number) => x + dx * dir;
  const body = hueColor(hue);
  const dark = hueColor(hue, 0.25, 0.4);
  const bodyW = size * (0.26 + traits[G] * 0.018);
  const bodyH = size * 0.2;
  const line = Math.max(0.6, size * 0.035);

  // Legs: one pair per two points of movement, at least one pair.
  const pairs = 1 + Math.floor(traits[M] / 2);
  for (let i = 0; i < pairs; i++) {
    const lx = X(-bodyW * 0.7 + (bodyW * 1.4 * (i + 0.5)) / pairs);
    const reach = size * (0.16 + traits[M] * 0.01);
    g.moveTo(lx, y - bodyH * 0.6).lineTo(lx - size * 0.05 * dir, y - bodyH - reach * 0.6);
    g.moveTo(lx, y + bodyH * 0.6).lineTo(lx - size * 0.05 * dir, y + bodyH + reach * 0.6);
  }
  g.stroke({ width: line, color: dark, alpha });

  // Brood: small eggs behind the body.
  const eggs = Math.floor(traits[F] / 2);
  for (let i = 0; i < eggs; i++) {
    g.circle(X(-bodyW - size * 0.05), y + (i - (eggs - 1) / 2) * size * 0.07, size * 0.028);
  }
  if (eggs > 0) g.fill({ color: 0xf1e6c8, alpha });

  // Body, with a belly that grows with plant eating.
  g.ellipse(x, y, bodyW, bodyH).fill({ color: body, alpha });
  if (traits[G] > 0) {
    g.ellipse(X(-bodyW * 0.1), y + bodyH * 0.25, bodyW * 0.55, bodyH * (0.25 + traits[G] * 0.04)).fill({
      color: 0xf4efd8,
      alpha: alpha * 0.55,
    });
  }

  // Shell: plates along the back, thicker with defense.
  if (traits[D] > 0) {
    const plates = 1 + Math.floor(traits[D] / 2);
    for (let i = 0; i < plates; i++) {
      const px = X(-bodyW * 0.6 + (bodyW * 1.2 * (i + 0.5)) / plates);
      g.roundRect(px - bodyW / plates / 1.9, y - bodyH * 1.05, bodyW / plates * 1.05, bodyH * (0.5 + traits[D] * 0.05), size * 0.02);
    }
    g.fill({ color: 0x5b5e66, alpha });
  }

  // Jaws: two mandibles at the front, longer with hunting.
  if (traits[H] > 0) {
    const len = size * (0.07 + traits[H] * 0.02);
    const fx = X(bodyW * 0.95);
    g.moveTo(fx, y - bodyH * 0.25).lineTo(fx + len * dir, y - bodyH * 0.05 - len * 0.35);
    g.moveTo(fx, y + bodyH * 0.25).lineTo(fx + len * dir, y + bodyH * 0.05 + len * 0.35);
    g.stroke({ width: line * 1.6, color: 0x7a1d1d, alpha });
  }

  // Eyes: one more for every three points of perception.
  const eyes = 1 + Math.floor(traits[P] / 3);
  for (let i = 0; i < eyes; i++) {
    const ey = y + (i - (eyes - 1) / 2) * bodyH * 0.55;
    g.circle(X(bodyW * 0.62), ey, size * 0.035).fill({ color: 0xffffff, alpha });
    g.circle(X(bodyW * 0.68), ey, size * 0.018).fill({ color: 0x111111, alpha });
  }
}
