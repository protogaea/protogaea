// The last world day replayed in about thirty seconds: compact frames from the server, played on
// the map with organisms walking between them, and the day's stories as captions when they happen.

import type { MapState } from './api';

export interface Frame {
  epoch: number;
  id: Uint32Array;
  cell: Uint16Array;
  clade: Uint32Array;
  hue: Uint16Array;
  kind: Uint8Array;
}

/** Parses the binary format of `/v0/replay` (see the server's API). */
export function parseFrames(buf: ArrayBuffer): Frame[] {
  const view = new DataView(buf);
  const frames: Frame[] = [];
  let o = 0;
  while (o + 8 <= buf.byteLength) {
    const epoch = view.getUint32(o, true);
    const n = view.getUint32(o + 4, true);
    o += 8;
    const f: Frame = {
      epoch,
      id: new Uint32Array(n),
      cell: new Uint16Array(n),
      clade: new Uint32Array(n),
      hue: new Uint16Array(n),
      kind: new Uint8Array(n),
    };
    for (let i = 0; i < n; i++, o += 13) {
      f.id[i] = view.getUint32(o, true);
      f.cell[i] = view.getUint16(o + 4, true);
      f.clade[i] = view.getUint32(o + 6, true);
      f.hue[i] = view.getUint16(o + 10, true);
      f.kind[i] = view.getUint8(o + 12);
    }
    frames.push(f);
  }
  return frames;
}

/** A map state for a frame: organisms from the frame, the land from the live state. */
export function stateOf(frame: Frame, base: MapState): MapState {
  const n = frame.id.length;
  return {
    ...base,
    epoch: frame.epoch,
    effects: [],
    organisms: {
      id: Array.from(frame.id),
      cell: Array.from(frame.cell),
      clade: Array.from(frame.clade),
      hue: Array.from(frame.hue),
      kind: Array.from(frame.kind),
      energy: new Array(n).fill(0),
      age: new Array(n).fill(0),
      // Traits are not in the frames; a replay is watched from afar, where they are not drawn.
      traits: new Array(n * 6).fill(4),
    },
  };
}
