// The time machine (roadmap B7): any past epoch, recomputed in the browser from the nearest
// earlier snapshot by the same core the world server runs, and checked against the `state_root`
// the server published for that epoch.

import type { MapState } from './api';
import type { Reply, Request } from './timemachine.worker';

export interface Computed {
  state: MapState & { state_root: string };
  /** The snapshot it was computed from. */
  base: number;
  /** Epochs stepped in the browser for this request. */
  stepped: number;
  ms: number;
}

let worker: Worker | undefined;
/** Where the worker's world is: the snapshot it was loaded from and the epoch it has reached. */
let loaded: { base: number; epoch: number } | undefined;
let busy: Promise<unknown> = Promise.resolve();

function run(req: Request, transfer: Transferable[], onProgress: (epoch: number) => void): Promise<{ epoch: number; map: string }> {
  worker ??= new Worker(new URL('./timemachine.worker.ts', import.meta.url), { type: 'module' });
  const w = worker;
  return new Promise((resolve, reject) => {
    w.onmessage = (ev: MessageEvent<Reply>) => {
      const r = ev.data;
      if (r.kind === 'progress') onProgress(r.epoch);
      else if (r.kind === 'done') resolve(r);
      else {
        loaded = undefined;
        reject(new Error(r.message));
      }
    };
    w.postMessage(req, transfer);
  });
}

/**
 * Computes the world at `target` from the snapshot at `base` (the nearest one at or before it).
 * Continues from the worker's last result when it can, so that stepping forward is cheap.
 */
export function compute(
  target: number,
  base: number,
  fetchSnapshot: (epoch: number) => Promise<ArrayBuffer>,
  onProgress: (done: number, total: number) => void,
): Promise<Computed> {
  const job = busy.then(async () => {
    const t0 = performance.now();
    const cont = loaded && loaded.base === base && loaded.epoch <= target;
    const from = cont ? loaded!.epoch : base;
    const progress = (e: number) => onProgress(e - from, target - from);
    let result;
    if (cont) {
      result = await run({ kind: 'step', target }, [], progress);
    } else {
      const snapshot = await fetchSnapshot(base);
      result = await run({ kind: 'load', snapshot, target }, [snapshot], progress);
    }
    loaded = { base, epoch: result.epoch };
    return {
      state: JSON.parse(result.map),
      base,
      stepped: result.epoch - from,
      ms: performance.now() - t0,
    } as Computed;
  });
  busy = job.catch(() => undefined);
  return job;
}
